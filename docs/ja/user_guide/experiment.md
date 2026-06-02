# Experiment の復帰と cleanup

{mod}`ommx.experiment` は、最適化の試行錯誤を 1 つの OMMX Artifact として記録するための API です。Experiment のデータモデル、実行できる logging 例、共有、確認、fork については [実験を記録して共有する](../tutorial/experiment_management.md) を参照してください。

このページでは、失敗時の挙動に絞って説明します。Experiment が commit される前に何が書かれるのか、checkpoint からどう復帰するのか、Local Registry cleanup がどの blob を削除可能と判断するのかを扱います。

## 保存の境界

Experiment のデータは 3 つの層に分かれて保存されます。

| 層 | 保存先 | 役割 |
|---|---|---|
| Blob | Local Registry BlobStore の content-addressed file | Attachment、Instance、Solution、run parameters、config、manifest の bytes |
| Manifest | OCI Image Manifest blob | 1 つの immutable な OMMX Artifact を構成する blob 一覧 |
| Ref | Local Registry index の SQLite rows | manifest を到達可能にする名前または checkpoint pointer |

{py:meth}`~ommx.experiment.Experiment.log_json` や {py:meth}`~ommx.experiment.Run.log_solve` のような logging method は、payload bytes をすぐ BlobStore に書き込みます。Experiment の最後まで全データをメモリに保持してから一括保存するわけではありません。同じ内容が既に存在する場合は既存の CAS blob を再利用し、その modification time を更新します。これにより、最近の active write は GC の grace period で保護されます。

正常に {py:meth}`~ommx.experiment.Experiment.commit` すると、Experiment config と root manifest が書かれ、requested image reference が SQLite に publish されます。ref の publish は payload blob を書き直しません。この順序のため、process が途中で終了すると、どの manifest や ref からも到達できない blob file が残ることがあります。そのような blob は Local Registry GC の対象になります。

## Context manager が復帰の境界になる

Experiment と Run はどちらも context manager として使ってください。

```python
from ommx.experiment import Experiment

image_name = "ghcr.io/example/team/experiment:baseline"

with Experiment(image_name) as experiment:
    experiment.log_json("dataset", {"name": "demo"})
    with experiment.run() as run:
        run.log_parameter("capacity", 47)
```

context manager の exit が、何を復帰可能な状態として残すかを決めます。

| Event | 保存される状態 |
|---|---|
| `Run` が正常終了する | Run は status `"finished"` として記録され、best-effort に draft checkpoint が publish されます。 |
| `Run` が例外で終了する | Run は status `"failed"` または `"interrupted"` として記録され、best-effort に draft checkpoint が publish されます。例外はそのまま伝播します。 |
| `Experiment` が正常終了する | final Experiment が commit され、requested image reference が publish されます。その Experiment の local checkpoint があれば削除されます。 |
| `Experiment` が例外で終了する | 成功用の requested image reference は進みません。status `"failed"` または `"interrupted"` の checkpoint Experiment が publish されます。 |
| open `Run` の exit 前に process が kill される | その open Run が書いた payload blob は存在する可能性がありますが、復帰可能な Run state には含まれません。復帰地点は、close 済み Run または Experiment 例外処理が作った最新 checkpoint です。 |

`KeyboardInterrupt` は Run / Experiment ともに `"interrupted"` として記録されます。それ以外の例外は `"failed"` として記録されます。

## Checkpoint から復帰する

checkpoint から復帰するには、元の Experiment image name を渡します。

```python
from ommx.experiment import Experiment

image_name = "ghcr.io/example/team/experiment:baseline"

with Experiment.restore_from_checkpoint(image_name) as experiment:
    with experiment.run() as run:
        run.log_parameter("capacity", 64)
```

checkpoint ref は元の image name から導出される internal Local Registry ref です。通常の Artifact handle としては公開されないため、復帰したい Experiment では元の image name を保持してください。

restore された Experiment は未 commit の Experiment です。正常終了すれば元の requested image reference に commit され、checkpoint は削除されます。restore 後の Experiment が再び失敗した場合は、成功用の image reference を進めず、新しい failed または interrupted checkpoint が publish されます。

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

OMMX は SQLite に orphan table を作りません。orphan は GC report のたびに refs と manifests を辿って reachable set を作り、それを BlobStore 上の file と比較して計算します。

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
