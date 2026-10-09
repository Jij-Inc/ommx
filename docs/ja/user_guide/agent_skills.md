# コーディングエージェントで OMMX を使う

OMMX は、Python SDK、ソルバー Adapter、Artifact、Experiment を扱うエージェント
プラグインとスキルを同梱しています。プロジェクトにインストールした OMMX から、
[JijModeling](https://jij-inc-jijmodeling-tutorials-ja.readthedocs-hosted.com/ja/latest/advanced/agent_plugin_installation.html)
と同じコマンド構成で取得できます。

`.agents/skills` を検出するエージェントでは、プロジェクトのルートでリンクを作成します。

```bash
mkdir -p .agents/skills
ln -s "$(uv run ommx skill path)/ommx" .agents/skills/ommx
```

新しいセッションを開始し、`ommx` がスキル一覧に表示され、読み込み元がこのプロジェクトの
`SKILL.md` であることを確認します。以下のシェル例は POSIX 系シェルを想定しています。

## プラグイン・マーケットプレイス・スキルのパス確認

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

JijModeling と同様に、プラグイン名・スキル名は省略可能な位置引数です。

```bash
uv run ommx plugin path ommx
uv run ommx skill path ommx
```

`plugin path ommx` は `plugin path` と同じプラグインを返します。
`skill path ommx` は親ディレクトリではなく個別のスキルディレクトリを返します。
同梱名は `ommx` です。

Rust の単体実行ファイルでも同じコマンドが使えます。Python モジュールとしても
`uv run python -m ommx.cli plugin path` で実行できます。

## プロジェクトへのプラグイン導入

同梱内容には [Agent Plugins 1.0](https://agent-plugins.org/specification) の manifest
と Claude Code 用メタデータが含まれます。Claude Code のローカルマーケットプレイス
経由でプロジェクトスコープに導入するには、次のように実行します。

```bash
claude plugin marketplace add --scope project "$(uv run ommx plugin marketplace path)"
claude plugin install --scope project ommx@ommx
```

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

冒頭の `.agents/skills/ommx` へのリンクを使うか、スキルディレクトリ全体をコピーします。

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
OMMX 依存バージョンと合わせて更新します。Claude Code のプロジェクト設定にも
ローカルマーケットプレイスの絶対パスが入るため、このローカルパスを共有設定に含めないようにします。

## バージョン・キャッシュ・更新

プラグインの manifest は、Python 経由で起動した場合も、`ommx --version` で表示される
Rust CLI と同じバージョンになります。OMMX の Rust SDK と Python SDK のバージョンは
独立して管理されています。

OMMX は bundle 全体を Rust 実行ファイルと Python wheel に埋め込みます。パス取得
コマンドは、manifest のバージョンを含む全ファイルの内容から識別したサブディレクトリへ、
OS の OMMX キャッシュ内に展開します。ソースのチェックアウトやネットワークアクセスは
不要で、Artifact の Local Registry も開きません。配置先を選ぶ場合は
`OMMX_PLUGIN_CACHE_DIR` に書き込み可能なディレクトリを指定します。キャッシュ内の
ファイルは生成物であり、削除・変更されたファイルは次の実行時に復元されます。

OMMX を更新したらパスを再取得します。リンク先が変わった場合はシンボリックリンクを
作り直し、コピーの場合はスキルディレクトリ全体を置き換えてください。Claude Code の
ローカルマーケットプレイスでは、新しいパスを登録してからプラグインを更新します。

```bash
claude plugin marketplace add --scope project "$(uv run ommx plugin marketplace path)"
claude plugin update --scope project ommx@ommx
```

`--plugin-dir` の場合は新しいパスでセッションを起動します。エージェントが更新後の
スキルを読み込んでいることを確認します。キャッシュを削除した場合は、リンクや
マーケットプレイス設定を使う前にパス取得コマンドを再実行して bundle を復元してください。
