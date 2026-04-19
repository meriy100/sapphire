# 14. `sapphire-runtime` R4 — `Ruby` effect monad primitives

本文書は R4（`Sapphire::Runtime::Ruby` effect monad 評価器）の
実装で行った設計判断を記録する。契約は `docs/spec/11-ruby-
monad.md`（規範）と `docs/build/03-sapphire-runtime.md` §The
`Ruby` monad evaluator（Ruby 側の契約）、基礎実装は
`docs/impl/11-runtime-adt-marshalling.md`（R2/R3）に既にあるの
で、**本文書は契約をどう R4 の Ruby コードに畳み込んだか** に
限定する。

状態: **active**。R5（`RubyError` 拡充＋スレッド分離）と R6
（生成コードのロード契約）で本実装の上に積み上げる。

## スコープ

- `runtime/lib/sapphire/runtime/ruby.rb` のファイル名採否と
  ディレクトリ配置の根拠。
- effect monad 値（`Ruby a`）の内部表現（opaque `Action` クラス、
  `:pure` / `:embed` / `:bind` の 3 kind、thunk の保持）。
- `prim_return` / `prim_bind` / `prim_embed` / `run` の責務境界と、
  spec 11 / spec 10 のどの条項をどの Ruby コードに落としたかの
  対応表。
- R5 / R6 への引き継ぎ（スレッド分離、ランタイムバージョン検証、
  `Result` タグ付きハッシュへの格上げ）。
- 11-OQ1（並列合成 / Ractor 方針メモ）との関係。

対象外:

- 型検査・codegen 側。R4 は **Ruby 側プリミティブのみ** で、I6c
  / I7c は別 track。
- スレッド分離の実装（spec 11 §Execution model が要求する「fresh
  thread per `run`」の OS スレッド境界）。R5 のスコープとして
  `docs/impl/06-implementation-roadmap.md` §Track R が既に切って
  いる。本 R4 では同期実行で契約を満たす（後述）。

## ファイル名: `ruby.rb` を踏襲

新規作成ではなく、R1（`docs/impl/08-runtime-layout.md` §ファイル
構成）が敷いた `lib/sapphire/runtime/ruby.rb` の空モジュール
プレースホルダに実装を入れる形を採る。`ruby_monad.rb` への改名も
候補だったが、以下の理由で `ruby.rb` のまま。

1. **R1 のレイアウト契約との整合**。`docs/impl/08-runtime-
   layout.md` は `ruby.rb` 名で `require` 順序・テストファイル対を
   敷いており、改名すると `lib/sapphire/runtime.rb` の
   `require "sapphire/runtime/ruby"` も触る必要がある。R1 以降の
   各実装トラックは R1 のレイアウトを触らずに積み増す方針を
   `docs/impl/11-runtime-adt-marshalling.md` §R1 レイアウトとの
   整合 で既に表明済。本文書もこの方針を継ぐ。
2. **`build/03` との整合**。build 03 §Sub-module map が
   `Sapphire::Runtime::Ruby` をサブモジュール名として確定済。
   ファイル名も同じく `ruby.rb` が自然。
3. **命名衝突の不在**。effect monad の型名は Sapphire 側で
   `Ruby a` だが、Ruby ランタイム側では型ではなく **モジュール**
   `Sapphire::Runtime::Ruby` が対応し、モナド値は `Ruby::Action`
   というネストしたクラスで opaque に持つ。モジュール名とクラス名
   が別レイヤなので `ruby.rb` でも混乱しない。

rspec ファイル名のほうは `ruby_monad_spec.rb` を採る。`ruby_spec`
だと同梱される言語ランタイムのスペック用途と紛れるため、
"monad" を補うことで可読性を上げた。生成側（`ruby.rb`）と
spec 側（`ruby_monad_spec.rb`）がファイル名対応しない点は
R3 の `marshal.rb` / `marshal_spec.rb` との一貫性をわずかに
崩すが、spec ファイル名は契約ではないので許容する。

## effect monad 値の内部表現

### opaque クラス `Ruby::Action`

spec 11 §Type signature は `Ruby a` を opaque と規定するので、
内部表現は **クラス** で隠す。タグ付きハッシュ（`{ tag:, values: }`）
を流用しない理由は以下。

- spec 10 §ADTs と同じ shape にしてしまうと `ADT.tagged?` が
  true を返し、生成コードの ADT ヘルパ（spec 10 §ADTs、R2）と
  見分けが付かなくなる。effect monad 値は ADT ではないので、
  `ADT.tagged?(action) == false` を明確に担保したい。rspec の
  "opacity of action values" セクションで 3 kind すべてに
  対し `tagged?` が false であることを assert する。
- frozen クラスインスタンスは default `Object#==` で identity
  比較になり、effect monad 値について構造的等価を定義しない
  という spec 11 §`run` の「Ruby 側の effect は非決定的であり
  うる」ニュアンスと整合する。ADT のように構造的等価を持たせる
  と「同じ action description が同じ結果を意味する」と誤読しやすい。
- Marshal との区別も付く。`Marshal.from_ruby(action)` は
  `Action` インスタンスを `Hash` / `Array` / `Integer` 等のどの
  既存 case にもマッチさせず、`else` 節で `MarshalError` を
  raise する。これにより spec 11 §There is no `unsafeRun` /
  `runIO` の「effect monad 値は境界をデータとしては渡らない」
  不変条件が自動的に成立する。rspec で from_ruby / to_ruby の
  両方について MarshalError を assert する。

`Action` は `kind :: Symbol`（`:pure` / `:embed` / `:bind` の
いずれか）と `payload` の 2 フィールドを持ち、生成直後に
`freeze`。kind が未知の場合は `BoundaryError` を raise する
（R2/R3 の `ADT.make` と同じ方針）。

### thunk の保持

- `:pure` — `payload` に Sapphire-ready な値そのもの（`prim_return`
  が受け取った value）。評価時は `payload` を返すだけ。
- `:embed` — `payload` に zero-arity の `Proc`。評価時に
  `payload.call` を走らせ、結果を `Marshal.from_ruby` でラップ。
  spec 11 §Execution model item 2（`:=` sub-step は Ruby source
  を実行して結果を marshal）に対応。
- `:bind` — `payload` に `[upstream_action, k]` の 2 要素配列。
  評価時に upstream をまず評価し、得られた値に `k` を適用して
  **次の `Action`** を得て、そのまま継続評価する。

### 再入可能性と反復評価

`evaluate` は内部ループで `:bind` の **右側 spine** を iterative
に剥がしていく。`:bind` の継続 `k` が返した次の `Action` を
`current` に差し替えて同じループで消費するため、**右結合** bind
chain は任意の深さで tail に消費されコールスタックを伸ばさない。
do-notation の脱糖（spec 11 §do-notation は右結合 chain を生む）
から来る Sapphire 側の bind chain はこの経路に入るので、Sapphire
ソースから生成される chain はスタック安全。

一方、`:bind` の左側 `upstream_action` の評価は `evaluate(ma)`
の再帰呼び出しで処理される。Ruby 側から手書きで **左結合** bind
chain `((m >>= f) >>= g) >>= h`（Haskell でいう `foldl (>>=)`
相当）を構築すると、N 段のネストが Ruby のコールスタック N 段と
して現れる。M9 範囲の例題は左結合 chain を出さないので実害は
観測されないが、Ruby 側で `foldl (>>=) m fs` のような
構築を行うと十分な深さで `SystemStackError` に達しうる。必要に
なったら (a) `prim_bind` 構築時に右結合へリバランスする、または
(b) `evaluate` を明示的な work-stack に書き換える、のどちらかで
対処する方針。どちらを選んでも右結合経路の tail consumption は
維持できる。rspec の "sequences three binds in order" / monad
laws セクションで基本的な連鎖深度を確認。

**再入自体は admitted**（`prim_bind` の継続中で別 action を
`run` することは Ruby 側からは可能）。ただし spec 11
§Execution model は「`>>=` は step ごとに完全に評価される」
としか言っておらず、再入中のネストした `run` の意味論は未規定。
R4 の実装ではネスト `run` を特別扱いせず「それぞれ独立に同期
評価される」として通すが、これが意図しない副作用（同じ Ruby
スレッド上で複数の `run` が重なる）を起こしうる点は
**I-OQ39** で追跡する。

## プリミティブ名と spec 11 のクロスリファレンス

| spec 11 §Primitives (Sapphire) | R4 実装 (Ruby)     | 備考 |
|---|---|---|
| `primReturn : a -> Ruby a`               | `Ruby.prim_return(value)` | snake_case は Ruby 慣例。 |
| `primBind   : Ruby a -> (a -> Ruby b) -> Ruby b` | `Ruby.prim_bind(action, &k)` | 継続は block で受け、内部では `Proc` として保持。 |
| (なし — `:=` 束縛は spec 10 側)         | `Ruby.prim_embed(&body)` | spec 10 §The embedding form の `:=` 束縛を R4 の primitive 層に射影した橋渡し。`build/03` §`:=`-bound snippet entry の `snippet` helper に相当。 |
| `run : Ruby a -> Result RubyError a`     | `Ruby.run(action)` | 現在は `[:ok, a]` / `[:err, e]` のタプルを返す。タグ付きハッシュの `Result` ADT への格上げは R5。 |

Sapphire side の camelCase と Ruby side の snake_case の 1:1
対応は生成コード（I7c）が橋渡しする想定で、R4 はランタイム側の
surface だけを整える。

## `run` の返却形: タプル vs `Result` タグ付きハッシュ

R4 では `run` は `[:ok, value] | [:err, RubyError]` のタプル（2
要素 Array）を返す。理由:

- `case run(action) in [:ok, v]; ...; in [:err, e]; ...; end`
  のようなパターンマッチが Ruby 側からそのまま書ける。examples /
  rspec で自然。
- 最終的に spec 11 §`run` が規定する `Result RubyError a` =
  `{ tag: :Ok, values: [a] } | { tag: :Err, values: [e] }` への
  包みは、生成コード（I7c）が `Ruby.run` の戻り値を受けて
  `ADT.make(:Ok, [v])` / `ADT.make(:Err, [e])` するレイヤで
  行えば十分。ランタイム側でタグ付きハッシュを返しても悪くは
  ないが、「Ruby から直接 `run` を呼ぶデバッグ / テストユース
  ケース」をサポートするにはタプルのほうが読みやすい。
- R5（ランタイムバージョン検証＋生成コード接続）でこの選択を
  再訪する。ランタイム側で `Result` ADT に包む形に切り替えて
  も生成コード側のコード量に影響は出ないので、後から動かせる
  判断。

この判断は **I-OQ40** として追跡する（R5 到達時点で確認）。

## 例外 boundary: `StandardError` と内部エラー

`run` は `begin / rescue StandardError / end` で包み、`StandardError`
のサブクラスをすべて捕捉して `RubyError.from_exception(e)` で
Sapphire 側 ADT に包む。B-03-OQ5 DECIDED（2026-04-18）に忠実に、
システム例外（`Interrupt` / `SystemExit` / `NoMemoryError` /
`SystemStackError`）は捕捉せず propagate する。rspec で
`Interrupt` が境界を突き抜けることを assert。

### ランタイム内部例外も境界で包む

`Sapphire::Runtime::Errors::Base < StandardError` により、
`BoundaryError` / `MarshalError` も effect monad の走行中に
raise された場合は rescue 対象に入る。これは **仕様通り**:
`docs/build/03-sapphire-runtime.md` §Errors namespace が
「When such an error is raised *inside* a `Ruby a` action's
execution, the boundary catch repackages it as a `RubyError`
like any other exception」と明記している。

結果として「生成コード側の calling-convention 違反（例:
`prim_bind` の継続が Action を返さない）」も `[:err, RubyError]`
として Sapphire 側に surface する。rspec の「継続が Action
を返さないケース」で class_name
`"Sapphire::Runtime::Errors::BoundaryError"` を assert して、
この挙動をテストに固定した。

### backtrace と文字列サニタイズ

`RubyError.from_exception` は以下の正規化を行う:

- `class_name` は `e.class.name`（匿名クラスは空文字列へフォール
  バック）。
- `message` は `e.message.to_s`（`#message` が非 String を返す
  極端なケースへ対応）。
- `backtrace` は `e.backtrace || []`（spec 10 §Exception model
  が nil 許容と書いている）を `List String` 契約へ合わせ、各
  要素も to_s + UTF-8 化。
- すべて UTF-8 不正バイト列は `scrub("?")` / `encode(..., invalid:
  :replace)` で差し替え、raise しない。境界は「失敗を誠実に
  伝える」ための層であり、backtrace の文字化けで二次的な raise
  を起こすと情報を失う。

## スレッド分離は R5 送り

> **R5 で完了**: 本節は R4 時点の判断ログ。R5 で `Ruby.run` は
> 実際に `Thread.new { ... }.value` でフレッシュな evaluator
> Thread を挟む形になった。設計と分離境界の詳細は
> `docs/impl/16-runtime-threaded-loading.md` を参照。以下は R4
> が「単一スレッド同期でも契約を満たす」と判断した経緯の記録で、
> R5 がその判断を覆したポイント（`Thread#value` による `Interrupt`
> 等の自動再 raise、`Thread.current[:...]` のフリー分離、再入時
> の独立 evaluator Thread）は 16 章で更新済。
>
> また、spec 11 §Execution model 項 1 の「fresh Ruby-side scope」
> は in-process `Thread` 分離では **locals と `Thread.current[:...]`
> まで** しかカバーできない（I-OQ48 DECIDED）。global 変数 /
> top-level constants / `$LOADED_FEATURES` / monkey-patch は
> `run` 間で共有される前提になるため、生成コード（I7c）はこれら
> の process-wide mutable state に依存しない契約を持つ。判断根拠
> は 16 章 §Thread 分離方式の選択 / §分離の境界。

spec 11 §Execution model 冒頭は `run` が **fresh Ruby thread** を
spawn すると規定し、`build/03` §Threading model も同趣旨。ただし
実行意味論として観測可能な事項は以下の 5 点のみで、**単一スレッド
上で同期評価してもこれらはすべて満たせる**:

1. 呼び出し側（Sapphire caller 相当）が `run` で block。
   → 同期評価でも block する。
2. sub-step が順次実行される。
   → iterative evaluator で保証。
3. `:=` sub-step は fresh local scope を持つ。
   → `prim_embed(&body)` の block は Ruby の通常のクロージャで
      毎回 fresh local を持つ（Ruby の block scoping）。
4. 例外は以降の sub-step を short-circuit する。
   → rescue 1 回で `evaluate` を抜ける形で保証、rspec で assert。
5. thread は **同一 Ruby プロセス**。
   → 同期評価は当然満たす。

Sapphire-side caller を Ruby 側の別スレッドで走らせる必要性は
R4 の spec surface には現れず、「Ruby VM state が Sapphire 側と
分離される」という `docs/project-status.md` 由来の motivation も
Ruby 側 local scope の分離だけで実用上十分。

本実装は **単一スレッド同期** で R4 契約を満たす。`Thread.new`
を挟んで `value` で join するラッパを被せる実装変更は R5 の
スコープ（`docs/impl/06-implementation-roadmap.md` §Track R R5:
「`RubyError` + 境界 rescue（`StandardError` scope）」）。R5 が
thread 分離を正式に入れる時点で、本 R4 実装の `run` は
`thread.value` を内部で呼ぶ形に 1 行差し替えるだけで済む粒度で
書いた。

`build/03` §Threading model が明示する「pool する場合は per-step
scope isolation を担保する」は、R5 以降で Ractor（11-OQ1 方針
メモ）を parallel コンビネータ限定で使う場合に再訪する。本 R4
default 実行モデルは Ractor を使わない。

## I7c 生成コードのインタフェース想定

生成コード（I7c）が `Ruby.prim_embed` を呼び出す形を想定しておく。
spec 10 §The embedding form の Sapphire surface

```
rubyGreet : String -> Ruby {}
rubyGreet name := """
  puts "Hello, #{name}!"
"""
```

は I7c で以下のような Ruby メソッドを emit する想定。

```ruby
def self.rubyGreet(name)
  # name は Ruby 側で既に marshal 済（Sapphire String -> Ruby
  # String、spec 10 §Ground types）として渡ってくる。
  Sapphire::Runtime::Ruby.prim_embed do
    puts "Hello, #{name}!"
    # Ruby {} = 空レコード = Ruby 側では {} という空ハッシュ。
    # spec 10 §Records。Marshal.from_ruby({}) は凍結した空ハッシュ。
    {}
  end
end
```

- 引数 `name` は `prim_embed` の block がクロージャとして
  キャプチャし、`run` で実評価される時点で参照される。spec 11
  §Execution model item 4（per-step scope isolation）との
  整合: Ruby 側 local scope は block 単位で fresh なので、
  過去のスニペットの locals は見えない。
- do 記法の脱糖で出てくる chain は I7c が `prim_bind` を
  ネストして emit することになる。`examples/runtime-monad/
  chained_bind.rb` が実際に出てくるコード形の縮約版。

block がキャプチャするクロージャに別の ADT / 関数値が紛れ込む
場合の相互作用は **I-OQ39** で追跡する（キャプチャした ADT を
block 内で破壊できる可能性は `ADT.make` が frozen を強制して
いるので低いが、関数値 / lambda が非純粋な場合の referential
transparency の境界は厳密には spec 11 §`run` の「Ruby 側は非
決定的でありうる」許容範囲で吸収される）。

## 新規 OQ

本文書の判断から派生する OQ を `docs/open-questions.md` §1.5 に
`I-OQ39` 以降で登録する（39〜43 予約、使い切らなくてよい）:

- **I-OQ39**: effect monad の `run` 再入と `prim_embed` block の
  closure キャプチャ相互作用。ネストした `run` の意味論、および
  block 内で関数値 / lambda をキャプチャした際の純粋性境界。
  spec 11 §Execution model は再入について silent。11-OQ5（Ruby
  側共有状態の脱出口）と境界が接近するが、11-OQ5 は「`:=` 群で
  Ruby-side の mutable state を共有したい」ユースケースなので
  別問題。R5 で thread 分離が入った時点で再評価。
- **I-OQ40**: `Ruby.run` が返す形を `[:ok, a] | [:err, e]`
  タプルにするか `Result` ADT `{ tag: :Ok, ... }` まで包むか。
  R4 default はタプル、R5 で生成コード接続時に再訪。
- **I-OQ41**: `prim_embed` の block 内で `Interrupt` を raise した
  ときの挙動。`StandardError` scope の catch を通らず伝播する
  のが現仕様 (B-03-OQ5 DECIDED) だが、ensure での後片付けが
  必要な Ruby スニペット（開いたファイル / ソケット）が出てきた
  ときにどこまで責務を持つか。11-OQ2（タイムアウト）と境界が
  重なる。

11-OQ2（タイムアウト）・11-OQ5（共有状態）との境界: R4 は
**default 実行モデル**（単一スレッド同期、`StandardError` rescue）
の範囲内で閉じており、タイムアウトや共有状態はすべて R4 の
surface には入れない。これらを入れるには spec 11 側の OQ を
decide する必要がある。

## 他文書との関係

- **`docs/spec/11-ruby-monad.md`**: 規範。R4 の全判断は spec 11
  §Execution model / §`run` / §Class instances のどこを根拠に
  したかを参照できるようにしている。
- **`docs/spec/10-ruby-interop.md`**: `RubyError` スキーマと
  `StandardError` scope の source of truth。
- **`docs/build/03-sapphire-runtime.md`**: Ruby gem としての
  契約。R4 は §The `Ruby` monad evaluator と §Threading model
  の default を埋めた。pool / fresh-thread の選択は R5 へ。
- **`docs/impl/08-runtime-layout.md`**: R1 のレイアウト決定。
  本文書は `ruby.rb` に実体を入れる判断の根拠を示している。
- **`docs/impl/11-runtime-adt-marshalling.md`**: R2/R3 の実装
  方針書。本文書はその上に R4 を積む方針の続編。
- **`docs/impl/06-implementation-roadmap.md`**: R4 の完了条件を
  与える。R5 以降へ送った項目は本文書の「スレッド分離は R5
  送り」「新規 OQ」節で明示した。
