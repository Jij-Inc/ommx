# OMMX のエージェントプラグインとスキルを取得・導入する

<!-- 同梱 OMMX スキルの本文を実装したら、この注記を削除する。 -->
```{note}
現在同梱されている OMMX スキルはモックで、OMMX の利用方法を説明する本文は
含まれていません。
```

インストールした OMMX に付属するプラグインやスキルを取得し、
コーディングエージェントに導入します。まず
[OMMX CLI のインストールと実行](../how_to/manage_local_registry.md) に従って、`ommx` コマンドを利用できる
状態にしてください。以下では Python パッケージに付属する CLI を `uv run` で
実行します。cargo-binstall で単体の実行ファイルをインストールした場合は、
`uv run ommx` を `ommx` に置き換えます。

以下のシェル例は POSIX 系シェルを想定しています。

## プラグイン・マーケットプレイス・スキルのパスを取得する

```bash
uv run ommx plugin path
uv run ommx plugin marketplace path
uv run ommx skill path
```

いずれも絶対ディレクトリパスを標準出力に 1 行で表示します。

| コマンド | ディレクトリの内容 |
| --- | --- |
| `plugin path` | `plugin.json`、`.claude-plugin/plugin.json`、`skills/` |
| `plugin marketplace path` | `.claude-plugin/marketplace.json`、`plugins/ommx/` |
| `skill path` | `ommx/SKILL.md` |

プラグイン名・スキル名は省略可能な位置引数です。

```bash
uv run ommx plugin path ommx
uv run ommx skill path ommx
```

`plugin path ommx` は `plugin path` と同じプラグインを返します。
`skill path ommx` は親ディレクトリではなく個別のスキルディレクトリを返します。
同梱名は `ommx` です。

## プロジェクトへのプラグイン導入

同梱内容には [Agent Plugins 1.0](https://agent-plugins.org/specification) の manifest
と Claude Code 用メタデータが含まれます。Claude Code のローカルマーケットプレイス
経由で `local` スコープに導入するには、次のように実行します。

```bash
claude plugin marketplace add --scope local "$(uv run ommx plugin marketplace path)"
claude plugin install --scope local ommx@ommx
```

`local` スコープでは、端末固有のマーケットプレイスのパスとプラグインの有効化設定を
`.claude/settings.local.json` に保存し、このプロジェクトで自分だけが利用します。

Claude Code の 1 セッションだけで使う場合は、起動時にプラグインのパスを指定します。

```bash
claude --plugin-dir "$(uv run ommx plugin path)"
```

Cursor CLI でも、1 セッションの起動時にプラグインを指定できます。

```bash
agent --plugin-dir "$(uv run ommx plugin path)"
```

新しいセッションで、設定したプラグインから OMMX スキルが検出されることを確認します。
Claude Code では `/ommx:ommx` という名前になります。マーケットプレイスの形式と
導入時の動作は [Claude Code のドキュメント](https://code.claude.com/docs/en/plugin-marketplaces)
を参照してください。

## スキル単体の導入

`.agents/skills` を検出するエージェントでは、プロジェクトのルートでリンクを作成します。

```bash
mkdir -p .agents/skills
ln -s "$(uv run ommx skill path ommx)" .agents/skills/ommx
```

新しいセッションを開始し、`ommx` がスキル一覧に表示され、読み込み元がこのプロジェクトの
`SKILL.md` であることを確認します。スキルディレクトリ全体をコピーすることもできます。

```bash
mkdir -p .agents/skills
cp -R "$(uv run ommx skill path ommx)" .agents/skills/ommx
```

既存の `.agents/skills/ommx` ディレクトリがない状態で実行してください。
親ディレクトリを受け取るスキルマネージャーには `skill path` の出力を渡せます。
例えば次のように実行します。

```bash
gh skill install "$(uv run ommx skill path)" ommx --from-local --scope project
```

選択を求められたら利用するエージェントを選び、新しいセッションでプロジェクトスコープと
読み込み元のパスを確認します。環境固有のリンクやコピーには、`.gitignore` に
`.agents/skills/ommx` を追加してください。チームでコピーを Git 共有する場合は
OMMX 依存バージョンと合わせて更新します。

## バージョン・キャッシュ・更新

プラグインの manifest は、Python 経由で起動した場合も、`ommx --version` で表示される
Rust CLI と同じバージョンになります。OMMX の Rust SDK と Python SDK のバージョンは
独立して管理されています。

OMMX は bundle 全体を Rust 実行ファイルと Python wheel に埋め込みます。パス取得
コマンドは、manifest のバージョンを含む全ファイルの内容から識別したサブディレクトリへ、
OS の OMMX キャッシュ内に展開します。ソースのチェックアウトやネットワークアクセスは
不要で、Artifact の Local Registry も開きません。配置先を選ぶ場合は
`OMMX_PLUGIN_CACHE_DIR` に、改行を含まない UTF-8 のパスで書き込み可能なディレクトリを
指定します。キャッシュ内のファイルは生成物であり、ファイルの削除や内容の変更は
次の実行時に復元されます。

OMMX を更新したらパスを再取得します。リンク先が変わった場合はシンボリックリンクを
作り直し、コピーの場合はスキルディレクトリ全体を置き換えてください。Claude Code の
ローカルマーケットプレイスでは、`local` スコープの登録を解除して新しいパスを登録し、
プラグインを再インストールします。Rust CLI のバージョンが変わらない場合も、この手順で
新しい内容を取り込めます。

```bash
claude plugin marketplace remove --scope local ommx
claude plugin marketplace add --scope local "$(uv run ommx plugin marketplace path)"
claude plugin install --scope local ommx@ommx
```

`--plugin-dir` の場合は新しいパスでセッションを起動します。エージェントが更新後の
スキルを読み込んでいることを確認します。キャッシュを削除した場合は、リンクや
マーケットプレイス設定を使う前にパス取得コマンドを再実行して bundle を復元してください。
