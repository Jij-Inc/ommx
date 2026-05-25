---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# 実験管理機能を使う

数理最適化の実験では、同じ元データから少しずつ条件を変えてインスタンスを作り、ソルバーやパラメータを変えながら複数回解くことがよくあります。このとき、個々の `Solution` だけを保存していると、後から「どの入力を」「どの条件で」「どのソルバーに渡して」得た結果なのかを追いにくくなります。

{py:mod}`ommx.experiment` は、このような実験の単位を OMMX Artifact として保存するための API です。

```{list-table}
:header-rows: 1

* - 概念
  - 役割
* - {py:class}`~ommx.experiment.Experiment`
  - 1つの実験全体。元データ、複数のRun、Run parameter、Solveをまとめて1つのArtifactに保存する
* - {py:class}`~ommx.experiment.Run`
  - 1つの実行条件。例えば「容量47で解く」「分割戦略Aで解く」のような比較単位
* - Attachment
  - 実験やRunに添付する任意のペイロード。JSON、{py:class}`~ommx.v1.Instance`、{py:class}`~ommx.v1.Solution`、{py:class}`~ommx.v1.SampleSet`、任意のbytesを保存できる
* - Run parameter
  - Runを比較するためのスカラー値。`bool`、`int`、`float`、`str`を表形式で保存する
* - {py:class}`~ommx.experiment.Solve`
  - 1回のソルバー呼び出し。入力 {py:class}`~ommx.v1.Instance`、出力 {py:class}`~ommx.v1.Solution`、実際に渡したsolver kwargsを保存する
```

このチュートリアルでは、簡単なナップサック問題を条件違いで2回解き、その実行記録を1つの `Experiment` として保存・読み出しします。

+++

## 問題データを用意する

まず、ナップサック問題の元データと、容量を指定して {py:class}`ommx.v1.Instance` を作る関数を用意します。

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable

v = [10, 13, 18, 31, 7, 15]  # 各アイテムの価値
w = [11, 25, 20, 35, 10, 33]  # 各アイテムの重さ
N = len(v)

x = [
    DecisionVariable.binary(
        id=i,
        name="x",
        subscripts=[i],
    )
    for i in range(N)
]


def build_instance(capacity: int) -> Instance:
    return Instance.from_components(
        decision_variables=x,
        objective=sum(v[i] * x[i] for i in range(N)),
        constraints={
            0: (sum(w[i] * x[i] for i in range(N)) <= capacity).add_name("重量制限")
        },
        sense=Instance.MAXIMIZE,
    )
```

+++

## ExperimentとRunを作成する

{py:class}`~ommx.experiment.Experiment` は commit すると OMMX Artifact として Local Registry に保存されます。このチュートリアルでは、実行環境の既定の Local Registry を汚さないように {py:meth}`~ommx.experiment.Experiment.with_temp_local_registry` を使います。通常のアプリケーションで永続的に保存したい場合は、`Experiment("example.com/project/name:tag")` のように明示的なArtifact名を付けます。

ここでは、実験全体の元データを Experiment-space Attachment として保存し、容量の違う2つのRunを作ります。各Runでは、比較用の `capacity` を Run parameter として記録し、補足情報をRun Attachmentとして保存します。

```{code-cell} ipython3
from ommx.experiment import Experiment

experiment = Experiment.with_temp_local_registry()

# 実験全体に属するペイロード。ここでは元データをJSONとして添付する。
experiment.log_json(
    "source-data",
    {
        "description": "knapsack demo",
        "values": v,
        "weights": w,
    },
)
```

{py:meth}`~ommx.experiment.Run.log_solve` を使うと、ソルバーを実行しながら、入力 {py:class}`~ommx.v1.Instance` と出力 {py:class}`~ommx.v1.Solution` がSolveとして自動的に保存されます。`adapter` には {py:class}`ommx.adapter.SolverAdapter` のサブクラスを渡します。後から見たときに「Solveはあるが入力Instanceが残っていない」という状態を避けるため、`log_solve` は常に入力Instanceを記録します。

```{code-cell} ipython3
from ommx_highs_adapter import OMMXHighsAdapter

capacities = [47, 56]

for capacity in capacities:
    instance = build_instance(capacity)
    with experiment.run() as run:
        # Run parameterはRunを横比較するためのスカラー値。
        run.log_parameter("capacity", capacity)
        run.log_parameter("strategy", "capacity-scenario")

        # Run Attachmentは、そのRunに紐づく任意のペイロード。
        run.log_json("scenario", {"capacity": capacity})

        # 実際にソルバーを呼び、Solveとして input/output/kwargs を保存する。
        solution = run.log_solve(OMMXHighsAdapter, instance, verbose=False)
        assert solution.feasible
```

+++

## commitしてArtifactとして保存する

{py:meth}`~ommx.experiment.Experiment.commit` によって、これまでの記録が1つのArtifactにまとめられます。commit後の {py:class}`~ommx.experiment.Experiment` は読み取り用の状態になり、{py:attr}`~ommx.experiment.Experiment.artifact` から対応するArtifactを取得できます。

```{code-cell} ipython3
artifact = experiment.commit()
experiment
```

`.ommx` ファイルとして共有したい場合は、commit後に {py:meth}`artifact.save <ommx.artifact.Artifact.save>` でアーカイブにエクスポートできます。ここでは以降の例を簡単にするため、メモリ上の `artifact` をそのまま使います。

+++

## 保存したExperimentを読み出す

ArtifactからExperimentを復元するには {py:meth}`~ommx.experiment.Experiment.from_artifact` を使います。明示的な名前で既定の Local Registry に保存したExperimentであれば、別プロセスから {py:meth}`~ommx.experiment.Experiment.load` で読み込むこともできます。

```{code-cell} ipython3
loaded = Experiment.from_artifact(artifact)
loaded
```

実験全体に添付したAttachmentは {py:attr}`~ommx.experiment.Experiment.experiment_attachments` から参照できます。Attachmentの中身はArtifact側の {py:meth}`~ommx.artifact.Artifact.get_json`、{py:meth}`~ommx.artifact.Artifact.get_instance` などで読み出します。

```{code-cell} ipython3
experiment_attachment = loaded.experiment_attachments[0]
artifact.get_json(experiment_attachment)
```

RunごとのAttachmentも同様に参照できます。

```{code-cell} ipython3
import pandas as pd

pd.DataFrame(
    [
        {
            "run_id": run.run_id,
            "attachment_name": attachment.annotations["org.ommx.attachment.name"],
            "media_type": attachment.media_type,
            "payload": artifact.get_json(attachment),
        }
        for run in loaded.runs
        for attachment in run.attachments
    ]
)
```

+++

## Run parameterを表として比較する

Run parameterは、Runを比較するための表形式データとして保存されます。{py:meth}`~ommx.experiment.Experiment.run_parameters_df` は `run_id` をindexにした `pandas.DataFrame` を返します。

```{code-cell} ipython3
loaded.run_parameters_df()
```

Run parameterは比較やフィルタリングに使うスカラー値を置く場所です。一方で、入力データ、生成したモデル、ソルバーログ、追加のJSONなど、ペイロードとして残したいものはAttachmentに保存します。

+++

## Solveの入力と出力を確認する

各Runには0個以上のSolveが紐づきます。{py:meth}`~ommx.experiment.Run.log_solve` で記録した {py:class}`~ommx.experiment.Solve` には、入力 {py:class}`~ommx.v1.Instance`、出力 {py:class}`~ommx.v1.Solution`、adapter名、Python側で `json.dumps` されたsolver kwargsが保存されています。

```{code-cell} ipython3
import json

solve_rows = []

for run in loaded.runs:
    for solve in run.solves:
        solution = artifact.get_solution(solve.output)
        solve_rows.append(
            {
                "run_id": run.run_id,
                "solve_id": solve.solve_id,
                "objective": solution.objective,
                "adapter": solve.parameters["adapter"],
                "kwargs": json.loads(solve.parameters["kwargs"]),
            }
        )

pd.DataFrame(solve_rows)
```

入力Instanceも同じように取得できます。

```{code-cell} ipython3
first_solve = loaded.runs[0].solves[0]
first_input = artifact.get_instance(first_solve.input)
first_input.sense == Instance.MAXIMIZE
```

`log_solve` が保存するsolver kwargsはSolveの属性であり、Run parameterではありません。Run parameterは、実験の比較軸として明示的に記録した値だけを含みます。

+++

## 使い分け

最後に、このチュートリアルで使った保存先の使い分けをまとめます。

```{list-table}
:header-rows: 1

* - 保存したいもの
  - 使うAPI
  - 用途
* - 実験全体の元データ、説明、補助ファイル
  - {py:meth}`~ommx.experiment.Experiment.log_json` / {py:meth}`~ommx.experiment.Experiment.log_attachment`
  - 実験全体に共有されるペイロード
* - Runごとの補足データ、生成物、ログ
  - {py:meth}`~ommx.experiment.Run.log_json` / {py:meth}`~ommx.experiment.Run.log_attachment`
  - 特定のRunに属するペイロード
* - Runを比較するための条件値
  - {py:meth}`~ommx.experiment.Run.log_parameter`
  - `run_parameters_df()` で横比較するスカラー値
* - ソルバー呼び出しの入力と出力
  - {py:meth}`~ommx.experiment.Run.log_solve`
  - 入力 `Instance`、出力 `Solution`、adapter名、solver kwargsをまとめて保存する
```

この構造にしておくと、後からArtifactだけを受け取った人も、Runごとの比較条件、各Solveの入力と出力、補助データを同じAPIで確認できます。
