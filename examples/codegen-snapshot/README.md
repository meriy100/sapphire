# codegen-snapshot/

各 M9 例題を I7 codegen に通した結果の Ruby 出力を保存する。
コード生成器の変更を目視で確認するためのリグレッション・ス
ナップショットであり、コードベースがこれを **契約** として扱
うわけではない（契約は spec 10 §Generated Ruby module shape と
`docs/build/02-source-and-output-layout.md`、そして実行結果
`tests/codegen_m9.rs`）。

## ファイル

| ファイル | 元 | 説明 |
|---|---|---|
| `prelude.rb` | — | codegen が毎 build で emit する `Sapphire::Prelude`。どの例題でも同じ内容 |
| `01-hello-ruby-main.rb` | `examples/sources/01-hello-ruby/Main.sp` | `rubyPuts` 2 回の例題 |
| `02-parse-numbers-number_sum.rb` | `examples/sources/02-parse-numbers/NumberSum.sp` | ファイル読み込み + Result monad |
| `03-students-records-students.rb` | `examples/sources/03-students-records/Students.sp` | レコード + 高階関数（`main` なし） |
| `04-fetch-summarise-fetch.rb` + `04-fetch-summarise-http.rb` | `examples/sources/04-fetch-summarise/` | HTTP 取得（Ruby snippet） + ADT |

## 再生成

`sapphire build` で再生成できる：

```sh
sapphire build examples/sources/01-hello-ruby/Main.sp --out-dir /tmp/snap
cp /tmp/snap/sapphire/main.rb examples/codegen-snapshot/01-hello-ruby-main.rb
```

または一括で：

```sh
sapphire build examples/sources/01-hello-ruby/Main.sp --out-dir /tmp/snap01
sapphire build examples/sources/02-parse-numbers/NumberSum.sp --out-dir /tmp/snap02
sapphire build examples/sources/03-students-records/Students.sp --out-dir /tmp/snap03
sapphire build examples/sources/04-fetch-summarise --out-dir /tmp/snap04
```

生成された `.rb` は

```sh
ruby -I runtime/lib -I /tmp/snap01 \
  -e "require 'sapphire/main'; exit Sapphire::Main.run_main"
```

のように実行できる。例題ごとの実行手順は `examples/README.md`
（本 task では触らない）。

## 実行結果の期待値

| 例題 | 実行結果 | 備考 |
|---|---|---|
| 01 | `Hello, Sapphire!\nHello, world!\n` | 単純 |
| 02 | `<sum>\n` | `numbers.txt` の各行の整数合計（`current_dir` に置いて実行） |
| 03 | ライブラリ（`main` なし） | `topScorersByGrade` を Ruby から直接呼べる |
| 04 | `fetched N bytes\n` | ネットワーク到達 or `Net::HTTP.get_response` をモック |

`tests/codegen_m9.rs` が上記をそれぞれ end-to-end で検証する。
