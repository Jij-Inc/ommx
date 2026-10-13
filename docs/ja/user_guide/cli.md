# OMMX CLI のインストールと実行

`ommx` コマンドでは、Artifact の内容確認・入出力、Local Registry の管理、
同梱のエージェントプラグインやスキルの取得ができます。単体の実行ファイルとして
インストールする方法と、OMMX Python パッケージに付属する CLI を使う方法があります。

CLI を含む Python SDK がすでに uv プロジェクトに導入されている場合は、
プロジェクトのルートで次のように実行できます。

```bash
uv run ommx --help
```

## cargo-binstall で単体の実行ファイルをインストールする

[cargo-binstall](https://github.com/cargo-bins/cargo-binstall#installation) を
インストールし、[Rust SDK のリリース](https://github.com/Jij-Inc/ommx/releases?q=rust-3.0)
から利用する v3 のバージョンを選びます。例えば、公開済みの `3.0.0-beta.6` には
CLI のバイナリが添付されています。

```bash
cargo binstall ommx@3.0.0-beta.6
ommx --help
```

cargo-binstall はリリースのメタデータに従って、利用環境に対応するアーカイブを
選択します。単体の実行ファイルは Python 環境がなくても利用できます。

最新の安定版を選ぶ場合は `cargo binstall ommx` を使います。v3 のプレリリースを
利用する間は、v3 のバージョンを明示してください。バージョンを省略すると安定版の
v2 系列が選ばれます。利用したいコマンドを含むリリースを選んでください。
このドキュメントで説明するエージェントプラグイン・スキルのコマンドは
`3.0.0-beta.6` より後の機能です。提供状況は
[Rust SDK のリリースノート](https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/release_note/3.0.md)
と [Python SDK のリリースノート](../release_note/ommx-3.0.md) を参照してください。

## Python パッケージに付属する CLI を使う

Python の `ommx` パッケージには、`3.0.0b8` から `ommx` コマンドが付属します。
uv プロジェクトにパッケージを追加すると、プロジェクトの環境で CLI を実行できます。

```bash
uv add 'ommx>=3.0.0b8'
uv run ommx --help
```

プロジェクトの依存関係で選ばれた OMMX のバージョンが使われます。
対応する `ommx` がすでに依存関係に含まれている場合は、CLI を別途インストールする
必要はありません。同じ CLI バージョンであれば、コマンド構文も同じです。
利用できるコマンドは、インストールしたバージョンによって異なります。

## バージョンとコマンドを確認する

```bash
uv run ommx --version
uv run ommx --help
uv run ommx inspect --help
```

単体の実行ファイルを使う場合は、`uv run ommx` を `ommx` に置き換えます。
OMMX の Rust SDK と Python SDK のバージョンは独立して管理されているため、
Python 経由で起動した場合も、`ommx --version` は Rust CLI のバージョンを表示します。

同梱プラグインや個別のスキルを取得して導入する手順は、
[OMMX のエージェントプラグインとスキルを取得・導入する](agent_skills.md) を
参照してください。ref の削除・復元・ガベージコレクションは
{ref}`Local Registry の cleanup <experiment-cleanup-workflow>` を参照してください。
