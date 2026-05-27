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

まず、ナップサック問題の元データと、容量をパラメータとして持つ {py:class}`~ommx.v1.ParametricInstance` を作ります。OMMXの {py:class}`~ommx.v1.ParametricInstance` は、{py:class}`~ommx.v1.Instance` と同様に、目的関数や制約条件を定義できますが、定数項の代わりにパラメータを置くことができます。定数だけ異なるモデルを複数用意する必要がある場合に便利です。

```{code-cell} ipython3
from ommx.v1 import DecisionVariable, Parameter, Instance, ParametricInstance

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

実験の途中で保存されたデータはすべてOMMXの *Local Registry* に保存されます。

- OMMXのLocal RegistryはOMMX Artifactの構成要素を効率よく保存するためのストレージです。`OMMX_LOCAL_REGISTRY_ROOT` 環境変数で場所を変更できます。 {py:meth}`~ommx.experiment.Experiment.with_temp_local_registry` などの一時的なLocal Registryを生成して使うAPIもあります。
- `log_json` や `log_solve` ではデータは随時Local Registryに保存されていきます。メモリ上に置いておいてExperimentの最後にまとめて保存するわけではありません。これはデータの内容（SHA256ハッシュ値）をもとに保存パスが決められるので、同じデータはLocal Registry単位で一度だけ保存されます。
- Experimentの終了処理ではそのExperiment中に保存されたデータの一覧をまとめたJSON（Artifact Manifest）をLocal Registryに保存して、起動時に指定あるいは自動的に決めたExperimentの名前でこのArtifact Manifestを指すタグをLocal Registryに保存します。

## 実験を共有する

実験を共有するにはその実験を識別する名前が必要です。Experimentの名前は、実験の開始時に `Experiment(name=...)` で指定するか、あるいは実験の途中や最後に {py:meth}`Experiment.rename` で変更することができます。また指定しない場合はデフォルトで次の形式で名前を生成します。

```text
bb040f6d.ommx.local/experiment:20260527T132713-e3c041e71f4b
|                              |               ^^^^^^^^^^^^ 重複を防ぐためのランダムな文字列
|                              ^^^^^^^^^^^^^^^ 作成時刻（Local Time）
^^^^^^^^ Local Registry自体の識別子
```

このデフォルト名は `*.ommx.local` とあるようコンテナレジストリにはPushできないようになっており、主に一時的な管理を目的としています。一部のコマンドでこれらのデフォルト名を持つExperimentをClean upするので、永続的に保存したいExperimentには適切な名前を付けることが推奨されます。

例えば、実験をGitHub Container RegistryにPushして共有したい場合は、次のようにします。

```python
# <コンテナレジストリ>/<ユーザ名>/<リポジトリ名>:<タグ> の形式で名前を付ける
experiment.rename("ghcr.io/jij-inc/ommx/tutorial/experiment:knapsack")

# コンテナレジストリにPushする
experiment.push()
```

Tutorialの読者はOMMXのリポジトリにPushする権限はないと思うので、適宜読み替えてください。OMMXはコンテナレジストリへの認証はDockerに移譲するので、事前に `docker login` でコンテナレジストリにログインしておく必要があります。

### GitHub Container Registryの場合

To be written.

### Google Cloud Artifact Registryの場合

To be written.

### ファイルとしてExport/Importする

コンテナレジストリを使わずに、`.ommx` ファイルとしてExportすることもできます。これはメールやファイルストレージなどで一時的に受け渡すための補助的な方法です。

```python
experiment.save("tutorial_experiment.ommx")
```

受け取った `.ommx` ファイルは {py:meth}`~ommx.experiment.Experiment.import_archive` でLocal Registryに取り込んでから開きます。

```python
loaded_experiment = Experiment.import_archive(archive_path)
```

## 共有された実験を確認する

Experimentは名前で識別されているので、共有されたExperimentは名前で {py:meth}`~ommx.experiment.Experiment.load` することで読み込むことができます。

```python
loaded_experiment = Experiment.load("ghcr.io/jij-inc/ommx/tutorial/experiment:knapsack")
```

これはLocal Registry上で名前を探して、見つからなければコンテナレジストリからPullしてきてLocal Registryに保存してから読み込む、という動きをします。

`load` や `import_archive` は終了処理が終わった {py:class}`~ommx.experiment.Experiment` と同じ状態としてロードされるので、今回は上で作ったExperimentをそのまま使います。

```{code-cell} ipython3
loaded_experiment = experiment
```

読み込んだExperimentからは、まずRunごとに記録したパラメータを表として確認できます。

```{code-cell} ipython3
loaded_experiment.run_parameters_df()
```

{py:meth}`~ommx.experiment.Experiment.run_parameters_df` はRunを比較するための表です。ここには {py:meth}`~ommx.experiment.Run.log_parameter` で記録したRun単位のパラメータだけが現れます。SolverAdapterに渡したオプションはRunの比較軸ではなく、後で見るSolve単位の情報として保存されます。

Experiment単位で保存したAttachmentは名前で確認し、必要なものを名前で取り出します。{py:meth}`~ommx.experiment.Experiment.get_attachment` は保存時のMedia Typeを見て、JSONならPythonの値、{py:class}`~ommx.v1.ParametricInstance` ならそのオブジェクト、というように変換して返します。期待する型が分かっている場合は {py:meth}`~ommx.experiment.Experiment.get_json` や {py:meth}`~ommx.experiment.Experiment.get_parametric_instance` のような型ごとのメソッドを使うと、Media Typeが違っていた場合にエラーになります。

```{code-cell} ipython3
loaded_experiment.attachment_names
```

```{code-cell} ipython3
loaded_experiment.get_json("source-data")
```

```{code-cell} ipython3
loaded_experiment.get_parametric_instance("instance")
```

Runの一覧は {py:attr}`~ommx.experiment.Experiment.runs` から確認できます。終了済みのRunが作成順に並び、それぞれのRunに紐づくAttachmentとSolveの数を確認できます。

```{code-cell} ipython3
[
    {
        "run_id": run.run_id,
        "attachments": len(run.attachment_names),
        "solves": len(run.solves),
    }
    for run in loaded_experiment.runs
]
```

各Runの中で実行されたソルバー呼び出しは {py:attr}`~ommx.experiment.SealedRun.solves` に入っています。{py:class}`~ommx.experiment.Solve` は一回の `log_solve` 呼び出しに対応し、入力Instance、出力Solution、利用したAdapter、Adapterへ渡したキーワード引数を記録しています。

```{code-cell} ipython3
import json

[
    {
        "run_id": run.run_id,
        "solve_id": solve.solve_id,
        "adapter": solve.parameters["adapter"],
        "kwargs": json.loads(solve.parameters["kwargs"]),
        "input": type(solve.input).__name__,
        "objective": solve.output.objective,
        "feasible": solve.output.feasible,
    }
    for run in loaded_experiment.runs
    for solve in run.solves
]
```

このように、ExperimentのAPIでは「どのRunを行い、Runごとにどの比較パラメータを記録し、それぞれのRunの中でどのSolveが実行され、どの入力と出力が保存されたか」を一覧できます。実験管理の第一歩は、保存された個々のデータ本体を読むことではなく、このExperimentの構造を確認して、後から参照すべきRunやSolveを特定することです。
