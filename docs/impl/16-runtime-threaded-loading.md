# 16. `sapphire-runtime` R5 / R6 — Thread 分離と loading 契約

本文書は R5（`Ruby.run` の Thread 分離）と R6（生成コードから
見たランタイム loading 契約 / バージョン照合）で行った設計判断
を記録する。契約は `docs/spec/11-ruby-monad.md` §Execution model
（規範）と `docs/build/03-sapphire-runtime.md` §Threading model
／§Versioning and the calling convention、基礎実装は
`docs/impl/14-ruby-monad-runtime.md`（R4）に既にあるので、本文書
は **Thread 層をどう挟み、どんな loading 前提を生成コードに要求
するか** に限定する。

状態: **active**。R7 以降（Ractor / parallel コンビネータ、11-OQ1
方向、および I7c 側の version-stamp 方針）は本文書の上に積み上げ
る想定。

## スコープ

- `Ruby.run` を `Thread.new { ... }.value` で包み、spec 11
  §Execution model 項 1 の「fresh Ruby evaluator thread per run」
  を実装レベルで満たす根拠。
- Thread 分離で何が分離され、何が分離されない（されえない）か
  の線引き。生成コード（I7c）が依存してよい不変条件と、そうで
  ないものの明文化。
- `Ruby.run` 再入（`prim_embed` block 中で更に `Ruby.run` を呼ぶ）
  の admit 方針。
- R6: `Sapphire::Runtime.require_version!(constraint)` と、関連
  エラー `Errors::RuntimeVersionMismatch` / `Errors::LoadError`
  の shape。
- 生成コード（I7c）と CLI（I8）への引き継ぎ。

対象外:

- CLI 側の version 照合（I-OQ33）。R6 はランタイム側の `Sapphire::
  Runtime.require_version!` だけを決める。CLI が CI / 起動時に追加
  照合を走らせるかは I8 / D2 で別途決定。
- Ractor / parallel プリミティブ（11-OQ1）。本 R5 の default
  実行モデルは Ractor を使わない。
- `Ruby.run` 返却形の `Result` ADT 昇格（I-OQ40）。R5 到達時点で
  再訪し、**現状維持**（`[:ok, a] / [:err, e]` タプル）を確認
  した（§返却形 参照）。

## Thread 分離方式の選択

spec 11 §Execution model 項 1 は `run` ごとに **fresh Ruby
evaluator thread** を spawn すると規定する。Ruby 実装でこれを
実現する候補は以下の 3 通り。

### (A) `fork` ベース

- 利点: CoW によりメモリ空間が完全に分離され、global / constants
  / 読み込まれた require 状態も `run` ごとに fresh。spec 11 の
  "fresh scope" を字義通り実装できる。
- 欠点: Windows では `fork` が使えない（`sapphire-runtime` は
  `required_ruby_version = "~> 3.3"` を宣言、Windows は first-
  class 対象 B-03-OQ5 関連）。さらに `fork` のコストは `run` ごと
  に重く、M9 例題ではすぐ観測される。IPC で結果を戻す追加層も必要。
- 判定: **却下**。ポータビリティと性能の両方で現行要件を外れる。

### (B) `Thread` + `$SAFE` / 独自 sandbox

- 利点: スレッド内に閉じた sandbox を構築できれば (A) より軽量。
- 欠点: Ruby 3.0 で `$SAFE` は削除済み (`$SAFE = 0` のみ admit)。
  スレッド単位の global 分離は Ruby の言語レベルでは提供されな
  いため、自前で `$XX` を保存 / 復元する層は fragile で網羅的
  でもない。`Ractor` 経由の真の分離は (C) の議論を参照。
- 判定: **却下**。実装コストに対し得られる分離が保証されない。

### (C) `Thread` 分離＋ scope は locals / thread-locals に限定

spec 11 の "fresh Ruby-side scope" を **「Ruby 側 local 変数と
`Thread.current[:...]` フィーバーローカル / スレッドローカル
ストレージまで」** と解釈する。global / top-level constants /
`$LOADED_FEATURES` / monkey-patch は in-process `Thread` では
原理的に分離できないので、これらは `run` 間で共有される前提を
spec 11 の許容範囲内で受け入れる。

- 利点: `Thread.new { ... }.value` 1 本で実装でき、
  `Thread#value` が自動的に caller に例外を再 raise してくれる
  ため `Interrupt` / `SystemExit` の propagation（B-03-OQ5）が
  無料で成立。`Thread.current[:...]` は Ruby が自動的に thread
  ごとにフレッシュなストレージを与えるので、spec 11 の per-step
  scope isolation 実装上の骨子を担保できる。
- 欠点: global / constants の分離は提供しない。生成コード
  （I7c）はこの制約に依存しない形で emit する必要がある。
- 判定: **採用**（I-OQ48 DECIDED）。

spec 11 §Execution model は「leaked locals, globals, or loaded
constants」を **実装が pooling を選ぶ場合** の要求として書いて
いる。v0 ランタイムは pool せず fresh Thread per run を採るので
（B-03-OQ4 draft）、字義通りの「毎回 pristine な Ruby 空間」は
求められていないと読む。実務上、生成コードはグローバル状態に
依存しない純関数的振る舞いを Sapphire 側の型で既に担保しており、
`run` 間で global が共有されるのは「ユーザが `:=` block で `$var`
を書いた場合のみ観測できる」範疇に収まる。そのケースは
B-05-OQ8（`:=` スニペットと Ruby グローバル状態、DEFERRED-LATER）
の追跡対象にしておく。

## 分離の境界

**完全に分離される** (Thread 分離が担保する):

- `prim_embed` block 内の Ruby local 変数。block 自体が Ruby の
  通常のクロージャで毎回 fresh な block-local を持つ（R4 時点で
  spec 11 §Execution model 項 4 を満たしていた）うえに、R5 は
  評価そのものを別 Thread に移すため、caller 側の local とも
  明確に分離される。
- `Thread.current[:...]` — fresh Thread は fresh thread-local
  storage を持つ。caller が書いた `Thread.current[:foo]` は
  evaluator block から見えず、evaluator が書いたものは caller に
  戻らない。2 回連続で `run` を呼んでも `Thread.current[:...]` は
  共有されない。rspec `R5 thread isolation > thread-local
  isolation` で 3 ケース assert。
- evaluator Thread 間の identity — 2 つの `run` は別の `Thread`
  オブジェクトで走る。`Thread#equal?` で確認する。なお
  `Thread#object_id` は MRI が dead Thread の id を再利用する
  ことがあり、**値比較**では main thread と衝突することがある
  ので spec / example は identity 比較を使っている。

**共有される** (in-process `Thread` の原理上分離不能):

- Ruby global 変数 (`$...`)。
- top-level constants (`Sapphire::Runtime::VERSION` など)。これ
  は生成コード側にも必要（`require` 後に定数参照するため）なの
  で、分離してしまうと生成コードが壊れる — これが Ractor を
  採らない主要な理由。
- `require` の load-once 表 (`$LOADED_FEATURES`)。
- Class 変数 (`@@...`)、monkey-patch されたメソッド定義、class
  re-open の効果。

生成コード（I7c）契約としては **これらの process-wide mutable
state に `run` 間の semantic dependence を置かない**。Sapphire
の型付けが pure function を基調にしているため、ユーザが意図的に
`:=` block で global を触らない限り問題は顕在化しない。顕在化
ケースは 11-OQ5（Ruby 側共有状態の脱出口）の範疇に委ねる。

## `Ruby.run` 再入

再入（`prim_embed` block の中で更に `Ruby.run` を呼ぶ）は
**admit** する（I-OQ47 DECIDED）。R5 実装では以下のように振る舞う:

- 外側 `run` が evaluator Thread A を起こし、`evaluate` に入る。
- `:embed` step で block が実行され、その中で `Ruby.run(inner)`
  が呼ばれる。内側 `run` は evaluator Thread B を **更に** 起こし、
  `Thread#value` で join して結果を戻す。
- Thread B は Thread A と別オブジェクト、`Thread.current[:...]`
  も別ストレージ。Thread A は B の完了を同期的に待つので、並列
  実行には **ならない**。
- 内側 `run` が `[:err, RubyError]` を返した場合、それは外側
  block の通常の戻り値として扱われる。内側 `run` の中で起きた
  `StandardError` が外側まで raise することはない（内側 `run`
  が rescue 済みで Result-tuple に畳んでいるため）。
- 逆に内側 `run` が `Interrupt` / `SystemExit` を通過させた場合
  は、Thread B の `value` が外側 block 内で再 raise し、そのまま
  Thread A を脱出、Thread A の `value` が caller に再 raise する
  — 2 段の `Thread#value` で signal propagation が維持される。

再入時に「外側 evaluator Thread が生きたまま内側 evaluator
Thread も生きる」状態は OS リソース上は 2 Thread 並立するだけで、
Ruby の GVL を前提にしても deadlock は起きない（内側は外側の
block 内で同期 join されるだけ）。

rspec `R5 thread isolation > reentrant run` で 4 ケース assert。
example は `examples/runtime-threaded/reentrant.rb`。

## 返却形の再確認 (I-OQ40)

R4 の `Ruby.run` は `[:ok, value] | [:err, RubyError]` の 2 要素
タプルを返していた。R5 で生成コード接続時に再訪する約束だった
が、**現状維持** と判断する。

理由:

1. Ruby 側から `Ruby.run` を直接呼ぶデバッグ / テスト / host 統合
   ユースケース（`examples/runtime-*/*.rb` がまさにこれ）で、
   `case result in [:ok, v]` が素直に書ける。
2. spec 11 §`run` の `Result RubyError a` ADT は最終的には
   Sapphire 側の型で求められるもので、I7c 生成コードが
   `ADT.make(:Ok, [v])` / `ADT.make(:Err, [e])` で包めば 1 行
   で済む。ランタイム側で常に ADT で返すようにしても、生成コード
   側がそれを assume する必要があるだけで、非生成コードから呼ぶ
   ケースが読みづらくなる。
3. R4 実装 (`evaluate` が tuple を返す) と R5 実装（Thread block
   body が同じ tuple を返す）はこの点で互換。将来 ADT 返却に
   切り替える場合も、Thread block の末尾 2 行を書き換えるだけ。

I-OQ40 は **DECIDED（現状維持）** として更新する。

## R6 — loading 契約

生成コード（I7c）が `sapphire-runtime` gem に依存する形を以下
の通りに固める。

### `require` 順序

1. 生成 Ruby ファイルの先頭（shebang / frozen-string-literal
   pragma / provenance comment の後）で:

   ```ruby
   require "sapphire/runtime"
   ```

   これ 1 行で `Sapphire::Runtime::{ADT, Marshal, Ruby,
   RubyError, Errors, VERSION}` と本文書で追加した
   `Sapphire::Runtime.require_version!` が全部ロードされる
   （`docs/build/03-sapphire-runtime.md` §Loading and `require`
   order 契約）。

2. その直後で version 照合:

   ```ruby
   Sapphire::Runtime.require_version!("~> 0.1")
   ```

   制約はコンパイル時に I7c が埋め込む（コンパイラが自身の
   `sapphire-runtime` 依存から抽出した `~> X.Y` 形が有力）。

3. 他の生成 Sapphire モジュールへの `require` はそのあと。順序
   には依らない（02 §Cross-module requires）。

ランタイム gem がそもそも見つからない場合は Ruby 側 `LoadError`
（`::LoadError`、`Sapphire::Runtime::Errors::LoadError` では
**ない**）が step 1 で起きる。ユーザ側でのケアは `Gemfile` か
`$LOAD_PATH` を通すこと（`docs/build/05-runtime-host.md`）。
step 1 の失敗を runtime 側の helper で変換しに行くのは層違い
（自分自身がロードされていないのに runtime の helper は呼べない）
なので、R6 のスコープ外。

### Version 照合: `Sapphire::Runtime.require_version!`

`runtime/lib/sapphire/runtime.rb` に以下を追加した。

```ruby
def self.require_version!(constraint)
  # 型チェック（Gem::Requirement.create は Object を無視して
  # ">= 0" にしてしまうので、明示的に絞る）
  unless constraint.is_a?(String) || constraint.is_a?(Array) || ...
    raise Errors::LoadError, "..."
  end
  requirement = Gem::Requirement.create(constraint) rescue raise Errors::LoadError, "..."
  loaded = Gem::Version.new(VERSION)
  return VERSION if requirement.satisfied_by?(loaded)
  raise Errors::RuntimeVersionMismatch, "...#{requirement}..."
end
```

契約 (I-OQ49 DECIDED):

- 受理する constraint 形: `String`（"~> 0.1" / ">= 0.1.0" 等）、
  `Array[String]`（複数条件の AND）、`Gem::Requirement`、
  `Gem::Version`。それ以外は `Errors::LoadError`。
- 構文不正（`"not a version"` など `Gem::Requirement.create` が
  `Gem::Requirement::BadRequirementError` = `ArgumentError` を
  raise する入力）も `Errors::LoadError`。
- 制約を満たす場合は読み込まれた `VERSION` 文字列を返す（caller
  が log しやすいよう）。
- 制約を満たさない場合は `Errors::RuntimeVersionMismatch`。
  message は required constraint と loaded VERSION の双方、および
  `Gemfile` での解消方法を含む。
- `Errors::RuntimeVersionMismatch` / `Errors::LoadError` は
  いずれも `Errors::Base < StandardError`。したがって
  `Ruby.run` の boundary rescue に引っかかる — load 時（`Ruby a`
  action の外）での呼び出しでは通常の `raise` として propagate
  するが、万一 action 内で呼ばれても `[:err, RubyError]` に畳ま
  れる（rspec `loading_spec.rb > error hierarchy` で assert）。

Ruby の top-level `::LoadError` は `ScriptError < Exception` で
あり `StandardError` ではない。`Sapphire::Runtime::Errors::
LoadError` をあえて同名にしたのは、「sapphire-runtime の文脈で
ロードに失敗した」というセマンティクスが `::LoadError` に近い
からで、**継承関係はあえて持たせない**。`StandardError` 下に
居ることが boundary rescue に入る条件として重要なので、
`::LoadError` の下に入れてしまうと `Ruby.run` の rescue scope
を逸脱する。rspec で ancestors チェックを入れている。

### CLI 側照合 (I-OQ33) との関係

CLI (`sapphire`) が起動時に自分自身の埋め込み version と
`sapphire-runtime` gem の VERSION を照合する話は I8 / D2 で決める
（`docs/impl/12-packaging.md` §5）。R6 のスコープは **生成 Ruby
が load 時点で行う照合** だけ。両者は独立で、CLI 側 check が
fail したら生成フェーズで止まり、R6 の check は runtime フェーズ
で止まる、という 2 層防御になる。

## 生成コード (I7c) への引き継ぎ

I7c が emit する Sapphire モジュール 1 本あたりの頭は以下の形で
揃える想定:

```ruby
# frozen_string_literal: true
# Generated by sapphire X.Y.Z on <timestamp>
# Source: src/foo/bar.sap

require "sapphire/runtime"

Sapphire::Runtime.require_version!("~> 0.1")

module Sapphire
  module Foo
    module Bar
      # ...（module 本体）
    end
  end
end
```

- `require_version!` の constraint は I7c に定数で持たせる
  （コンパイラ側で単一の source of truth から引く）。
- `require` 順序については上記の通り、`require "sapphire/runtime"`
  が最初、次に `require_version!`、最後に他のモジュールの require
  と本体定義。
- `run` の返却形は tuple のまま（I-OQ40 現状維持）なので、
  Sapphire 側で `Result` ADT として扱いたい場合は I7c が
  `ADT.make(:Ok, [v])` / `ADT.make(:Err, [e])` で包む。

## R7 以降 (将来)

- **Ractor / parallel コンビネータ**（11-OQ1）。`Ruby.run` が
  spawn する Thread を Ractor に差し替えると global も分離できる
  が、top-level constants が読めなくなり生成コードが動かない。
  parallel プリミティブ限定で Ractor を使う道を探る余地はある。
- **スレッドプール**（B-03-OQ4）。measured cost が必要性を
  示せば pool を入れる余地があるが、spec 11 の per-step scope
  isolation を担保するため pool-reset のロジックが増える。R7 の
  判断。
- **タイムアウト / キャンセル**（11-OQ2）。Thread 分離があれば
  `thread.kill` や `Timeout.timeout` の受け皿は作れるが、spec 11
  が silent なので仕様決着が先。
- **host application 側からの公開 API**（B-03-OQ7）。host Ruby
  が `Ruby.run` を直接呼ぶケースの API を磨く。本 R5 の現状で
  既に使える（タプル返り値のおかげ）が、sugar 追加は別案件。

## 他文書との関係

- **`docs/spec/11-ruby-monad.md`**: 規範。R5 は §Execution model
  項 1 の「fresh thread per run」を実装レベルで満たした。
- **`docs/spec/10-ruby-interop.md`**: `RubyError` / `StandardError`
  scope は R4 踏襲。
- **`docs/build/03-sapphire-runtime.md`**: §Threading model /
  §Versioning and the calling convention / §Loading and `require`
  order。R5 / R6 が埋めた部分。
- **`docs/impl/08-runtime-layout.md`**: R1 のレイアウト。
  `runtime.rb` / `errors.rb` / `ruby.rb` のファイル配置は不変、
  内容追加のみ。
- **`docs/impl/14-ruby-monad-runtime.md`**: R4 設計書。
  「スレッド分離は R5 送り」「`run` の返却形は R5 で再訪」と
  書いた引き継ぎ項目を本文書で閉じた。
- **`docs/impl/12-packaging.md`**: I-OQ33（CLI と runtime gem
  の version 一致ポリシー）は R6 のスコープ外、D2 / D3 で決着。
