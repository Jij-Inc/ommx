# OMMX CLI のインストールと実行

`ommx` コマンドでは、Artifact の確認・受け渡しや、同梱のエージェントプラグイン・
スキルの取得ができます。Python SDK 2.9.4 以降には CLI が含まれます。
uv プロジェクトで v2 のパッケージを導入し、実行します。

```bash
uv add 'ommx>=2.9.4,<3'
uv run ommx --help
```

```{note}
CLI とエージェント関連コマンドは Python SDK 2.9.4 に向けて開発中です。
公開前に使う場合は、上記の `uv add` の代わりに `dev-2.x` のソースチェックアウトの
ルートで `uv sync` を実行してください。
```

## 単体の実行ファイルをインストールする

Rust の単体実行ファイルは Python 環境なしで使えます。次の Rust SDK 2.x リリース
より前にエージェント関連コマンドを使うには、`dev-2.x` からビルドします。

```bash
cargo install --git https://github.com/Jij-Inc/ommx --branch dev-2.x --locked ommx
ommx --help
```

これらのコマンドを含む Rust SDK 2.x が公開されたら、
`cargo install ommx --version '<version>' --locked` でそのバージョンを
crates.io から導入できます。v2 CLI を使い続けるには、2.x のバージョンを明示します。

## バージョンと利用可能なコマンドを確認する

```bash
uv run ommx --version
uv run ommx --help
uv run ommx inspect --help
```

単体の実行ファイルを導入した場合は、`uv run ommx` を `ommx` に置き換えます。
Rust SDK と Python SDK のバージョンは独立して管理しており、`ommx --version` は
Python 経由でも Rust CLI のバージョンを表示します。
v2 の Artifact 関連コマンドには `load`、`save`、`list`、`inspect`、`push`、`pull`
があります。引数は各コマンドの `--help` を参照してください。

同梱のプラグインやスキルの取得・導入は、
[OMMX のエージェントプラグインとスキルを取得・導入する](agent_skills.md)
に進んでください。
