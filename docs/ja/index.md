# OMMXとは？

OMMX（Open Mathematical prograMming eXchange; オミキス）は、**数理最適化の問題を共通の表現で扱い、その上に前処理や問題変換の技術を蓄積・再利用できる基盤**を目指すオープンなプロジェクトです。モデラーとソルバーの間に共通の問題表現を置き、さまざまな解法で同じ変換や評価の仕組みを使えるようにします。

例えば、新しい解法を試すたびに、問題の変換や返された候補の検査まで作り直す必要があると、解法そのものの開発に集中できません。問題と候補解の意味をソルバーから独立して定めることで、こうした処理を共有できます。OMMXは、そのためのデータ形式、構築・評価・変換・保存を行うPython/Rust SDK、ソルバーにつなぐAdapterを提供します。

## いまOMMXを使う理由

3.0.0 stable releaseまでは、このBookとAPI Referenceはともに`main`の内容を説明します。[インストール手順](./getting_started/install.md)には、最新の公開prereleaseと`main`からの導入方法を併記しています。未リリースの機能を使う場合は`main`を選んでください。現在の実装では、次の作業を共通の問題表現に対して行えます。

| 利用者の目的 | OMMXに任せられること | 利用者が決めること |
|---|---|---|
| 別のソルバーや解法を試す | 対応する特殊制約の変換や整数の符号化、Adapterを介した受け渡し | 使う解法、許可する変換、求解の設定 |
| 手元の候補が使えるか確かめる | 問題の定義域・制約・目的関数に基づく評価。対応する変換では、保持した情報による元の変数値の復元 | 業務上必要な条件のモデル化と、得られた候補を採用する判断 |
| 問題や結果を別の処理へ渡す | 共通形式での読み書き、Artifactによる保存・共有、Experimentによる条件と結果の記録 | 何を記録し、何と比較するか |

一部の機能から使えます。自作アルゴリズムが返した候補をSDKで評価するだけなら、AdapterやArtifactは不要です。モデル化は業務上の要件を変数・目的・制約に落とす仕事、OMMXはその問題を扱う共通処理、ソルバーやサンプラーは候補を探す処理を担います。

## 手元の候補を評価する

最適化の結果を理解するには、「変数にどの値が入ったか」だけでは不十分です。変数が取りうる値と満たすべき制約、比較する目的値が必要です。例えば、二つの項目を同時には選べない問題を記述して、手元の候補を検査できます。

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

この候補は制約を満たし、目的値は1です。候補が自作アルゴリズム、ソルバー、人の手のどこから来ても、同じ問題に対して検査できます。評価で「この候補は実行可能か」を確認することと、求解で「より良い候補を探す」ことは別の操作です。評価しただけでは、最適な候補かどうかは分かりません。

## 問題の意味と、解くための表現を分ける

ソルバーごとに、直接扱える目的関数や制約は異なります。OMMXは、問題を最初から一つのソルバーの表現に固定するのではなく、必要に応じて別の表現へ変換します。

例えば、三つの0-1変数からちょうど一つを選ぶ条件は、OneHotという特殊制約で表せます。通常の線形制約が必要なら、`x[0] + x[1] + x[2] == 1`へ変換できます。最初から式だけにしてしまわず、「ちょうど一つを選ぶ」という構造を保持することで、その意味を後の分析や変換に利用できます。v3では特殊制約を通常の制約と同様に、構築・評価・変換の対象として扱います。

どの構造をそのまま扱えるかは解法ごとに異なります。例えばIndicatorやSOS1は、対応するモデルなら[PySCIPOpt Adapterでそのまま解く](./tutorial/solve_special_constraints_with_pyscipopt_adapter.md)ことも、変数の範囲などの適用条件を満たせば[通常制約へ変換してHiGHSに渡す](./tutorial/prepare_instance_for_adapter.md)こともできます。利用者が選んだ変換方針に従って、必要な段階で表現を変えます。

ただし、変換が何を保存するかは操作ごとに異なります。同じ条件を別の式で表す場合もあれば、課す条件や比較する目的値を変える場合もあります。変換前後の実行可能性・目的値・最適性を、すべて同じ保証として扱うことはできません。具体的な条件の例は{class}`~ommx.IndicatorConstraint`・{class}`~ommx.OneHotConstraint`・{class}`~ommx.Sos1Constraint`、変換した問題の結果の読み方は[候補を評価して結果を判断する](./user_guide/evaluate_and_interpret.md)で説明します。

## 共通の表現の上に、変換の技術を蓄積する

前処理や変換の技術を共通の表現に対して実装すれば、モデラーや解法ごとに同じ処理を作り直さずに活用できます。新しい解法も、問題の受け取り方と結果の返し方を接続することで、蓄積された共通処理を利用できます。このため、OMMXではデータの交換と、それに対する共通処理の両方を整備します。

現在は、対応する変換とその適用条件を選び、候補を評価するための部品を提供しています。例えば制約から変数の範囲を絞る処理もありますが、[一回の更新を行う操作](./how_to/tighten_bounds.md)であり、前処理全体を自動で最適化する仕組みではありません。

今後は、こうした前処理や変換を組み合わせ、問題の構造に応じて選択・反復する仕組みを育てていくことを目指します。個々の技術だけでなく、その使い分けも蓄積し、人が個別に行ってきた準備をより多く共通基盤に任せられるようにする方向です。

## 目的に合わせて読み始める

| やりたいこと | 最初に読むページ | この段階で学ぶこと |
|---|---|---|
| ソルバーで問題を解く | [Adapterで解くTutorial](./tutorial/solve_with_ommx_adapter) | Instanceを作り、求解して結果を読む |
| 手元の問題と候補解を評価する | [候補を評価して結果を判断する](./user_guide/evaluate_and_interpret.md) | 評価する問題・目的・条件を揃え、実行可能性と最適性を区別する |
| 目的関数以外の指標を評価する | {class}`~ommx.NamedFunction` | 費用とは別の距離などを候補ごとに計算する |
| 結果を名前と添字で取り出す | {meth}`名前付きの変数値の抽出 <ommx.Solution.extract_decision_variables>` | 変数値を後処理用の辞書や表にする |
| OpenJijでサンプリングする | [小さな問題をサンプリングするHow-to](./how_to/sample_with_openjij_adapter) | 重みを設定して問題を準備し、候補を目的値と実行可能性で読む |
| 独自のAdapterを実装する | [Adapter Developer Guide](./developer_guide/adapter.md) | 入力・変換・評価の契約と参照実装 |
| 問題や候補解の意味を理解する | {class}`~ommx.Instance` → {class}`~ommx.Solution` | 変数の役割、実行可能性、目的値、最適性の違い |
| 式を構築・評価する | {class}`~ommx.Function` | 式を作り、式の値と候補の実行可能性を区別する |
| 固定した構造の数値条件を変える | {class}`~ommx.ParametricInstance` | 構造と変数・制約のIDを再利用し、パラメータの値を変えたInstanceを作る |
| 特殊制約を使う | {class}`~ommx.IndicatorConstraint`・{class}`~ommx.OneHotConstraint`・{class}`~ommx.Sos1Constraint` | Indicator・OneHot・SOS1の成立条件 |
| 問題を変換し、解法を選ぶ | [問題を変換して解法へ渡す](./user_guide/transform_and_solve.md) | 構造を保つ理由と、入力条件・変換の保証・設定を分ける |
| 制約から変数の上下限を絞る | [bound更新のHow-to](./how_to/tighten_bounds.md) | 一回の更新と、変更された変数を確認する |
| 式を変数名で確認する | {meth}`ラベルを使った式の表示 <ommx.Instance.format_function>` | Instanceの名前・添字を使って式を読む |
| 変換後の結果を解釈する | [候補を評価して結果を判断する](./user_guide/evaluate_and_interpret.md) | activeな問題と、元の条件を使った評価の違い |
| 複数の候補を比較する | {class}`~ommx.SampleSet` | サンプルごとの評価と、得られた候補の中からの選択 |
| 既存の問題を入手する | [既存の問題をInstanceとして読み込む](./how_to/load_existing_instance.md) | MIPLIB/QPLIBや手元のファイルから問題を読み込む |
| 外部ツールへモデルを渡す | {meth}`~ommx.Instance.save_mps` | 出力できるモデルと、OMMXとして保存する場合との違い |
| 問題・結果・試行を保存して共有する | [記録する内容と方法を選ぶ](./user_guide/record_and_share.md) | 受け渡しと、後で結果や条件を調べるために残す情報 |

このBookでは、何ができるか、なぜ・いつ使うか、何を選び、結果をどう読むかを説明します。小さな例で意味をつかみ、実装に必要な詳しい呼び出し方や引数・前提条件・保証は{doc}`API Reference <api/index>`で確認してください。全ページを順に読む必要はありません。

問題と候補の値がすでにある人は、[候補を評価して結果を判断する](./user_guide/evaluate_and_interpret.md)から始められます。具体的な評価の呼び出し方は{class}`~ommx.Solution`、ファイルから読み込む例は[候補評価のTutorial](./tutorial/evaluate_existing_candidate)にあります。Adapter Tutorialの実行は前提ではありません。

## Adapterを使う・作る

Adapterを使って解きたい人は、まず[Python SDKとAdapterのインストール](./getting_started/install.md)で実行環境を用意してください。次の3つのTutorialで、変換なしの求解からOMMXによるPreparationへ進めます。

1. [PySCIPOpt Adapterで0-1ナップサック問題を解く](./tutorial/solve_with_ommx_adapter)：Instanceを作り、`solve()`を呼び、pandasで結果を読む。
2. [PySCIPOpt Adapterで特殊制約をそのまま解く](./tutorial/solve_special_constraints_with_pyscipopt_adapter.md)：IndicatorとSOS1の意味を知り、対応ソルバーでそのまま解く。
3. [Adapter向けにInstanceを準備する](./tutorial/prepare_instance_for_adapter.md)：特殊制約を直接扱わないHiGHSへ、loweringを経て同じモデルを渡す。

独自の変換方針が必要なら{class}`明示的なPreparationと求解の例 <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>`、同じ問題を複数ソルバーで解きたいなら[複数のAdapterで結果を比較する](./how_to/compare_adapters.md)へ進んでください。

OpenJijを使いたい人は、上記3つのTutorialを実行せず、[OpenJij Adapterで小さな問題をサンプリングする](./how_to/sample_with_openjij_adapter)から始められます。[TSPのNotebook](./tutorial/tsp_sampling_with_openjij_adapter)は任意の応用例です。

Adapterを実装する人は、利用者向けTutorialの通読ではなく、[Developer Guide](./developer_guide/adapter.md)から共通の入力・変換・評価の契約と参照実装を確認してください。

## データの交換と共有を支える仕組み

問題や解に共通の意味を与えることで、ソフトウェア間だけでなく、担当者間でも同じ問題と結果を扱えます。OMMXは次の要素でその受け渡しを支えます。

| 要素 | 役割 |
|---|---|
| OMMX Message | 問題や解を表す、言語・OSに依存しないデータ形式 |
| OMMX SDK | 問題と結果の構築・評価・変換・読み書き |
| OMMX Adapter | OMMXと個々のソルバーの入力・出力の接続 |
| OMMX Artifact | 問題・結果・添付データをメタデータ付きで保存・共有するパッケージ |

Messageのスキーマは[Protocol Buffers](https://protobuf.dev/)で定義され、PythonとRustのSDKを提供しています。Pythonのユーザーコードでは `from ommx import Instance` のように公開APIを使います。現行のwire formatの定義は[スキーマのソース](https://github.com/Jij-Inc/ommx/tree/main/proto/ommx)、v1の生成済み一覧は[OMMX v1 Message Schema](https://jij-inc.github.io/ommx/protobuf.html)を参照してください。

```{figure} ./assets/introduction_02.png
:alt: 共通のOMMX表現を介して複数のソフトウェアを接続する図
:width: 70%

共通の問題・結果表現を介して、ソフトウェアを接続する。
```

Artifactには、問題と結果に加えて「何を、どの設定で解いたか」を記録できます。[OCI Artifact](https://opencontainers.org/)を利用しているため、GitHub Container Registryなど既存のregistryで配布することもできます。OMMXはこの仕組みでMIPLIBやQPLIBの問題を公開しています。形式を直接読み書きするツール向けのmanifest・layer・media typeの定義は、[Artifact形式仕様](https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/artifact_design.md)を参照してください。

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
```

```{toctree}
:caption: Tutorial：候補を評価する
:maxdepth: 1
:hidden:

tutorial/evaluate_existing_candidate
```

```{toctree}
:caption: How-to Guide
:maxdepth: 1
:hidden:

how_to/compare_adapters
how_to/sample_with_openjij_adapter
how_to/load_existing_instance
how_to/tighten_bounds
how_to/share_artifact_registry
```

```{toctree}
:caption: 問題を変換して解法へ渡す
:maxdepth: 1
:hidden:

user_guide/transform_and_solve
```

```{toctree}
:caption: 候補を評価して結果を判断する
:maxdepth: 1
:hidden:

user_guide/evaluate_and_interpret
```

```{toctree}
:caption: User Guide：変換とソルバー接続
:maxdepth: 1
:hidden:

how_to/inspect_adapter_diagnostics
how_to/use_initial_state
```

```{toctree}
:caption: 問題・結果・試行を保存して共有する
:maxdepth: 1
:hidden:

user_guide/record_and_share
```

```{toctree}
:caption: User Guide：記録と運用
:maxdepth: 1
:hidden:

how_to/trace_execution
how_to/manage_local_registry
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
