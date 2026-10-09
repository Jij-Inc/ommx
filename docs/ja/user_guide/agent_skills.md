# コーディングエージェントで OMMX を使う

OMMX は、Python SDK、ソルバー Adapter、Artifact、Experiment を扱うコーディング
エージェント向けのスキルを同梱しています。プロジェクトにインストールした OMMX
からスキルを取得すると、利用する SDK に対応する説明を参照できます。

プロジェクトのルートで、スキルへのリンクをプロジェクトのスキル配置先に作成します。

```bash
mkdir -p .agents/skills
ln -s "$(uv run ommx skill path --name ommx)" .agents/skills/ommx
```

`.agents/skills` を検出するエージェントで新しいセッションを開始し、スキル一覧に
`ommx` が表示され、読み込み元がこのプロジェクトの `SKILL.md` であることを確認します。
リンクが環境固有の場合は、プロジェクトの `.gitignore` に `.agents/skills/ommx` を
追加してください。以下のシェル例は POSIX 系シェルを想定しています。

## スキルのパス確認とコピー

```bash
uv run ommx skill path
uv run ommx skill path --name ommx
```

いずれも絶対ディレクトリパスを標準出力に 1 行で表示します。前者は
`ommx/SKILL.md` を含む親ディレクトリ、`--name ommx` を指定した後者は `ommx`
ディレクトリ自体を返します。同梱スキル名として指定できるのは `ommx` のみです。

シンボリックリンクの代わりにコピーする場合は、次のように配置します。

```bash
mkdir -p .agents/skills
cp -R "$(uv run ommx skill path --name ommx)" .agents/skills/ommx
```

既存の `.agents/skills/ommx` ディレクトリがない状態で実行してください。コピーを
Git で共有する場合は、プロジェクトの OMMX 依存バージョンと合わせて更新します。
親ディレクトリを受け取る外部スキルマネージャーには、
[JijModeling の導入手順](https://jij-inc-jijmodeling-tutorials-ja.readthedocs-hosted.com/ja/latest/advanced/agent_plugin_installation.html)
と同様に `ommx skill path` の出力を渡せます。

Rust の単体実行ファイルでも、`ommx skill path` および
`ommx skill path --name ommx` が使えます。Python モジュールとしても
`uv run python -m ommx.cli skill path` で実行できます。

## キャッシュと更新

スキルは Python wheel を含め CLI に埋め込まれています。`skill path` は、OS の
OMMX キャッシュディレクトリ内に、スキルの内容を識別するサブディレクトリを作り、
スキルを展開します。ネットワークアクセスやソースのチェックアウトは不要で、
Artifact の Local Registry も開きません。別の書き込み可能なキャッシュ配置先を
使う場合は `OMMX_SKILL_CACHE_DIR` を設定します。キャッシュ内のファイルは生成物
として扱ってください。削除・変更された `SKILL.md` は次の実行時に復元されます。

OMMX を更新したら、再度 `skill path` を実行します。出力パスが変わった場合は、
プロジェクトのシンボリックリンクを作り直してください。コピーの場合はスキルの
ディレクトリ全体を置き換えます。その後、新しいエージェントセッションで読み込み元の
パスを確認します。キャッシュを削除するとリンク先もなくなるため、コマンドを再実行して
復元してください。同梱スキルの内容が異なる SDK は、それぞれ別のキャッシュパスを使います。
