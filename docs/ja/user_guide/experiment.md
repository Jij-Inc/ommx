# Experiment の検索・復帰・cleanup

{mod}`ommx.experiment` は、最適化の試行錯誤を 1 つの OMMX Artifact として記録するための API です。Experiment のデータモデル、実行できる logging 例、共有、確認、fork については [実験を記録して共有する](../tutorial/experiment_management.md) を参照してください。

このページでは、commit 済みまたは中断された Experiment を Local Registry で扱う workflow を説明します。プロジェクト固有 metadata から目的の Experiment を検索する方法、checkpoint からの復帰、Local Registry cleanup が削除できる blob の判断を扱います。

## Manifest blob を開かずに Artifact catalog を確認する

{py:func}`ommx.artifact.list_artifacts` は、汎用 Artifact と Experiment を含む、
一致したすべての OMMX Artifact ref を一覧します。各 record には image name、
Manifest/Config digest、更新時刻、`artifactType`、Manifest annotation、完全な OCI
Manifest が Python の dictionary として含まれます。

```python
from ommx.artifact import list_artifacts

refs = list_artifacts("example.com/optimization")
for ref in refs:
    print(ref.image_name, ref.artifact_type, ref.annotations)
```

Local Registry は SQLite の `refs` と digest-addressed Manifest cache を JOIN して
これらの record を読み出します。Manifest row がない場合、最初の一覧取得時に
content-addressed blob store から読み出して検証し、cache を backfill します。以降の
一覧取得では Manifest blob を開かず、同じ immutable な Manifest JSON を SQLite
から返します。任意の `prefix` を指定すると、backfill と返却対象の両方を registry
namespace または full image reference の途中までに限定できます。

rolling Experiment checkpoint などの Local Registry 内部 ref は、default では一覧に
含まれません。`list_artifacts(..., include_internal=True)` は内部 ref を調査するための
diagnostic escape hatch です。復帰 workflow では、後述する checkpoint 専用 API を
使います。

一覧用 cache の source of truth は CAS です。cache 内の Manifest が不正な場合、default
の一覧取得は CAS から修復し、`RuntimeWarning` を出します。CAS blob も利用できない
場合は warning を出してその ref を除外し、残りの record を返します。診断や検証で
最初の不正 ref を error にしたい場合は `strict=True` を指定します。database schema
全体、query、SQLite cache write の failure は、常に一覧全体の error になります。

Experiment のみを対象にし、status、run/solve 数、完全な Experiment Config も必要な
場合は {py:func}`~ommx.experiment.list_experiments` を使います。

## Annotation で Experiment を一覧・filter する

継続的に QAP の solver comparison を行うチームを考えます。1 つの commit 済み
Experiment は、特定の問題 instance、solver、formulation、source revision に対する
1 batch を表します。image name はすべての batch を同じ registry namespace にまとめ、
manifest annotation はプロジェクトが後から検索したい軸を表します。

プロジェクト固有の field は reverse-DNS 形式の annotation key で定義し、Experiment
を commit する前に設定します。`org.ommx.*` 以下の key は OMMX が予約しており、
annotation value は文字列です。

```python
from ommx.experiment import Experiment

image_name = "example.com/optimization/qap-experiments:tai20a-highs-20260710"

with Experiment(image_name) as experiment:
    experiment.set_annotation("com.example.study", "qap-solver-comparison")
    experiment.set_annotation("com.example.instance", "tai20a")
    experiment.set_annotation("com.example.solver", "highs")
    experiment.set_annotation("com.example.formulation", "assignment")
    experiment.set_annotation("com.example.git-revision", "a1b2c3d")

    with experiment.run() as run:
        run.log_parameter("seed", 42)
        run.log_parameter("time_limit_seconds", 300)
```

annotation は Artifact 全体で共有される Experiment-level の catalog field に使います。
この例の seed や time limit のように Run ごとに変わる値は、代わりに
{py:meth}`~ommx.experiment.Run.log_parameter` へ記録します。

後から registry namespace を一覧し、プロジェクトが定義した annotation schema を
通常の DataFrame column へ投影します。`list_artifacts` と同じ Manifest cache を基礎に、
{py:func}`~ommx.experiment.list_experiments` は Experiment Config cache も JOIN します。
一致する各 Experiment ref について、image name、immutable な Manifest/Config digest、
更新時刻、status、run/solve 数、Manifest annotation、完全な Experiment Config を返します。

```python
import pandas as pd

from ommx.experiment import Experiment, list_experiments

annotation_columns = {
    "study": "com.example.study",
    "instance": "com.example.instance",
    "solver": "com.example.solver",
    "formulation": "com.example.formulation",
    "git_revision": "com.example.git-revision",
}

refs = list_experiments("example.com/optimization/qap-experiments")
rows = []
for ref in refs:
    row = {
        "image_name": ref.image_name,
        "manifest_digest": ref.manifest_digest,
        "config_digest": ref.config_digest,
        "updated_at": ref.updated_at,
        "status": ref.status,
        "run_count": ref.run_count,
        "solve_count": ref.solve_count,
    }
    row.update(
        {
            column: ref.annotations.get(annotation_key)
            for column, annotation_key in annotation_columns.items()
        }
    )
    rows.append(row)

catalog = pd.DataFrame.from_records(
    rows,
    columns=[
        "image_name",
        "manifest_digest",
        "config_digest",
        "updated_at",
        "status",
        "run_count",
        "solve_count",
        *annotation_columns,
    ],
)
catalog["updated_at"] = pd.to_datetime(catalog["updated_at"], utc=True)

candidates = catalog.loc[
    (catalog["status"] == "finished")
    & (catalog["study"] == "qap-solver-comparison")
    & (catalog["instance"] == "tai20a")
    & (catalog["formulation"] == "assignment")
    & catalog["solver"].isin(["highs", "scip"])
].sort_values("updated_at", ascending=False)

selected_experiments = [
    Experiment.load(image_name) for image_name in candidates["image_name"]
]
```

`prefix` argument は Local Registry で行う粗い filter で、full image-reference 文字列に
対して一致します。annotation ごとの filter は、一覧取得後に行う設計です。各
プロジェクトが annotation の語彙、column type、欠損値 policy を所有するため、registry
schema を変更せず DataFrame への投影を調整できます。この例では、存在しない
annotation は `None` になります。

完全な config は `ref.config` から取得できます。Config には Run と Solve の構造が
含まれるため、Local Registry schema に column を追加せず、consumerが
プロジェクト固有のtableを作れます。例えば、adapterとstatusを分析するために、
Solveごとのrowへ次のように投影できます。

```python
import json

solve_rows = []
for ref in refs:
    for run in ref.config["runs"]:
        for solve in run.get("solves", []):
            solve_rows.append(
                {
                    "manifest_digest": ref.manifest_digest,
                    "run_id": run["run_id"],
                    "solve_id": solve["solve_id"],
                    "status": solve["status"],
                    "adapter": solve["adapter"],
                    "adapter_options": json.loads(solve["adapter_options"]),
                }
            )

solves = pd.json_normalize(solve_rows)
```

Configに入っているのはpayload layerへの参照であり、payloadの値そのものでは
ありません。特にscalarなRun parameter値はRun parameter layerに保存されます。
その値が必要な場合は、候補のExperimentをloadし、
{py:meth}`~ommx.experiment.Experiment.run_parameters_df` を使ってください。

image name は mutable な ref であり、後から別の commit を指す可能性があります。
row の重複排除、解析した正確な Experiment の記録、異なる時点で取得した catalog の
比較には、immutable な identity である `manifest_digest` を使ってください。複数の
ref が同じ manifest を指している場合、同じ manifest が複数行に現れることがあります。

## 保存の境界

ExperimentのデータはCASに保存され、refと一覧用cacheはSQLiteに保存されます。

| 層 | 保存先 | 役割 |
|---|---|---|
| Blob | Local Registry の content-addressed file | Attachment、Instance、Solution、run parameters、config、manifest の bytes |
| Manifest | OCI Image Manifest blob | 1 つの immutable な OMMX Artifact を構成する blob 一覧 |
| Ref | Local Registry index の SQLite rows | manifest を到達可能にする名前または checkpoint pointer |
| 一覧用cache | manifestまたはconfig digestをkeyとするSQLite rows | registry一覧で使う元のManifest JSONとExperiment Config JSON |

cacheには元のJSON bytesをcontent digestをkeyとして保存し、読み出すときにもdigestを
検証します。cache rowがない場合は、一覧取得時にCASからbackfillします。そのため、
v1 Local Registryからのmigration直後や、古いwrite pathで作られたArtifactを初めて
一覧するときはManifestとConfig blobを読みます。backfill後の一覧は、各Experimentを
構築せずSQLite内のJSONを読みます。refを置換または削除すると、どのrefからも
到達できなくなったcache rowも削除されます。

このページで **publish** と呼ぶのは、Local Registry の ref を更新して、
すでに書き込まれた manifest を指すようにする操作です。これは local SQLite
上の操作であり、Artifact を remote container registry に push することでは
ありません。

{py:meth}`~ommx.experiment.Experiment.log_json` や {py:meth}`~ommx.experiment.Run.log_solve` のような logging method は、payload bytes をすぐ Local Registry に書き込みます。Experiment の最後まで全データをメモリに保持してから一括保存するわけではありません。同じ内容が既に存在する場合は既存の CAS blob を再利用し、その modification time を更新します。これにより、最近の active write は GC の grace period で保護されます。

正常に {py:meth}`~ommx.experiment.Experiment.commit` すると、Experiment config と root manifest が書かれ、requested image reference が SQLite に publish されます。ref の publish は payload blob を書き直しません。この順序のため、process が途中で終了すると、どの manifest や ref からも到達できない blob file が残ることがあります。そのような blob は Local Registry GC の対象になります。

## Run context と Experiment commit

`Run` は context manager として使ってください。Run は 1 つの試行であり、
Run を close することが、その close 済み Run を親 Experiment の未 commit 状態へ
追加する復帰境界になります。default では Run が close された後、OMMX はその親
Experiment の draft checkpoint を書き込み、checkpoint ref を publish します。

一方、Experiment は必ずしも context manager として使う必要はありません。
notebook では、1 つの Experiment を複数 cell にまたがって開いたままにするのが
自然な workflow です。1 つの Run を実行し、その結果を可視化・考察し、
次の条件を決めて別の Run を開始し、人間の workflow が終わった時点で明示的に
commit します。

```python
from ommx.experiment import Experiment

image_name = "ghcr.io/example/team/experiment:baseline"

experiment = Experiment(image_name)
experiment.log_json("dataset", {"name": "demo"})

with experiment.run() as run:
    run.log_parameter("capacity", 47)

# 結果を確認し、plot し、次の条件を決める。

with experiment.run() as run:
    run.log_parameter("capacity", 64)

artifact = experiment.commit()
```

短い Run を多数含む parameter sweep では、増え続ける Experiment を Run ごとに
checkpoint すると、final commit までに置き換え済み config blob と run-parameter
blob が多数作られます。unsealed session に autosave policy を設定して、復帰可能性と
保存量の tradeoff を変更できます。

```python
from ommx.experiment import AutosavePolicy, Experiment

experiment = Experiment(image_name)
experiment.set_autosave_policy(AutosavePolicy.every_n_runs(25))
```

`every_n_runs(n)` は追加で `n` 個の Run が close されるたびに checkpoint します。
`min_interval(seconds)` は次に close された最初の Run の checkpoint を試み、その後は
指定 interval あたり最大 1 回に制限します。publish に失敗した attempt も、次の retry
まで同じ interval を待ちます。`disabled()` は Run-close draft checkpoint を
無効にし、`every_run_close()` は default に戻します。policy を変更すると、その時点の
close 済み Run 数から新しい schedule が始まります。policy は現在の unsealed session
だけに属し、checkpoint や commit 済み Experiment には保存されません。Experiment
context が例外終了したときの failed / interrupted checkpoint は、この policy では
無効になりません。

すべての Run があらかじめ決まっている batch script では、
`with Experiment(...)` は便利な書き方です。正常終了時には `commit()` を呼び、
例外終了時には成功用の image reference を進めず failed または interrupted
checkpoint を publish します。

| 操作または event | 保存される状態 |
|---|---|
| `Run` が正常終了する | close 済み Run が status `"finished"` として親 Experiment に追加されます。autosave policy の条件を満たす場合は best-effort draft checkpoint が publish されます。 |
| `Run` が例外で終了する | close 済み Run が status `"failed"` または `"interrupted"` として親 Experiment に追加されます。autosave policy の条件を満たす場合は best-effort draft checkpoint が publish されます。例外はそのまま伝播します。 |
| `experiment.commit()` が成功する | final Experiment が commit され、requested image reference が publish されます。その Experiment の local checkpoint があれば削除されます。 |
| `with Experiment(...)` が正常終了する | block 末尾で `commit()` を呼ぶのと同じです。 |
| `with Experiment(...)` が例外で終了する | 成功用の requested image reference は進みません。status `"failed"` または `"interrupted"` の checkpoint Experiment が publish されます。 |
| Run close 後、`commit()` 前に notebook kernel または process が終了する | autosave policy が許可した最新の Experiment draft checkpoint から復帰します。その checkpoint より後に close された Run は再実行が必要です。 |
| open `Run` の exit 前に notebook kernel または process が終了する | その open Run が書いた payload blob は存在する可能性がありますが、復帰可能な Run state には含まれません。復帰地点は、その Run より前の最新 checkpoint です。 |

`KeyboardInterrupt` は Run / Experiment ともに `"interrupted"` として記録されます。それ以外の例外は `"failed"` として記録されます。

Run status は Run scope がどう close されたかを表します。子 Solve record の集約 status ではないため、
adapter error を Run 内で処理した場合、status `"finished"` の Run が failed Solve attempt を含むことがあります。

Experiment を context manager として使わない場合、Run の外側で起きた例外は
failed Experiment checkpoint を自動 publish しません。通常の interactive workflow
では、探索中は Run close 後に作られる Experiment draft checkpoint によって復帰可能性を確保し、
Experiment を公開できる状態になった時点で明示的に
{py:meth}`~ommx.experiment.Experiment.commit` します。

## Checkpoint から復帰する

{py:func}`~ommx.experiment.list_experiment_checkpoints` を使うと、元の requested image
name から復帰可能な Experiment を検索できます。`"draft"` は Run close 後に書かれる
rolling autosave であり、process や notebook kernel が強制終了した場合の復帰地点でも
あります。`"failed"` と `"interrupted"` は、OMMX が Experiment close 時に観測した
例外を記録した checkpoint です。

```python
from ommx.experiment import list_experiment_checkpoints

checkpoints = list_experiment_checkpoints(
    "ghcr.io/example/team",
    statuses=["draft", "failed", "interrupted"],
)
for checkpoint in checkpoints:
    print(
        checkpoint.requested_image_name,
        checkpoint.status,
        checkpoint.updated_at,
    )
```

`prefix` は hashed internal checkpoint ref ではなく `requested_image_name` に一致します。
`statuses` を省略すると 3 status すべてを対象にします。他の catalog function と同様、
個別 cache の failure は default では warning として除外し、残りを返します。最初の
不正 checkpoint を error にするには `strict=True` を指定します。

選択した checkpoint の元の requested image name を渡して復帰します。

```python
from ommx.experiment import Experiment, list_experiment_checkpoints

checkpoint = list_experiment_checkpoints(
    "ghcr.io/example/team/experiment:baseline"
)[0]

experiment = Experiment.restore_from_checkpoint(checkpoint.requested_image_name)

with experiment.run() as run:
    run.log_parameter("capacity", 64)

artifact = experiment.commit()
```

checkpoint ref は元の image name から導出される Local Registry 内部実装です。
`checkpoint_image_name` は registry 診断のため一覧 record に含まれますが、復帰には
`requested_image_name` を使います。

restore された Experiment は未 commit の Experiment なので、新しく作った
Experiment と同じように notebook cell をまたいで開いておけます。`commit()` を
呼ぶと元の requested image reference に publish され、checkpoint は削除されます。
restore 後の Experiment を context manager として使い、そこで再び失敗した場合は、
成功用の image reference を進めず、新しい failed または interrupted checkpoint が
publish されます。

## 失敗後の到達可能性

Local Registry cleanup は SQLite refs からの到達可能性で判断します。

| Data | 到達可能か | cleanup の挙動 |
|---|---|---|
| commit 済み Experiment image ref | Yes | `ommx gc` は manifest、config、layers、subject chain を保持します。 |
| Experiment checkpoint ref | Yes | `ommx gc` は復帰できるよう checkpoint を保持します。正常 commit すると checkpoint は削除されます。 |
| fork 先 Experiment から OCI `subject` で参照される parent manifest | child ref が保持されていれば Yes | `ommx gc` は subject chain を辿り、保持されている child から到達可能な parent payload を保持します。 |
| anonymous artifact refs | ref が存在する間は Yes | `ommx prune-anonymous` でこれらの ref を削除します。その後の `ommx gc` で、到達不能になった blob を回収できます。 |
| manifest/ref publish 前に process が終了して残った blob | No | grace period を過ぎると `ommx gc` が orphan candidate として report します。 |
| 実行中 process が書いている blob | checkpoint または commit されるまでは通常 No | `ommx gc` は grace period より新しければ deferred として削除を見送ります。 |

OMMX は SQLite に orphan table を作りません。orphan は GC report のたびに refs と manifests を辿って reachable set を作り、それを Local Registry の CAS file と比較して計算します。

## Cleanup workflow

cleanup command はまず report mode で実行してください。

```bash
ommx prune-anonymous
ommx gc
```

どちらもデフォルトでは dry-run で、registry を変更するのは `--delete` 指定時だけです。

```bash
ommx prune-anonymous --delete
ommx gc --delete
```

同じ操作は Python SDK からも実行できます。Python API は整形済みの CLI output ではなく、
structured report を返します。

```python
from ommx.artifact import gc, prune_anonymous

prune_report = prune_anonymous()
gc_report = gc()

prune_deleted = prune_anonymous(delete=True)
gc_deleted = gc(delete=True)
```

default 以外の Local Registry を調べる場合は `root=...`、GC の grace period を
変える場合は `grace_period="2h"` を指定します。

一時的な Artifact build や名前なし archive import から anonymous Artifact ref が残っている場合は、先に {command}`ommx prune-anonymous` を実行します。この command は該当する SQLite refs だけを削除し、blob は unlink しません。その blob は、他の ref から到達できなければ {command}`ommx gc` で回収可能になります。

{command}`ommx gc` は mark-sweep を行います。

- root は Experiment checkpoint refs を含むすべての SQLite refs です。
- 到達可能な manifest ごとに、manifest blob、config blob、layer blobs、OCI `subject` manifest chain を mark します。
- mark されなかった blob file は unreachable です。
- unreachable かつ `--grace-period` より古い blob は orphan candidate として report されます。
- unreachable だが `--grace-period` より新しい blob は deferred として report されます。
- `--delete` 指定時は orphan candidate だけを unlink し、削除直前にも各 candidate を再確認します。

default grace period は `24h` です。`s`、`m`、`h`、`d` suffix を指定できます。

```bash
ommx gc --grace-period 2h
ommx gc --grace-period 0s
```

`0s` は、その registry に書き込み中の OMMX process がないと分かっている場合だけ使ってください。共有 registry や default Local Registry では、open Run や interrupted import の書き込みを消さないよう、非ゼロの grace period を残してください。

通常の report は raw digest ではなく件数と byte size を表示します。特定の missing、invalid、orphan、deferred blob を調べる場合は `--show-digests` を追加します。

```bash
ommx gc --show-digests
ommx gc --delete --show-digests
```

default 以外の Local Registry を調べる、または掃除する場合は `--root <path>` を指定します。
