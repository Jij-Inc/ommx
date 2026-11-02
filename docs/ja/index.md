# OMMXとは？

OMMX（Open Mathematical prograMming eXchange、オミキス）を使うと、**何を解きたいか**と**どう解くか**を分けて扱えます。ソルバーに渡す定式化を変えても、得られた候補は元の問題の制約と目的関数に照らして評価できます。

モデラーは利用者の要件を問題として記述します。OMMXは共通のデータ形式とPython・RustのSDKを提供し、問題の変換と候補の評価を担います。Adapterはこの共通表現を、候補を探索するソルバーやサンプラーにつなぎます。共通表現に対して実装した変換は、複数のモデラーや解法から再利用できます。

## いまOMMXを使う理由

3.0.0 stable releaseまでは、このBookとAPI Referenceはともに`main`の内容を説明します。[インストール手順](./getting_started/install.md)には、最新の公開prereleaseと`main`からの導入方法を併記しています。未リリースの機能を使う場合は`main`を選んでください。現在の実装では、次の作業を共通の問題表現に対して行えます。

| 利用者の目的 | OMMXに任せられること | 利用者が決めること |
|---|---|---|
| 定式化やソルバーを変えて試す | 対応する特殊制約の変換や整数の符号化、Adapterを介した受け渡し | 使う解法、許可する変換、求解の設定 |
| 手元の候補が使えるか確かめる | 問題の定義域・制約・目的関数に基づく評価。対応する変換では、保持した情報による元の変数値の復元 | 業務上必要な条件のモデル化と、得られた候補を採用する判断 |
| 問題や結果を別の処理へ渡す | 共通形式での読み書き、Artifactによる保存・共有、Experimentによる条件と結果の記録 | 何を記録し、何と比較するか |

モデルが満たすべき要件は、AIをモデル作成に使う場合も利用者が決めます。自作アルゴリズムの候補を確かめたいときは、SDKの評価機能だけでも使えます。

## 手元の候補を評価する

ある生産ラインは通常運転では毎時100個まで、低速運転では毎時60個まで生産できます。低速運転で毎時65個を生産する計画は、ソルバーが返した候補でも、担当者が作った計画でも、この上限に違反します。OMMXはどちらも同じ問題に照らして評価できます。[候補を評価して結果を判断する](./user_guide/evaluate_and_interpret.md)では、この条件を満たすこと、モデル全体で実行可能なこと、最適性が示されることを区別します。候補を評価して結果を取り出す実行例は{class}`SolutionのReference <ommx.Solution>`にあります。

## 問題の意味と、解くための表現を分ける

低速運転時の上限は、二値変数が指定された値のときに条件を課す **Indicator制約** で表せます。この線形のIndicator制約を{class}`PySCIPOpt Adapter <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>`からSCIPに直接渡すことも、生産量の既知の上限を使って通常の不等式に変換し、同じSCIPで試すこともできます。どちらを選んでも、生産条件は一度記述すれば済みます。[問題を変換して解法へ渡す](./user_guide/transform_and_solve.md)では、この不等式を導き、別の変換を選ぶときに確認すべき保証を説明します。

## 共通の表現の上に、変換の技術を蓄積する

前処理や変換の技術を共通の表現に対して実装すれば、モデラーや解法ごとに同じ処理を作り直さずに活用できます。新しい解法も、問題の受け取り方と結果の返し方を接続することで、蓄積された共通処理を利用できます。このため、OMMXではデータの交換と、それに対する共通処理の両方を整備します。

現在は、対応する変換とその適用条件を選び、候補を評価するための部品を提供しています。例えば制約から変数の範囲を絞る処理もありますが、[一回の更新を行う操作](./how_to/tighten_bounds.md)であり、前処理全体を自動で最適化する仕組みではありません。

今後は、こうした前処理や変換を組み合わせ、問題の構造に応じて選択・反復する仕組みを育てていくことを目指します。個々の技術だけでなく、その使い分けも蓄積し、人が個別に行ってきた準備をより多く共通基盤に任せられるようにする方向です。

## 目的に合わせて読み始める

| やりたいこと | 最初に読むページ | この段階で学ぶこと |
|---|---|---|
| ソルバーで問題を解く | {class}`PySCIPOpt AdapterのReference <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>` | Instanceを作り、求解して結果を読む |
| 手元の問題と候補解を評価する | [候補を評価して結果を判断する](./user_guide/evaluate_and_interpret.md) | 評価する問題・目的・条件を揃え、実行可能性と最適性を区別する |
| 目的関数以外の指標を評価する | {class}`~ommx.NamedFunction` | 費用とは別の距離などを候補ごとに計算する |
| 結果を名前と添字で取り出す | {meth}`名前付きの変数値の抽出 <ommx.Solution.extract_decision_variables>` | 変数値を後処理用の辞書や表にする |
| OpenJijでサンプリングする | {class}`OpenJij SA AdapterのReference <ommx_openjij_adapter.OMMXOpenJijSAAdapter>` | 重みを設定して問題を準備し、候補を目的値と実行可能性で読む |
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

問題と候補の値がすでにある人は、[候補を評価して結果を判断する](./user_guide/evaluate_and_interpret.md)から始められます。具体的な評価の呼び出し方は{class}`~ommx.Solution`、ファイルから読み込む例は[候補評価のTutorial](./tutorial/evaluate_existing_candidate)にあります。候補評価はAdapterから独立して使えます。

## Adapterを使う・作る

まず[Python SDKとAdapterのインストール](./getting_started/install.md)で実行環境を用意し、使いたいAdapterのReferenceで完全な使用例を選びます。{class}`PySCIPOpt <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>`にはナップサック問題の求解、特殊制約の直接利用、明示的なPreparationの例があります。{class}`HiGHS <ommx_highs_adapter.OMMXHighsAdapter>`では、HiGHSが直接受け取れない特殊制約を通常の制約で表す例を扱います。{class}`OpenJij SA <ommx_openjij_adapter.OMMXOpenJijSAAdapter>`では、利用者が重みを選び、samplingして候補を選ぶ例を扱います。各例はモデルをその場で作ります。ほかの接続先は{doc}`Adapterの一覧と入力契約 </api/adapters>`から探せます。

同じ問題を複数ソルバーで解きたい場合は、API Referenceの{mod}`Adapter比較の完全な使用例 <ommx.adapter>`へ進んでください。[OpenJijのTSP Notebook](./tutorial/tsp_sampling_with_openjij_adapter)は、経路の描画まで扱う任意の応用例です。

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
