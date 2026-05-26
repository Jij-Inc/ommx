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

# 実験を記録し共有する

多くの実務的な数理最適化の問題は数理モデルを作ってソルバーに投げれば終わり、という単純なフローで終わる幸運なケースは少なく、複数のモデル化を比較したり一部の制約条件を緩和したり、より解きやすい部分問題を解いてみたり、という試行錯誤のプロセスが伴います。OMMXはモデリングされた問題の管理とAdapterを通した最適化ソルバーによる求解結果の管理に加えて、これらの試行錯誤のプロセスも「実験」として記録し、保存・共有するためのAPIを提供しています。

{py:mod}`~ommx.experiment` は、このような実験の単位を OMMX Artifact として保存するための API です。

```{list-table}
:header-rows: 1

* - 概念
  - 役割
* - {py:class}`~ommx.experiment.Experiment`
  - 1つの実験全体。実験そのものに紐づいた Attachment に加えて複数のRunを持つことができる。共有の単位でありコンテナとしての名前が常に付与される。
* - {py:class}`~ommx.experiment.Run`
  - 実験の中で行った一つの試行、比較の単位。複雑な問題を解く際に複数回のソルバー呼び出しを伴うことがよくあるため、Runは複数回のソルバー呼び出し（Solve）を持つことができる。加えてRun毎に比較の軸となるスカラー値のパラメータを付与でき、Experiment全体でのRunの比較を容易にする。
* - {py:class}`~ommx.experiment.Solve`
  - Runの中で行った一回のソルバー呼び出し。入力として {py:class}`~ommx.v1.Instance` を取り、出力として {py:class}`~ommx.v1.Solution` を保存する。さらに、どのAdapterを使ったか、ソルバー呼び出しに渡したオプションも記録する。
* - Attachment
  - ExperimentやRunに添付する任意のペイロード。JSON、`numpy.ndarray`、{py:class}`~ommx.v1.Instance`、{py:class}`~ommx.v1.Solution`などのデータ型に加えて、任意のbytesをMedia Typeを指定して保存できる。
```

このチュートリアルでは、簡単なナップサック問題を条件違いで2回解き、その実行記録を1つの {py:class}`~ommx.experiment.Experiment` として保存・読み出しします。

+++

## 数理モデルを用意する

まず、ナップサック問題の元データと、容量をパラメータとして持つ {py:class}`~ommx.v1.ParametricInstance` を作ります。OMMXの {py:class}`~ommx.v1.ParametricInstance` は、{py:class}`~ommx.v1.Instance` と同様に、目的関数や制約条件を定義できますが、定数項の代わりにパラメータを置くことができます。後からRun parameterやRun Attachmentで比較したい条件を記録するのではなく、あらかじめParametricInstanceのパラメータとして置いておくと、実験全体で一貫した表現になります。

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable, ParametricInstance, Parameter

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

capacity = Parameter(N, name="capacity")

pi = ParametricInstance.from_components(
  decision_variables=x,
  parameters=[capacity],
  objective=sum(v[i] * x[i] for i in range(N)),
  constraints={
      0: (sum(w[i] * x[i] for i in range(N)) <= capacity).add_name("重量制限")
  },
  sense=Instance.MAXIMIZE,
)
```

## 実験する

今回は上で作ったナップサック問題に対して２つの異なる容量で解いてみます。

```{code-cell} ipython3
from ommx.experiment import Experiment
from ommx_highs_adapter import OMMXHighsAdapter

# 実験を開始する。名前を指定しないと自動的に名前が割り当てられる
with Experiment() as experiment:

  # 上で作ったモデルをExperimentの情報として保存する。
  experiment.log_parametric_instance("instance", pi)

  # 今回は必要ないが、モデルの情報をJSONで保存することもできる。
  experiment.log_json(
    "source-data",
    {
      "description": "knapsack demo",
      "values": v,
      "weights": w,
    },
  )

  # 容量の異なる2つのRunを作る
  for c in [47, 56]:

    # モデルのパラメータを具体的な値で置き換える
    instance = pi.with_parameters({capacity.id: c})

    # Runを開始する。Runは初期化と終了処理を伴うので、with文で囲むのが推奨される。
    with experiment.run() as run:

      # Runの比較パラメータとしてcapacityを記録する。
      run.log_parameter("capacity", c)

      # HiGHS Adapterを呼び出して解く。入力Instanceと出力Solutionは自動的に保存される。
      solution = run.log_solve(OMMXHighsAdapter, instance, verbose=False)

      # 解けていることを確認
      assert solution.feasible

      # withブロックを抜けるとRunの終了処理が行われる

  # experimentのwithブロックを抜けるとExperimentの終了処理が行われる。
```


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
