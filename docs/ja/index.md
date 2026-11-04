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

問題はモデラーや保存済みファイルから受け取ることも、SDKで作ることもできます。同じモデル構造に異なる入力値を当てはめる場合は、{class}`~ommx.ParametricInstance`でケースごとにパラメータを指定できます。

モデルが満たすべき要件は、AIをモデル作成に使う場合も利用者が決めます。自作アルゴリズムの候補を確かめたいときは、SDKの評価機能だけでも使えます。

## 手元の候補を評価する

ある生産ラインは通常運転では毎時100個まで、低速運転では毎時60個まで生産できます。低速運転で毎時65個を生産する計画は、ソルバーが返した候補でも、担当者が作った計画でも、この上限に違反します。OMMXはどちらも同じ問題に照らして評価できます。[候補を評価して結果を判断する](./user_guide/evaluate_and_interpret.md)では、この条件を満たすこと、モデル全体で実行可能なこと、最適性が示されることを区別します。候補を評価して結果を取り出す実行例は{class}`SolutionのReference <ommx.Solution>`にあります。

## 問題の意味と、解くための表現を分ける

低速運転時の上限は、二値変数が指定された値のときに条件を課す **Indicator制約** で表せます。この線形のIndicator制約を{class}`PySCIPOpt Adapter <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>`からSCIPに直接渡すことも、生産量の既知の上限を使って通常の不等式に変換し、同じSCIPで試すこともできます。どちらを選んでも、生産条件は一度記述すれば済みます。[問題を変換して解法へ渡す](./user_guide/transform_and_solve.md)では、この不等式を導き、別の変換を選ぶときに確認すべき保証を説明します。

## 共通の表現の上に、変換の技術を蓄積する

前処理や変換の技術を共通の表現に対して実装すれば、モデラーや解法ごとに同じ処理を作り直さずに活用できます。新しい解法も、問題の受け取り方と結果の返し方を接続することで、蓄積された共通処理を利用できます。このため、OMMXではデータの交換と、それに対する共通処理の両方を整備します。

現在は、対応する変換とその適用条件を選び、候補を評価するための部品を提供しています。例えば制約から変数の範囲を絞る処理もありますが、{meth}`一回の更新を行う操作 <ommx.Instance.tighten_bounds_simultaneously_once>`であり、前処理全体を自動で最適化する仕組みではありません。

今後は、こうした前処理や変換を組み合わせ、問題の構造に応じて選択・反復する仕組みを育てていくことを目指します。個々の技術だけでなく、その使い分けも蓄積し、人が個別に行ってきた準備をより多く共通基盤に任せられるようにする方向です。

## 目的に合わせて読み始める

この導入ではOMMXを採用する理由を説明しました。Bookの残りの3章は、それぞれの章から直接読み始められます。

| 判断したいこと | Bookの章 |
|---|---|
| このモデルをソルバーに直接渡せるか。定式化を変えるなら、どの性質を保つ必要があるか | [問題を変換して解法へ渡す](./user_guide/transform_and_solve.md) |
| 得られた候補は元の条件を満たすか。ソルバーが報告する目的値やboundから何が言えるか | [候補を評価して結果を判断する](./user_guide/evaluate_and_interpret.md) |
| 別の人が結果を調べたり、試行を再実行したりするには、何を残す必要があるか | [問題・結果・試行を保存して共有する](./user_guide/record_and_share.md) |

実行環境を用意するには、[SDKと必要なAdapterのインストール](./getting_started/install.md)から始めます。題材を通して試したい場合は、[公開ベンチマークを解いて結果を照合する例](./examples/published_benchmarks.md)や[巡回セールスマン問題の経路をサンプリングして描画する例](./tutorial/tsp_sampling_with_openjij_adapter)があります。

個々の操作は{doc}`Python API Reference <api/index>`から調べられます。例えば、候補の値と制約の違反を調べる例は{class}`~ommx.Solution`、ファイルから候補を読む例は{func}`~ommx.qplib.load_solution`にあります。{doc}`Adapterの一覧 <api/adapters>`から、各ソルバーの入力条件と求解例に進めます。自分の解法を接続する場合は{class}`~ommx.adapter.SolverAdapter`または{class}`~ommx.adapter.SamplerAdapter`を参照してください。

探索の進捗や終了理由を調べる手順は、{class}`SCIP <ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer>`と{class}`HiGHS <ommx_highs_adapter.HighsDiagnosticsAnalyzer>`の診断情報のReferenceにあります。

保存したデータを扱うには、[手元のArtifactを調べる手順](./how_to/manage_local_registry.md)や[レジストリで共有する手順](./how_to/share_artifact_registry.md)に進みます。外部ツールを接続するには、[エージェントプラグインの導入](./user_guide/agent_skills.md)や[OpenTelemetryコレクターへの送信設定](./how_to/export_traces.md)を参照してください。

(exchange-formats)=
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
:caption: OMMXを使う目的と判断
:maxdepth: 1
:hidden:

user_guide/transform_and_solve
user_guide/evaluate_and_interpret
user_guide/record_and_share
```

```{toctree}
:caption: 実行環境を用意する
:maxdepth: 1
:hidden:

getting_started/install
```

```{toctree}
:caption: 題材を通して試す
:maxdepth: 1
:hidden:

examples/published_benchmarks
tutorial/tsp_sampling_with_openjij_adapter
```

```{toctree}
:caption: Artifactを調べて共有する
:maxdepth: 1
:hidden:

how_to/manage_local_registry
how_to/share_artifact_registry
```

```{toctree}
:caption: 外部ツールを接続する
:maxdepth: 1
:hidden:

user_guide/agent_skills
how_to/export_traces
```

```{toctree}
:caption: API・形式仕様を調べる
:maxdepth: 1
:hidden:

api/index
Rust API（公開版） <https://docs.rs/ommx/>
Message形式のスキーマ <https://github.com/Jij-Inc/ommx/tree/main/proto/ommx>
Artifact形式仕様 <https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/artifact_design.md>
```

```{toctree}
:caption: SDKを移行する
:maxdepth: 1
:hidden:

migration/python_sdk_v2_to_v3
migration/python_sdk_v1_to_v2
```

```{toctree}
:caption: リリースごとの変更を確認する
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
