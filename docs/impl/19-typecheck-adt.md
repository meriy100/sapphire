# 19. 型検査 I6b: ADT と record

Status: **draft**. I6b 着地 commit 時点のメモ。

## スコープ

spec 03 (data types) と spec 04 (records) を I6a に上乗せする：

- `data T a₁ ... aₙ = C₁ τ₁₁ ... | ... | Cₘ ...` の kind 計算と ctor scheme 生成。
- constructor pattern の arity check と引数型分解。
- structural records (`{ f₁ : τ₁, ... }`)、record literal / update / field access / record pattern。
- `type T a b = τ` の transparent alias。

## 設計

### data 宣言の登録

`infer::register_ast_data` で次を行う：

1. `TypeEnv.datas` にまず名前を登録 (ctor list は空)。これにより
   相互再帰 (`data List a = Nil | Cons a (List a)`) に対応できる。
2. type parameters ごとに fresh rigid TyVar を作成し locals に
   入れる。
3. 各 ctor について `args... -> T p₁ ... pₙ` 型を組み立てて
   `TypeEnv.ctors` と `TypeEnv.globals` に登録。
4. 最後に `DataInfo.ctor_names` を埋める。

kind は `* -> * -> ... -> *` (引数個数分) で統一。

### record

`Ty::Record(Vec<(String, Ty)>)` で持つ。field は **常に name 昇順**
でソートして格納するので、構造的等価性は `PartialEq` で成立する。
unification も順序揃いで walk する (`unify.rs` の Record case)。

### record literal / update / field access

- literal: 各 field expr を infer し、得られた型でフィールドを並
  べるだけ。
- update `{ e | f = ... }`: 左辺を infer → record 型に unify →
  各 field を unify した後、同じフィールドを type 置き換えた新
  record を返す。
- field access `e.f`: 左辺が record なら直接 lookup、fresh var な
  ら **pending_fields** に登録して後で解決する。spec 04 は closed
  records なので row polymorphism に踏まない範囲でこれを実現す
  る。

### pending_fields による deferred field access

spec 04 の closed records では `\s -> s.grade == g` のような
lambda を単独で type-check するとき `s` の type が決まらない。この
ため field access は：

1. その場では fresh result type を返し、`(record_ty, field, result_ty, span)`
   を `InferCtx::pending_fields` に積む。
2. `check_value_binding` の終盤で、pending の record_ty が record
   に具体化しているかを繰り返し調べ、できたら field lookup + unify。
3. 1 ラウンドで進まない pending が残ったら "add a type annotation"
   エラー。

ラウンド反復なので、1 field access の解決がさらに別 pending を具
体化する連鎖にも対応する。

### type alias

spec 09 の transparent semantics を直訳：`ty_from_ast` で alias 名
を見つけたら、ai.params と実引数で substitute して返す。ここで
alias の RHS が再度 alias を含んでいても (ast 側で展開されていな
いため) `ty_from_ast` を通るので再帰的に expand される。

### constructor pattern

`bind_pattern(Pattern::Con { name, args })`:

1. ctor の scheme を `instantiate` して得たモノタイプをパラメータ
   `τ₁ -> τ₂ -> ... -> T ...` として `split_fun` で分解。
2. 最後の `T ...` を expected_ty と unify。
3. 各 arg pattern を対応する τᵢ に対して再帰 bind。

arity 不一致は `CtorArity` エラー。

## エラー報告

- `CtorArity { ctor, expected, found }`。
- `RecordPatternField { field, ty }`: record pattern が指定した
  field が record の型に存在しない。
- `MissingField { field, ty }`: field access / update が指定した
  field が record に存在しない。
- `UnknownType`: data / alias / 組み込み型に見つからない。

## 代替検討と却下理由

- **row polymorphism** (Ur/Web, PureScript など): closed records が
  spec で決まっている以上採らない。pending_fields は "構造を決め
  打たずに後で具体化されるのを待つ" 程度の軽量代替。
- **named-field ctor**: 04-OQ2 DECIDED (位置引数のみ)。よってレコー
  ド ctor は実装しない。

## I7 への引き渡し

- ctor 名 → `CtorInfo { type_name, scheme, arity }` が引ける。
- record 型は Ty::Record の fields 順に codegen が Hash に map
  すれば良い (field 名は UTF-8 string)。
- alias は I6b 時点で完全展開される。I7 は alias の存在を知らな
  くて済む。
