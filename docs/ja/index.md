# OMMXとは？

OMMX（Open Mathematical prograMming eXchange、オミキス）を使うと、**何を解きたいか**と**どう解くか**を分けて扱えます。ソルバーに渡す定式化を変えても、得られた候補は元の問題の制約と目的関数に照らして評価できます。結果を元の条件に結びつけて理解することが、要求や入力データの見直しにつながります。

## モデラー・OMMX・Adapterの責務

モデラーは利用者の要件を問題として記述します。OMMXは共通のデータ形式とPython・RustのSDKを提供し、問題の変換と候補の評価を担います。Adapterの責務は、この共通表現をソルバーやサンプラーへ意味を保って接続し、候補や終了報告を実際に解いた問題に結びつけて解釈することです。共通表現に対して実装した変換は、複数のモデラーや解法から再利用できます。

この責務の分担は、OMMXとしての契約です。個々の実装の対応範囲にかかわらず、Adapterには{ref}`この契約 <adapter-responsibilities>`を求めます。利用できる機能や対応状況は機能・Adapterごとに異なるため、詳細は{doc}`API Reference <api/index>`で確認してください。

## OMMXを使う理由

共通の問題表現を使うことで、次のように処理を分担できます。

| 利用者の目的 | OMMXが担う処理 | 利用者が決めること |
|---|---|---|
| 定式化やソルバーを変えて試す | 対応する特殊制約の変換や整数の符号化、Adapterを介した受け渡し | 使う解法、許可する変換、求解の設定 |
| 手元の候補が使えるか確かめる | 問題の定義域・制約・目的関数に基づく評価。対応する変換では、保持した情報による元の変数値の復元 | 業務上必要な条件のモデル化と、得られた候補を採用する判断 |
| 結果から要求や入力データを見直す | ラベルによる対象の識別と、特殊制約から生成した通常制約の来歴の保持 | 元の宣言・データとの対応をモデラーやアプリケーションで管理し、何を見直すか決める |
| 問題や結果を別の処理へ渡す | 共通形式での読み書き、Artifactによる保存・共有、Experimentによる条件と結果の記録 | 何を記録し、何と比較するか |

問題はモデラーや保存済みファイルから受け取ることも、SDKで作ることもできます。同じモデル構造に異なる入力値を当てはめる場合は、{class}`~ommx.ParametricInstance`でケースごとにパラメータを指定できます。

モデルが満たすべき要件は、AIをモデル作成に使う場合も利用者が決めます。自作アルゴリズムの候補を確かめたいときは、SDKの評価機能だけでも使えます。

## 手元の候補を評価する

ある生産ラインは通常運転では毎時100個まで、低速運転では毎時60個まで生産できます。低速運転で毎時65個を生産する計画は、ソルバーが返した候補でも、担当者が作った計画でも、この上限に違反します。OMMXはどちらも同じ問題に照らして評価できます。[候補を評価して結果を判断する](./book/evaluate_and_interpret.md)では、この条件を満たすこと、モデル全体で実行可能なこと、最適性が示されることを区別します。さらに、候補が返らない場合の結論を読み、結果を元の条件やデータへ対応づけて見直す考え方を説明します。候補を評価して結果を取り出す実行例は{class}`SolutionのReference <ommx.Solution>`にあります。

## 問題の意味と、解くための表現を分ける

低速運転時の上限は、二値変数が指定された値のときに条件を課す **Indicator制約** で表せます。この線形のIndicator制約を{class}`PySCIPOpt Adapter <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>`からSCIPに直接渡すことも、生産量の既知の上限を使って通常の不等式に変換し、同じSCIPで試すこともできます。どちらを選んでも、生産条件は一度記述すれば済みます。[問題を変換して解法へ渡す](./book/transform_and_solve.md)では、この不等式を導き、別の変換を選ぶときに確認すべき保証を説明します。

## 共通の表現の上に、解析と変換の技術を蓄積する

前処理や変換の技術を共通の表現に対して実装すれば、モデラーや解法ごとに同じ処理を作り直さずに活用できます。新しい解法も、問題の受け取り方と結果の返し方を接続することで、蓄積された共通処理を利用できます。このため、OMMXではデータの交換と、それに対する共通処理の両方を整備します。

## 拡張方針：宣言と共通処理の範囲を広げる

今後は、特殊制約で保持している構造を起点に、割当費用や巡回路の長さといった評価の構造、複数の評価軸を組み合わせる目的の構造へも、宣言できる範囲を広げる方針です。例えば費用・遅延・欠品数を個別の目的として保持し、要求に応じて重み付き和や辞書式の優先順位で具体化し、結果を元の各目的で評価することを目指します。顧客が指定した優先条件は、その具体化方針が満たすべき要求です。現在の{class}`~ommx.NamedFunction`による追加指標の評価と、こうした目的構造の保持・具体化は区別してください。

この方向では、自然言語の意図を利用者・AI・モデラーが確認し、確定した高位の宣言から数式へ展開する処理を共通化していきます。JijModelingなどのモデラーとOMMXをつなぐことで、案件ごとに人やAIが展開方法を組み立て、正しさを確かめる負担を減らすことを目指します。

今後は、問題の性質を調べる解析と、問題を書き換える変換を分け、解析結果の再利用や変換の選択・反復を共通化していく方針です。利用者が必要な保証を選び、処理系が各変換の保証を組み合わせて元の問題での結果解釈につなげることで、組合せの正しさを人が逐一追う負担も減らすことを目指します。解析・変換・保証を共通化する考え方は{ref}`変換章の末尾 <shared-analysis-and-transformations>`で説明します。

## 目的に合わせて読み始める

この導入ではOMMXを採用する理由を説明しました。Bookの残りの3章は、それぞれの章から直接読み始められます。

| 判断したいこと | Bookの章 |
|---|---|
| このモデルをソルバーに直接渡せるか。定式化を変えるなら、どの性質を保つ必要があるか | [問題を変換して解法へ渡す](./book/transform_and_solve.md) |
| 候補の有無やソルバーの報告から何が言えるか。結果を元の要求やデータへどう戻すか | [候補を評価して結果を判断する](./book/evaluate_and_interpret.md) |
| 別の人が結果を調べたり、試行を再実行したりするには、何を残す必要があるか | [問題・結果・試行を保存して共有する](./book/record_and_share.md) |

実行環境を用意するには、[SDKと必要なAdapterのインストール](./getting_started/install.md)から始めます。題材を通して試したい場合は、[巡回セールスマン問題の経路をサンプリングして描画する例](./examples/tsp_sampling_with_openjij_adapter)があります。

個々の操作や対応範囲は{doc}`Python API Reference <api/index>`から調べられます。3.0.0 stable releaseまでは、API Referenceは`main`を説明します。[インストール手順](./getting_started/install.md)には、最新の公開prereleaseと`main`からの導入方法を併記しています。例えば、候補の値と制約の違反を調べる例は{class}`~ommx.Solution`、ファイルから候補を読む例は{func}`~ommx.qplib.load_solution`にあります。{doc}`Adapterの一覧 <api/adapters>`から、各ソルバーの入力条件と求解例に進めます。自分の解法を接続する場合は{class}`~ommx.adapter.SolverAdapter`または{class}`~ommx.adapter.SamplerAdapter`を参照してください。

探索の進捗や終了理由を調べる手順は、{class}`SCIP <ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer>`と{class}`HiGHS <ommx_highs_adapter.HighsDiagnosticsAnalyzer>`の診断情報のReferenceにあります。

保存したデータを扱うには、[手元のArtifactを調べる手順](./how_to/manage_local_registry.md)や[レジストリで共有する手順](./how_to/share_artifact_registry.md)に進みます。外部ツールを接続するには、[エージェントプラグインの導入](./how_to/agent_skills.md)や{mod}`OpenTelemetryコレクターへの送信設定 <ommx.tracing>`を参照してください。

## データの交換と共有を支える仕組み

問題や解に共通の意味を与えることで、ソフトウェア間だけでなく、担当者間でも同じ問題と結果を扱えます。OMMXは次の要素でその受け渡しを支えます。

| 要素 | 役割 |
|---|---|
| OMMX Message | 問題や解を表す、言語・OSに依存しないデータ形式 |
| OMMX SDK | 問題と結果の構築・評価・変換・読み書き |
| OMMX Adapter | OMMXと個々のソルバーの入力・出力の接続と、解いた問題に対する報告の解釈 |
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

book/transform_and_solve
book/evaluate_and_interpret
book/record_and_share
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

examples/tsp_sampling_with_openjij_adapter
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

how_to/agent_skills
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
