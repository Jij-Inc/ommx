# OMMXとは？

OMMX（Open Mathematical prograMming eXchange; オミキス）は、**ソルバーから独立して、最適化問題とその候補解の意味を定義する**オープンなプロジェクトです。問題を記述するデータ形式と、それを構築・評価・変換・保存するSDK、ソルバーにつなぐAdapterを提供します。

最適化の結果を理解するには、「変数にどの値が入ったか」だけでは不十分です。変数が取りうる値と満たすべき制約、比較する目的値、変換後の結果を元の問題でどう解釈するかが必要です。OMMXは、その基準を特定のソルバーの内部表現に委ねずに扱います。

## 手元の候補を評価する

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

候補を探索して解を求めたい場合は、同じ問題をAdapter経由でソルバーへ渡せます。評価で「この候補は実行可能か」を確認することと、求解で「より良い候補を探す」ことは別の操作です。

## 問題の意味と、解くための表現を分ける

ソルバーごとに、直接扱える目的関数や制約は異なります。OMMXは、問題を最初から一つのソルバーの表現に固定するのではなく、必要に応じて別の表現へ変換します。

例えば「いくつかの変数のうち、ちょうど一つを選ぶ」という条件を、対応するソルバーにはそのまま渡し、必要なら通常の等式へ変換して渡せます。この条件そのものの意味はOMMXが定義します。変換後も、元の変数や制約で結果を確認するための情報を保持できます。

ただし、変換が何を保存するかは操作ごとに異なります。同じ条件を別の式で表す場合もあれば、課す条件や比較する目的値を変える場合もあります。変換前後の実行可能性・目的値・最適性を、すべて同じ保証として扱うことはできません。具体的な条件の例は[特殊制約](./user_guide/special_constraints.md)、変換した問題の結果の読み方は[変換後の問題とRemoved constraints](./user_guide/removed_constraints.md)で説明します。

このソルバーに依存しない問題記述は、将来、勤務パターンのような業務上の条件を直接記述したり、意味が変わらないことを確かめながら問題を単純化したりする土台にもなります。これらは将来の方向性であり、現在の対応機能を示すものではありません。

## 目的に合わせて読み始める

| やりたいこと | 最初に読むページ | この段階で学ぶこと |
|---|---|---|
| ソルバーで問題を解く | [Adapterで解くTutorial](./tutorial/solve_with_ommx_adapter) | Instanceを作り、求解して結果を読む |
| 独自のAdapterを実装する | [Adapter Developer Guide](./developer_guide/adapter.md) | 入力・変換・評価の契約と参照実装 |
| 問題や候補解の意味を理解する | [Instance](./user_guide/instance.md) → [StateからSolutionへの評価](./user_guide/solution.md) | 変数の役割、実行可能性、目的値、最適性の違い |
| 特殊制約を使う | [特殊制約の意味と評価](./user_guide/special_constraints.md) | Indicator・OneHot・SOS1の成立条件 |
| 問題を変換し、結果を確認する | [Instanceを変換するHow-to](./how_to/transform_instance_without_solver.md) | target classとPolicyを指定し、変換結果と手元の候補を検査する |
| 変換後の結果を解釈する | [変換後の問題とRemoved constraints](./user_guide/removed_constraints.md) | activeな問題と、元の条件を使った評価の違い |
| 複数の候補を比較する | [SampleSet](./user_guide/sample_set.md) | サンプルごとの評価と、得られた候補の中からの選択 |
| 既存の問題を入手する | [MIPLIB](./tutorial/download_miplib_instance.md) / [QPLIB](./tutorial/download_qplib_instance.md) | 公開された最適化問題の取得 |
| 実験を記録・比較する | [実験管理Tutorial](./tutorial/experiment_management.md) | 条件と結果を対応づけた記録 |
| 問題や結果を保存・共有する | [Artifact共有Tutorial](./tutorial/share_in_ommx_artifact.md) | メタデータとともに受け渡す |

Tutorialは小さな成功例から操作を学ぶためのものです。User Guideは仕組みや意味を理解するために、API Referenceは個々の操作の引数・前提条件・保証を確認するために使います。全ページを順に読む必要はありません。

## Adapterを使う・作る

Adapterを使って解きたい人は、まず[Python SDKとAdapterのインストール](./getting_started/install.md)で実行環境を用意してください。次の3つのTutorialで、変換なしの求解からOMMXによるPreparationへ進めます。

1. [PySCIPOpt Adapterで0-1ナップサック問題を解く](./tutorial/solve_with_ommx_adapter)：Instanceを作り、`solve()`を呼び、pandasで結果を読む。
2. [PySCIPOpt Adapterで特殊制約をそのまま解く](./tutorial/solve_special_constraints_with_pyscipopt_adapter.md)：IndicatorとSOS1の意味を知り、対応ソルバーでそのまま解く。
3. [Adapter向けにInstanceを準備する](./tutorial/prepare_instance_for_adapter.md)：特殊制約を直接扱わないHiGHSへ、loweringを経て同じモデルを渡す。

独自の変換方針が必要なら[Preparationで変換する制約族を選ぶ](./how_to/customize_preparation.md)、同じ問題を複数ソルバーで解きたいなら[複数のAdapterで結果を比較する](./how_to/compare_adapters.md)へ進んでください。サンプリングの応用例として、[OpenJij AdapterでTSPを扱うTutorial](./tutorial/tsp_sampling_with_openjij_adapter)もあります。これは上記3つのTutorialの必須の続きではありません。

Adapterを実装する人は、利用者向けTutorialの通読ではなく、[Developer Guide](./developer_guide/adapter.md)から共通の入力・変換・評価の契約と参照実装を確認してください。

## データの交換と共有を支える仕組み

問題や解に共通の意味を与えることで、ソフトウェア間だけでなく、担当者間でも同じ問題と結果を扱えます。OMMXは次の要素でその受け渡しを支えます。

| 要素 | 役割 |
|---|---|
| OMMX Message | 問題や解を表す、言語・OSに依存しないデータ形式 |
| OMMX SDK | 問題と結果の構築・評価・変換・読み書き |
| OMMX Adapter | OMMXと個々のソルバーの入力・出力の接続 |
| OMMX Artifact | 問題・結果・添付データをメタデータ付きで保存・共有するパッケージ |

Messageのスキーマは[Protocol Buffers](https://protobuf.dev/)で定義され、PythonとRustのSDKを提供しています。Pythonのユーザーコードでは `from ommx import Instance` のように公開APIを使います。wire formatのスキーマは[OMMX Message Schema](https://jij-inc.github.io/ommx/protobuf.html)を参照してください。

```{figure} ./assets/introduction_02.png
:alt: 共通のOMMX表現を介して複数のソフトウェアを接続する図
:width: 70%

共通の問題・結果表現を介して、ソフトウェアを接続する。
```

Artifactには、問題と結果に加えて「何を、どの設定で解いたか」を記録できます。[OCI Artifact](https://opencontainers.org/)を利用しているため、GitHub Container Registryなど既存のregistryで配布することもできます。OMMXはこの仕組みでMIPLIBやQPLIBの問題を公開しています。

```{toctree}
:caption: Getting Started
:maxdepth: 1
:hidden:

getting_started/install
```

```{toctree}
:caption: Tutorial
:maxdepth: 1
:hidden:

tutorial/solve_with_ommx_adapter
tutorial/solve_special_constraints_with_pyscipopt_adapter
tutorial/prepare_instance_for_adapter
tutorial/tsp_sampling_with_openjij_adapter
tutorial/experiment_management
tutorial/share_in_ommx_artifact
tutorial/download_miplib_instance
tutorial/download_qplib_instance
```

```{toctree}
:caption: How-to Guide
:maxdepth: 1
:hidden:

how_to/customize_preparation
how_to/compare_adapters
how_to/transform_instance_without_solver
```

```{toctree}
:caption: User Guide：問題と評価
:maxdepth: 1
:hidden:

user_guide/instance
user_guide/solution
user_guide/sample_set
user_guide/function
user_guide/parametric_instance
user_guide/special_constraints
user_guide/removed_constraints
```

```{toctree}
:caption: User Guide：変換とソルバー接続
:maxdepth: 1
:hidden:

user_guide/instance_class
user_guide/preparation_policy
user_guide/supported_ommx_adapters
user_guide/adapter_diagnostics
user_guide/adapter_initial_state
```

```{toctree}
:caption: User Guide：記録と運用
:maxdepth: 1
:hidden:

user_guide/experiment
user_guide/tracing
user_guide/cli
user_guide/agent_skills
```

```{toctree}
:caption: Developer Guide
:maxdepth: 1
:hidden:

developer_guide/adapter
```

```{toctree}
:caption: API Reference
:maxdepth: 2
:hidden:

api/index
```

```{toctree}
:caption: Migration Guide
:maxdepth: 1
:hidden:

migration/python_sdk_v2_to_v3
migration/python_sdk_v1_to_v2
```

```{toctree}
:caption: Release Note
:maxdepth: 1
:hidden:

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
