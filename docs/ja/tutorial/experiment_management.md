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
  - Runの中で行った一回の solver または sampler 呼び出し。入力の {py:class}`~ommx.Instance`、使用した Adapter、呼び出しに渡した option を常に記録する。finished Solve は出力の {py:class}`~ommx.Solution` または {py:class}`~ommx.SampleSet` も保存し、failed または interrupted Solve は output を持たない。
* - Attachment
  - ExperimentやRunに添付する任意のペイロード。JSON、`numpy.ndarray`、{py:class}`~ommx.Instance`、{py:class}`~ommx.Solution`などのデータ型に加えて、任意のbytesをMedia Typeを指定して保存できる。
```

このチュートリアルでは、簡単なナップサック問題を条件違いで2回解き、その実行記録を1つの {py:class}`~ommx.experiment.Experiment` として保存・読み出しします。

+++

## 数理モデルを用意する

まず、ナップサック問題の元データと、容量をパラメータとして持つ {py:class}`~ommx.ParametricInstance` を作ります。OMMXの {py:class}`~ommx.ParametricInstance` は、{py:class}`~ommx.Instance` と同様に、目的関数や制約条件を定義できますが、定数項の代わりにパラメータを置くことができます。定数だけ異なるモデルを複数用意する必要がある場合に便利です。

```{code-cell} ipython3
from ommx import DecisionVariable, Parameter, Instance, ParametricInstance

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
        0: (sum(w[i] * x[i] for i in range(N)) <= capacity).set_name("重量制限")
    },
    sense=Instance.MAXIMIZE,
)
```

(experiment-management-attachable-data-formats)=
### 添付できるデータ形式

上で作った {py:class}`~ommx.ParametricInstance` がソルバーに渡すOMMX形式の数理モデルです。実験を後から見直すためには、このOMMXモデルに加えて、元のモデリング用オブジェクトや入力ファイルなどもExperimentに添付しておくと便利です。

元のモデルをモデリング用パッケージで記述している場合は、そのソースモデルもAttachmentとして保存しておくと後から参照できます。外部パッケージが所有する型について、OMMXはAttachment CodecのProtocolと、それを呼び出す `log_with_codec` / `get_with_codec` メソッドだけを定義します。具体的なCodecはその型を所有するパッケージ側で提供します。このチュートリアルではJijModeling `Problem` 用の一時的な `ProblemCodec` を定義して使います。同等のCodecは将来的にJijModeling本体で提供される予定です。

```{code-cell} ipython3
import jijmodeling as jm


class ProblemCodec:
    media_type = "application/vnd.jijmodeling.problem+protobuf"

    @staticmethod
    def encode(problem: jm.Problem) -> bytes:
        return problem.to_protobuf()

    @staticmethod
    def decode(data: bytes) -> jm.Problem:
        return jm.Problem.from_protobuf(data)


@jm.Problem.define("Knapsack Problem", sense=jm.ProblemSense.MAXIMIZE)
def jij_problem(problem: jm.DecoratedProblem):
    N = problem.Length(description="アイテム数")
    W = problem.Float(description="耐荷重")
    w = problem.Float(shape=N, description="各アイテムの重さ")
    v = problem.Float(shape=N, description="各アイテムの価値")
    x = problem.BinaryVar(
        shape=N,
        description="アイテム i をナップサックに入れるときのみ x_i=1",
    )

    problem += jm.sum(v[i] * x[i] for i in N)
    problem += problem.Constraint(
        "重量制限",
        jm.sum(w[i] * x[i] for i in N) <= W,
    )
```

一方で、payload がすでにファイルとして存在するなら、そのファイルを直接添付します。`log_file` はファイルのbytesをExperimentにコピーします。後から読む側では、bytesとして読む `get_blob` か、実ファイルとして復元する `write_attachment` を使えます。Excel workbook、solver log、生成したplotなど、OMMXの外で作られたファイルにはこの経路を使うのが自然です。

```python
import io
from pathlib import Path

experiment.log_file("input-spreadsheet", "input.xlsx")

spreadsheet_file = io.BytesIO(loaded_experiment.get_blob("input-spreadsheet"))
# `spreadsheet_file` はbinary file-like objectを受け取るライブラリに渡せる。
Path("restored").mkdir(parents=True, exist_ok=True)
loaded_experiment.write_attachment("input-spreadsheet", "restored/input.xlsx")
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

    # 元のJijModeling Problemを上で定義した一時的なCodec経由で保存する。
    experiment.log_with_codec(
        ProblemCodec,
        "jijmodeling-problem",
        jij_problem,
    )

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

            # Runの比較パラメータとして目的関数値も記録する。
            run.log_parameter("objective", solution.objective)

            # withブロックを抜けるとRunの終了処理が行われる

    # experimentのwithブロックを抜けるとExperimentの終了処理が行われる。
```

実験の途中で保存されたデータはすべてOMMXの *Local Registry* に保存されます。

- OMMXのLocal RegistryはOMMX Artifactの構成要素を効率よく保存するためのストレージです。`OMMX_LOCAL_REGISTRY_ROOT` 環境変数で場所を変更できます。 {py:meth}`~ommx.experiment.Experiment.with_temp_local_registry` などの一時的なLocal Registryを生成して使うAPIもあります。
- `log_json`、`log_solve`、`log_sample` ではデータは随時Local Registryに保存されていきます。メモリ上に置いておいてExperimentの最後にまとめて保存するわけではありません。これはデータの内容（SHA256ハッシュ値）をもとに保存パスが決められるので、同じデータはLocal Registry単位で一度だけ保存されます。
- Experimentの終了処理ではそのExperiment中に保存されたデータの一覧をまとめたJSON（Artifact Manifest）をLocal Registryに保存して、起動時に指定あるいは自動的に決めたExperimentの名前でこのArtifact Manifestを指すタグをLocal Registryに保存します。

### ソルバーのModelを直接操作する場合

通常のRunでは {py:meth}`~ommx.experiment.Run.log_solve` を使います。これはadapterの `solve` メソッドを呼び出し、入力、出力、adapter名、adapter optionsをまとめて記録します。一方で、AdapterのAPIではサポートしきれていないソルバーの高度な機能を使う必要がある場合は、手動Solveスコープを開きます。

{py:class}`~ommx.adapter.SamplerAdapter` では、代わりに {py:meth}`~ommx.experiment.Run.log_sample` を使います。adapter の `sample` メソッドを呼び出し、完全な {py:class}`~ommx.SampleSet` を Solve の出力として記録します。sampling 自体が成功していれば、SampleSet に feasible sample がなくても finished として記録されます。

手動Solveスコープでは、まず `solver_input` でバックエンドソルバーのModelを受け取り、ユーザーがそのModelを直接操作して最適化を行います。最後に `solve.decode(model)` を呼ぶと、adapterがバックエンドの結果を {py:class}`~ommx.Solution` に変換し、そのSolutionがExperimentに記録されるSolveの出力になります。

```python
with experiment.run() as run:
    run.log_parameter("capacity", c)

    with run.open_solve(OMMXHighsAdapter, instance, verbose=False) as solve:
        model = solve.solver_input
        model.setOptionValue("time_limit", 10.0)
        solve.log_adapter_option("time_limit", 10.0)

        model.run()
        solution = solve.decode(model)
```

`solve.log_adapter_option(...)` は、バックエンドModelへ直接設定したoptionを `Solve.adapter_options` に残すための補助APIです。`open_solve` の詳細な挙動、diagnostics、trace、失敗時の扱いは {py:class}`~ommx.experiment.OpenSolve` を参照してください。

## 実験を共有する

実験を共有するにはその実験を識別する名前が必要です。Experimentの名前は、実験の開始時に `Experiment(name=...)` で指定するか、あるいは実験の途中や最後に {py:meth}`Experiment.rename` で変更することができます。また指定しない場合はデフォルトで次の形式で名前を生成します。

```text
bb040f6d.ommx.local/experiment:20260527T132713-e3c041e71f4b
|                              |               ^^^^^^^^^^^^ 重複を防ぐためのランダムな文字列
|                              ^^^^^^^^^^^^^^^ 作成時刻（Local Time）
^^^^^^^^ Local Registry自体の識別子
```

このデフォルト名は `*.ommx.local` とあるように、外部のコンテナレジストリにはPushできないようになっており、主に一時的な管理を目的としています。一部のコマンドでこれらのデフォルト名を持つExperimentをClean upするので、永続的に保存したいExperimentには適切な名前を付けることが推奨されます。

例えば、実験をGitHub Container Registry (ghcr.io) にPushして共有したい場合は、次のようにします。

```python
# <コンテナレジストリ>/<ユーザ名>/<リポジトリ名>:<タグ> の形式で名前を付ける
experiment.rename("ghcr.io/jij-inc/ommx/tutorial/experiment:knapsack")

# コンテナレジストリにPushする
experiment.push()
```

Tutorialの読者はOMMXのリポジトリにPushする権限はないはずなので適宜読み替えてください。OMMXはコンテナレジストリへの認証はDockerに移譲するので、事前に `docker login` でコンテナレジストリにログインしておく必要があります。

### GitHub Container Registryの場合

To be written.

### Google Cloud Artifact Registryの場合

To be written.

### ファイルとしてExport/Importする

コンテナレジストリを使わずに、`.ommx` ファイルとしてExportすることもできます。これはAWS S3などのファイルストレージなどで一時的に受け渡すための補助的な方法です。

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

{py:meth}`~ommx.experiment.Experiment.load` や {py:meth}`~ommx.experiment.Experiment.import_archive` は終了処理が終わった {py:class}`~ommx.experiment.Experiment` と同じ状態としてロードされるので、今回は上で作ったExperimentをそのまま使います。

```{code-cell} ipython3
loaded_experiment = experiment
```

### Run Parameters

読み込んだExperimentからは実験の情報を読み出すことができます。まず {py:meth}`~ommx.experiment.Experiment.run_parameters_df` はRunごとに {py:meth}`~ommx.experiment.Run.log_parameter` で記録したパラメータを `pandas.DataFrame` として一覧する機能を提供します。

```{code-cell} ipython3
loaded_experiment.run_parameters_df()
```

これは例えば次のようになっているはずです。

```text
        capacity  objective
run_id
     0        47         41
     1        56         49
```

### Attachments

Experiment単位で保存したAttachmentは名前で確認し、必要なものを名前で取り出します。{py:meth}`~ommx.experiment.Experiment.get_attachment` は保存時のMedia Typeを見て、JSONならPythonの値、{py:class}`~ommx.ParametricInstance` ならそのオブジェクト、というように変換して返します。期待する型が分かっている場合は {py:meth}`~ommx.experiment.Experiment.get_json` や {py:meth}`~ommx.experiment.Experiment.get_parametric_instance` のような型ごとのメソッドを使うと、Media Typeが違っていた場合にエラーになります。

```{code-cell} ipython3
# 保存したAttachmentの名前を確認する
assert loaded_experiment.attachment_names == [
    "instance",
    "jijmodeling-problem",
    "source-data",
]

# JSONとして保存したデータを取り出す
source_data = loaded_experiment.get_json("source-data")
assert source_data == {
    "description": "knapsack demo",
    "values": v,
    "weights": w,
}

# get_attachmentはMedia Typeを見て適切に変換してくれる
pi = loaded_experiment.get_attachment("instance")
assert isinstance(pi, ParametricInstance)

# CodecがMedia Typeを検証し、元のpayloadへdecodeして返す
restored_jij_problem = loaded_experiment.get_with_codec(
    ProblemCodec,
    "jijmodeling-problem",
)
assert restored_jij_problem.name == jij_problem.name
```

### RunsとSolves

Runの一覧は {py:attr}`~ommx.experiment.Experiment.runs` から確認できます。終了済みのRunが作成順に並び、それぞれのRunに紐づくAttachmentとSolveを確認できます。

trace storageを有効にして記録したRunでは、{py:attr}`~ommx.experiment.SealedRun.trace` から保存済みのRunトレースを取得できます。これは発展的な機能なので、詳細は {ref}`experiment-run-trace-storage` を参照してください。

```{code-cell} ipython3
from typing import Any
from ommx import Solution

for run in loaded_experiment.runs:
    # Runには実行順にIDが振られる
    assert run.run_id in [0, 1]

    # 今回はRun単位のAttachmentは保存していないので、Attachmentの数は0のはず
    assert len(run.attachment_names) == 0

    # 1回しかSolveしていないので、Solveの数は1のはず
    assert len(run.solves) == 1
    solve = run.solves[0]

    # Solveにも実行順にIDが振られるが、今回はRunごとに1回しかSolveしていないので、Solve IDは0のはず
    assert solve.solve_id == 0

    # 実行したAdapterの名前
    assert solve.adapter.endswith("OMMXHighsAdapter")

    # 入力と出力をロードする
    input: Instance = solve.input
    output: Solution | None = solve.output
    assert output is not None

    # ナップザック問題は解けているはず
    assert output.feasible

    # Adapterに渡したオプションもロードする
    options: dict[str, Any] = solve.adapter_options
    assert "verbose" in options and options["verbose"] == False
```

## 実験をForkする

{py:class}`~ommx.experiment.Experiment` は一度保存すると変更できなくなりますが、保存されたExperimentを元にして新しいExperimentを開始することができます。この操作を *Fork* と呼びます。ForkされたExperimentは元のExperimentと同じ情報を引き継いでいますが再び終了処理前の実行中の状態から始まるので、新たなRunやAttachmentを追加することができます。Forkは {py:meth}`~ommx.experiment.Experiment.fork` で行います。

```{code-cell} ipython3
with loaded_experiment.fork() as forked_experiment:
    # Fork先のExperimentは既存のRunを引き継いでいるので、新しいRun IDは2から始まる
    with forked_experiment.run() as run:
        assert run.run_id == 2

        c = 64
        instance = pi.with_parameters({capacity.id: c})

        run.log_parameter("capacity", c)
        solution = run.log_solve(OMMXHighsAdapter, instance, verbose=False)
        assert solution.feasible
        run.log_parameter("objective", solution.objective)
```

Fork元のExperimentは変更されません。一方、Fork先のExperimentには元のRunに加えて新しく追加したRunが含まれます。

```{code-cell} ipython3
assert list(loaded_experiment.run_parameters_df().index) == [0, 1]
assert list(forked_experiment.run_parameters_df().index) == [0, 1, 2]

forked_df = forked_experiment.run_parameters_df()
assert forked_df.loc[2, "capacity"] == 64
```

ForkされたExperimentはSolveやAttachmentのデータを引き継ぎますが、データはLocal Registryにデータの内容に基づいて保存されているので、Forkしてもデータが複製されるわけではありません。複製されるのはデータの一覧をまとめたArtifact Manifestだけで、Fork先のExperimentはFork元のExperimentと同じデータを指すようになります。

ForkされたExperimentを {py:meth}`~ommx.experiment.Experiment.save` や {py:meth}`~ommx.experiment.Experiment.push` で共有すると、共有されるのはFork後のExperiment全体です。元のExperiment由来のAttachment、Run、SolveもFork先のArtifactの `layers` に含まれるため、Fork後のExperimentを読み出すだけであれば元のExperimentは必要ありません。
