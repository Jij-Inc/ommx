# 保存したArtifactを調べ、Local Registryを管理する

Artifactは、InstanceやSolutionなどをまとめて保存・受け渡しするためのデータです。
ここでは`ommx`コマンドで内容を調べ、OMMXのローカル保存領域（Local Registry）を整理します。
求解やAdapter比較のために、この操作を覚える必要はありません。

まず{ref}`CLIの導入手順 <install-cli>`を確認してください。以降はSDKに付属するCLIを
SDKを導入した作業ディレクトリから`uv run --no-sync ommx`で呼び出します。
単体CLIを導入した場合は`uv run --no-sync ommx`を`ommx`に置き換えます。
利用できるコマンドやオプションは`uv run --no-sync ommx --help`と各サブコマンドの`--help`で確認できます。

## ファイルや保存済みの名前を調べる

受け取った`.ommx`ファイルのManifest（収録データを示す一覧）を見るには次を実行します。
これはInstanceやSolutionを評価する操作ではありません。内容の取り出し方は
{class}`~ommx.artifact.Artifact`のAPI Referenceを参照してください。

```sh
uv run --no-sync ommx inspect shared.ommx
uv run --no-sync ommx list
```

`list`は現在のLocal Registry内の名前を一覧します。名前が分かれば、その参照先や
論理サイズを調べられます。以下の名前は自分が保存した名前に置き換えてください。

```sh
uv run --no-sync ommx inspect example.com/team/result:trial-1
uv run --no-sync ommx size example.com/team/result:trial-1
```

末尾の`trial-1`はタグです。名前やタグは別の内容を指すように更新できますが、
Manifestのdigestは内容の一覧から計算される識別子です。複数の名前が同じデータを参照する
こともあります。`size`はManifest自身、config、重複を除いたlayerの論理サイズを示し、
親Artifactへのsubjectは含めません。別々の名前の共有データはそれぞれ数えられるため、
`size`の値を足しても実ディスク使用量にはなりません。
詳しくは{class}`Artifactの保存と識別 <ommx.artifact.Artifact>`を参照してください。

## Pythonから記録を探す

{func}`~ommx.artifact.list_artifacts`は汎用ArtifactとExperimentを一覧します。
Experiment専用の記録や、プロジェクトのannotationを使った絞り込みの完全な例は
{func}`~ommx.experiment.list_experiments`を参照してください。
Experiment全体の検索用メタデータと、試行ごとに記録する条件の使い分けも説明します。
いずれもソルバーは必要ありません。

中断した編集を探して復帰する場合は{func}`~ommx.experiment.list_experiment_checkpoints`と
{meth}`~ommx.experiment.Experiment.restore_from_checkpoint`を使います。
調べた内容を正確に特定するにはmanifest digestも残します。一覧にある名前は後から
別の内容を指すことがあるためです。

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
uv run --no-sync ommx prune-anonymous --root ./local-registry --experiments --older-than 7d
uv run --no-sync ommx gc --root ./local-registry
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
uv run --no-sync ommx rm example.com/team/experiment:obsolete --root ./local-registry
```

出力の`Rollback`行には、削除した名前と、その内容を識別するManifest digestを含む
`ommx restore-ref ...`コマンドが表示されます。復元するときは、そのコマンドを
実行してください。Pythonに付属するCLIを使う場合は先頭に`uv run --no-sync`を付けます。
`--root`も出力に含まれるため、同じ保存先へ復元できます。
同じ名前が既に別の内容を指している場合は、上書きせずエラーになります。

復元には、削除時のデータ本体が残っている必要があります。
必要な復元は、次のデータ回収より先に行ってください。

### 確認した対象を整理する

匿名の名前を一括削除する場合は、先ほどの報告を確認してから`--delete`を付けます。
削除した名前ごとに復元用コマンドが表示されます。

```bash
uv run --no-sync ommx prune-anonymous --root ./local-registry --experiments --older-than 7d --delete
```

データ本体の回収は、別途`gc --delete`で行います。

```bash
uv run --no-sync ommx gc --root ./local-registry --delete
```

残る名前やExperimentのcheckpoint、残した子から参照される親Artifactのデータは回収しません。
名前を公開する前に中断した書き込みのデータは、参照されていない場合があります。
参照されていないデータのうち、データ本体の最終更新から既定で24時間の
猶予期間（grace period）が経過したものを回収します。名前を削除してから24時間の
復元を保証するものではなく、古いデータは削除直後のGCでも回収される可能性があります。
**回収後は復元できない場合があります。** 書き込み中のデータを保護するため、
通常は猶予期間を短くせず、`ommx gc --help`で条件を確認してください。

Pythonから同じ操作を行う場合は、{py:func}`~ommx.artifact.remove_image`、
{py:func}`~ommx.artifact.restore_image`、{py:func}`~ommx.artifact.prune_anonymous`、
{py:func}`~ommx.artifact.gc`を使えます。引数や戻り値の詳細はAPI Referenceを参照してください。

調査のために猶予期間を指定する場合、`--grace-period`は`s`・`m`・`h`・`d`を受け付けます。
`0s`はその保存先に書き込むOMMXプロセスがない場合に限ります。通常の報告は件数と容量を示し、
`--show-digests`を付けると欠損・不正・回収候補・猶予中のデータの識別子を確認できます。

```sh
uv run --no-sync ommx gc --root ./local-registry --grace-period 2h --show-digests
```

CAS・index・cacheの実装は[Rust Local Registryの資料](https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/src/artifact/local_registry/mod.rs)に分けています。
