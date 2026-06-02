# Experiment 管理

{mod}`ommx.experiment` は、最適化の試行錯誤を 1 つの OMMX Artifact として記録するための API です。複数の solver run を比較したいとき、入力モデルと solver 出力をまとめて残したいとき、または実行履歴全体を別環境へ共有したいときに使います。

実行できる一連の例は [実験を記録して共有する](../tutorial/experiment_management.md) を参照してください。このページでは API model と lifecycle を説明します。

## データモデル

Experiment には 2 つの保存空間があります。

| 保存空間 | API | 用途 |
|---|---|---|
| Experiment space | {class}`~ommx.experiment.Experiment` の logging methods | source model、dataset metadata、分析メモなど、実験全体で共有する情報 |
| Run space | {class}`~ommx.experiment.Run` の logging methods | 1 つの試行に属する parameter、solver 入出力、trace、run 固有ファイル |

run 間で比較したい scalar 値には {meth}`~ommx.experiment.Run.log_parameter` を使います。これらの値は {meth}`~ommx.experiment.Experiment.run_parameters_df` に現れます。

payload には attachment を使います。JSON、{class}`~ommx.v1.Instance`、{class}`~ommx.v1.ParametricInstance`、{class}`~ommx.v1.Solution`、{class}`~ommx.v1.SampleSet` には typed helper があります。未知の media type は bytes として保存・読み込みされるため、外部 package が独自 codec を所有できます。

solver 呼び出しを 1 つの {class}`~ommx.experiment.Solve` として記録したい場合は {meth}`~ommx.experiment.Run.log_solve` を使います。入力 Instance、出力 Solution、adapter class name、JSON-serializable な adapter options が保存されます。

## Lifecycle

新しい {class}`~ommx.experiment.Experiment` は未 commit の session です。logging methods は payload bytes をすぐ Local Registry に保存しますが、Experiment は commit されるまで共有可能な Artifact にはなりません。正常に {meth}`~ommx.experiment.Experiment.commit` すると、Experiment config と manifest が書かれ、requested image reference が publish され、その object は read-only view になります。

`with Experiment(...)` を使うと、正常終了時に `commit()` されます:

```python
from ommx.experiment import Experiment

with Experiment("ghcr.io/example/team/experiment:baseline") as experiment:
    experiment.log_json("dataset", {"name": "demo"})
    with experiment.run() as run:
        run.log_parameter("capacity", 47)
```

Run も lifecycle を持ちます。user code が例外を投げた場合でも Run を close するため、`with experiment.run()` を使ってください。close 済み Run の status は `"finished"`、`"failed"`、または `"interrupted"` です。`KeyboardInterrupt` は `"interrupted"` として記録されます。

## Checkpoint

OMMX は途中状態を local checkpoint として保存します。

- Run を close すると best-effort に draft checkpoint を publish します。
- Experiment が例外で終了すると、成功用の Experiment image reference を進めず、failed または interrupted checkpoint を publish します。
- 正常に commit すると、存在する local checkpoint は削除されます。

checkpoint から復帰するには、元の Experiment image name を渡します:

```python
experiment = Experiment.restore_from_checkpoint(
    "ghcr.io/example/team/experiment:baseline",
)
```

checkpoint name や checkpoint Artifact handle は通常の public handle として公開されません。復帰したい Experiment では、元の Experiment image name を保持してください。

open Run の途中で書かれた payload は Local Registry に保存されますが、checkpoint から復元できるのは Run が close された後です。process が open Run の途中で kill された場合、復帰地点は最後に close された Run の checkpoint になります。

## 共有

commit 済み Experiment は、他の OMMX Artifact と同じように共有できます。

```python
experiment.rename("ghcr.io/example/team/experiment:baseline")
experiment.push()
experiment.save("experiment.ommx")
```

名前付き Experiment には {meth}`~ommx.experiment.Experiment.load` を使います。受け取った `.ommx` archive には {meth}`~ommx.experiment.Experiment.import_archive` を使います。

## Fork

commit 済み Experiment は immutable です。さらに Run を追加したい場合は {meth}`~ommx.experiment.Experiment.fork` を使います。

```python
loaded = Experiment.load("ghcr.io/example/team/experiment:baseline")

with loaded.fork("ghcr.io/example/team/experiment:capacity-64") as child:
    with child.run() as run:
        run.log_parameter("capacity", 64)
```

child は parent の attachments、Runs、Solves、run parameters を引き継ぎます。commit すると、child manifest は parent manifest を OCI `subject` として記録します。payload blob は content-addressed に再利用されるため、変わっていない Instance、Solution、attachment bytes は重複保存されません。

## Local Registry cleanup

Local Registry は blob を digest で、ref を SQLite で管理します。これにより logging は効率的になりますが、process が payload を書いたあと commit や checkpoint から到達可能になる前に終了すると orphan blob が残ることがあります。

cleanup command はまず dry-run mode で実行してください:

```bash
ommx prune-anonymous
ommx gc
```

どちらもデフォルトでは report だけを行い、registry を変更するのは `--delete` 指定時だけです。

```bash
ommx prune-anonymous --delete
ommx gc --delete
```

`ommx gc` は Experiment checkpoint を含む SQLite refs を root として扱います。grace period より新しい unreachable blob は、active write のデータを消さないため deferred になります。低レベルの診断が必要な場合だけ `--show-digests` を指定してください。
