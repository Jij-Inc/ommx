# 保存したArtifactを調べ、Local Registryを管理する

`ommx` コマンドでは、Artifact の内容確認・入出力、Local Registry の管理、
同梱のエージェントプラグインやスキルの取得ができます。単体の実行ファイルとして
インストールする方法と、OMMX Python パッケージに付属する CLI を使う方法があります。

公開Python SDK `3.0.0b7`にはCLIは同梱されません。b7の環境は変更せず、
まず下の単体CLIを導入してください。CLIを含む開発版Pythonパッケージの使い方は
後半で分けて説明します。

## CLIを用意する

このページの操作は任意です。求解・候補評価・変換・Adapter比較にはCLIは不要です。
Pythonの例で使うSDK `3.0.0b7`を変更せず、別にRust CLIを導入できます。

### cargo-binstallで単体の実行ファイルをインストールする

[cargo-binstall](https://github.com/cargo-bins/cargo-binstall#installation) を
インストールし、[Rust SDK のリリース](https://github.com/Jij-Inc/ommx/releases?q=rust-3.0)
から利用する v3 のバージョンを選びます。公開済みの `3.0.0-beta.7`には
CLIのバイナリが添付されています。

```bash
cargo binstall ommx@3.0.0-beta.7
ommx --help
```

cargo-binstall はリリースのメタデータに従って、利用環境に対応するアーカイブを
選択します。単体の実行ファイルは Python 環境がなくても利用できます。

最新の安定版を選ぶ場合は `cargo binstall ommx` を使います。v3 のプレリリースを
利用する間は、v3 のバージョンを明示してください。バージョンを省略すると安定版の
v2 系列が選ばれます。利用したいコマンドを含むリリースを選んでください。
補助ツールの提供状況は[エージェントプラグインとスキル](../user_guide/agent_skills.md)を
参照してください。このページではArtifactの管理だけを扱います。

### 開発版Pythonパッケージに付属するCLI

Python の `ommx` パッケージには、`3.0.0b8` から `ommx` コマンドが付属します。
uv プロジェクトにパッケージを追加すると、プロジェクトの環境で CLI を実行できます。

```bash
uv add 'ommx==3.0.0b8'
uv run ommx --help
```

`3.0.0b8`はこのBookの公開b7実行例の基準ではありません。公開されていない場合は
上の単体CLIを使ってください。b8の開発版を利用する場合は別の環境を用意し、
既存のb7環境をこの手順のためだけに更新する必要はありません。
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

## ファイルや保存済みの名前を調べる

受け取った`.ommx`ファイルのManifest（収録データを示す一覧）を見るには次を実行します。
これはInstanceやSolutionを評価する操作ではありません。内容の取り出し方は
[Artifact Tutorial](../tutorial/share_in_ommx_artifact.md)を参照してください。

```sh
ommx inspect shared.ommx
ommx list
```

`list`は現在のLocal Registry内の名前を一覧します。名前が分かれば、その参照先や
論理サイズを調べられます。以下の名前は自分が保存した名前に置き換えてください。

```sh
ommx inspect example.com/team/result:trial-1
ommx size example.com/team/result:trial-1
```

複数の名前が同じデータを参照することがあります。`size`の値を足しても実ディスク使用量には
なりません。[名前とdigestの違い](../user_guide/artifact.md)を踏まえて読みます。

## Pythonから絞り込んで探す

公開SDK b7の環境で次を実行できます。空のRegistryでは結果も空です。

```python
from ommx.artifact import list_artifacts

for record in list_artifacts("example.com/team"):
    print(record.image_name, record.manifest_digest, record.annotations)
```

Experimentだけを探す場合は{py:func}`~ommx.experiment.list_experiments`を使います。
検索したい全体条件は作成時に`experiment.set_annotation("com.example.study", "trial")`
のように記録します。`org.ommx.*`は予約されているため使いません。Runごとに異なる条件は
`run.log_parameter()`へ記録します。

```python
from ommx.experiment import Experiment, list_experiments

for record in list_experiments("example.com/team"):
    if record.annotations.get("com.example.study") == "trial":
        loaded = Experiment.load(str(record.image_name))
        print(loaded.run_parameters_df())
```

検索結果の名前は後から別の内容を指すことがあります。調査した正確な内容の識別には
`manifest_digest`も残してください。中断した作業の復帰は
[継続・checkpoint・forkのHow-to](continue_experiment.md)で扱います。

(experiment-cleanup-workflow)=
## 保存した記録を削除・復元し、保存領域を整理する

Local Registry はOMMXのローカル保存領域です。ArtifactやExperimentには、
`example.com/team/experiment:obsolete`のような名前（ref）を付けられます。
**名前を削除する操作と、データ本体を回収して容量を空ける操作は別です。**

以下の`./local-registry`は、実際に使っている保存領域のパスに置き換えてください。
各コマンドの`--root`で対象を明示します。省略すると`OMMX_LOCAL_REGISTRY_ROOT`、
それも未設定ならOS既定の保存先を使います。

### まず対象を確認する

```bash
ommx prune-anonymous --root ./local-registry --experiments --older-than 7d
ommx gc --root ./local-registry
```

どちらも既定では報告のみで、名前やデータを削除しません。
`prune-anonymous`は名前を自動生成した匿名Artifactを対象にし、`--experiments`で
匿名Experimentも含めます。`--older-than 7d`は7日以上経過した名前に絞る指定です。
`gc`（ガベージコレクション）は、どの名前からも参照されなくなったデータ本体の
回収候補を表示します。

### 一つの名前を削除・復元する

名前を確認し、例示名を実際の削除対象に置き換えてから実行します。
**`rm`は直ちに名前を削除します。** データ本体はこの操作では削除しません。

```bash
ommx rm example.com/team/experiment:obsolete --root ./local-registry
```

出力の`Rollback`行には、削除した名前と、その内容を識別するManifest digestを含む
`ommx restore-ref ...`コマンドが表示されます。復元するときは、そのコマンドを
実行してください。Pythonに付属するCLIを使う場合は先頭に`uv run`を付けます。
`--root`も出力に含まれるため、同じ保存先へ復元できます。
同じ名前が既に別の内容を指している場合は、上書きせずエラーになります。

復元には、削除時のデータ本体が残っている必要があります。
必要な復元は、次のデータ回収より先に行ってください。

### 確認した対象を整理する

匿名の名前を一括削除する場合は、先ほどの報告を確認してから`--delete`を付けます。
削除した名前ごとに復元用コマンドが表示されます。

```bash
ommx prune-anonymous --root ./local-registry --experiments --older-than 7d --delete
```

データ本体の回収は、別途`gc --delete`で行います。

```bash
ommx gc --root ./local-registry --delete
```

残る名前やExperimentのcheckpointから参照されているデータは回収しません。
参照されていないデータのうち、データ本体の最終更新から既定で24時間の
猶予期間（grace period）が経過したものを回収します。名前を削除してから24時間の
復元を保証するものではなく、古いデータは削除直後のGCでも回収される可能性があります。
**回収後は復元できない場合があります。** 書き込み中のデータを保護するため、
通常は猶予期間を短くせず、`ommx gc --help`で条件を確認してください。

Pythonから同じ操作を行う場合は、{py:func}`~ommx.artifact.remove_image`、
{py:func}`~ommx.artifact.restore_image`、{py:func}`~ommx.artifact.prune_anonymous`、
{py:func}`~ommx.artifact.gc`を使えます。引数や戻り値の詳細はAPI Referenceを参照してください。
