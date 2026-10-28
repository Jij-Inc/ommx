# OMMXとは？

OMMX（Open Mathematical prograMming eXchange; オミキス）は、**ソルバーから独立して、
最適化問題と候補解の意味を定義し、その上で使える操作を共通化する**オープンなプロジェクトです。
問題の記述・評価・変換・値の復元と、実行backendへの接続を、一つの共通モデルで扱います。

モデラーやAdapterが共通化できる仕事と、それぞれの責任は[OMMXで何を共通化できるか](getting_started/overview.md)で説明します。
[実行環境とバージョン](getting_started/compatibility.md)から、必要なものだけ用意してください。

## 目的に合わせて読み始める

| やりたいこと | 最初の入口と、ここまででできること |
|---|---|
| 問題を用意して解く・サンプリングする | [小さな問題を作って解く](tutorial/solve_with_ommx_adapter)、[既存問題を読み込む](how_to/load_existing_instance.md)、または[使うAdapterを選ぶ](user_guide/supported_ommx_adapters.md)。既知のAdapterの手順へ直接進めます |
| 手元の問題と候補解を評価する | [既存の問題と候補を評価する](tutorial/evaluate_existing_candidate)。ファイルから読み、目的値・実行可能性・満たさない条件を調べます |
| 問題を変換・検査する | [Instanceを変換し、結果を確認する](how_to/transform_instance_without_solver.md)。表現条件とPolicyを指定し、実行backendを呼ばずに変換・評価できます |
| 実験を記録する・データを受け渡す | [Experimentで試行を記録する](tutorial/experiment_management)、または[Artifactでファイルを受け渡す](tutorial/share_in_ommx_artifact.md)。目的に合う方だけ選べます |
| ソルバーや別のツール・言語を接続する | [Adapterを実装する](developer_guide/adapter.md)、または[別ツール・言語を接続する](developer_guide/integration.md)。共通概念からRust APIや形式の仕様へ進めます |

Tutorialは一つの小さな成功を、How-toは独立した作業を扱います。Guideは共通概念と結果の
読み方、Referenceは個別操作の正確な条件・数式・エラーを扱います。全ページを順番に読む
必要はありません。ArtifactとExperimentも、問題の構築・評価・変換・実行の前提ではありません。

## 候補を評価することと、探すことは別

例えば、二つの項目を同時には選べない問題を記述して、手元の候補を検査できます。

```python
from ommx import Instance

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = x + y
instance.add_constraint(x * y == 0)

solution = instance.evaluate({x.id: 1, y.id: 0})
assert solution.feasible
assert solution.objective == 1
```

`Instance`が問題、値の割り当てが候補、`Solution`がその評価結果です。より良い候補を
探索する場合は、Adapterを介してbackendへ渡します。実行可能性、目的値、最適性は別々に
読みます。[SolutionのGuide](user_guide/solution.md)が共通の評価モデルを説明し、
[SampleSet](user_guide/sample_set.md)は複数候補の扱いを説明します。

## 必要な表現へ変換して使う

backendごとに、直接扱える式・変数・制約は異なります。OMMXでは条件の意味を保つ共通の
問題記述と、実行先へ渡す表現を分けます。例えば「ちょうど一つを選ぶ」という特殊制約を、
対応backendではその形で使い、必要なら通常の等式へ変換できます。

Adapterを初めて使う場合は、次の小さな例で段階的に学べます。

1. [PySCIPOptで変換なしに解く](tutorial/solve_with_ommx_adapter)。
2. [Indicator・SOS1をそのまま解く](tutorial/solve_special_constraints_with_pyscipopt_adapter.md)。
3. [特殊制約を直接扱わないHiGHSへPreparationして渡す](tutorial/prepare_instance_for_adapter.md)。

この順序はすべての読者の前提ではありません。OpenJijを使いたい場合は、
[小さな問題をサンプリングするHow-to](how_to/sample_with_openjij_adapter)へ直接進めます。
[TSP](tutorial/tsp_sampling_with_openjij_adapter)は任意の応用例です。

変換の成功、元の変数値の復元、元の問題での実行可能性・最適性は同じ保証ではありません。
必要になった段階で[変換の保証](user_guide/transformation_guarantees.md)と
[変換後の結果](user_guide/removed_constraints.md)を確認してください。

```{toctree}
:caption: はじめに
:maxdepth: 1
:hidden:

getting_started/overview
getting_started/compatibility
getting_started/install
```

```{toctree}
:caption: Tutorial：問題を解く
:maxdepth: 1
:hidden:

tutorial/solve_with_ommx_adapter
tutorial/solve_special_constraints_with_pyscipopt_adapter
tutorial/prepare_instance_for_adapter
```

```{toctree}
:caption: Tutorial：候補を評価する
:maxdepth: 1
:hidden:

tutorial/evaluate_existing_candidate
```

```{toctree}
:caption: Tutorial：記録・受け渡し
:maxdepth: 1
:hidden:

tutorial/experiment_management
tutorial/share_in_ommx_artifact
```

```{toctree}
:caption: Guide：問題と評価
:maxdepth: 1
:hidden:

user_guide/instance
user_guide/function
user_guide/special_constraints
user_guide/solution
user_guide/sample_set
user_guide/variable_roles
user_guide/parametric_instance
```

```{toctree}
:caption: Guide：変換とソルバー接続
:maxdepth: 1
:hidden:

user_guide/transformation_guarantees
user_guide/removed_constraints
user_guide/instance_class
user_guide/preparation_policy
user_guide/supported_ommx_adapters
```

```{toctree}
:caption: Guide：記録と共有
:maxdepth: 1
:hidden:

user_guide/experiment
user_guide/artifact
```

```{toctree}
:caption: How-to：問題の準備
:maxdepth: 1
:hidden:

how_to/load_existing_instance
how_to/transform_instance_without_solver
how_to/customize_preparation
```

```{toctree}
:caption: How-to：Adapterの利用と結果の調査
:maxdepth: 1
:hidden:

how_to/sample_with_openjij_adapter
how_to/compare_adapters
how_to/inspect_adapter_diagnostics
how_to/use_initial_state
how_to/trace_execution
tutorial/tsp_sampling_with_openjij_adapter
```

```{toctree}
:caption: How-to：記録したデータの運用
:maxdepth: 1
:hidden:

how_to/continue_experiment
how_to/share_artifact_registry
how_to/manage_local_registry
how_to/attach_experiment_data
```

```{toctree}
:caption: Developer Guide
:maxdepth: 1
:hidden:

developer_guide/adapter
developer_guide/integration
```

```{toctree}
:caption: Reference
:maxdepth: 2
:hidden:

api/index
```

```{toctree}
:caption: 移行・リリース情報
:maxdepth: 1
:hidden:

migration/python_sdk_v2_to_v3
migration/python_sdk_v1_to_v2
release_note/ommx-3.0
release_note/ommx-2.5
release_note/ommx-2.4
release_note/ommx-2.3
release_note/ommx-2.2
release_note/ommx-2.1
release_note/ommx-2.0
release_note/ommx-1.9
release_note/ommx-1.8
release_note/ommx-1.7
release_note/ommx-1.6
release_note/ommx-1.5
```

```{toctree}
:caption: 補助ツール
:maxdepth: 1
:hidden:

user_guide/agent_skills
```
