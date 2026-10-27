# 使うAdapterを選ぶ

AdapterはOMMXのInstanceと実行backendの入力・出力を接続します。
使いたいソルバーやサンプラーが決まっていれば、そのAdapterの手順へ直接進めます。
すべてのTutorialを順番に実行する必要はありません。

| やりたいこと | 最初の手順 | 戻ってくる結果 |
|---|---|---|
| 小さな問題を初めて解く | [PySCIPOptのTutorial](../tutorial/solve_with_ommx_adapter) | Solution |
| Indicator・SOS1を直接扱う | [PySCIPOptで特殊制約を解く](../tutorial/solve_special_constraints_with_pyscipopt_adapter.md) | Solution |
| 特殊制約を通常の制約へ変換してHiGHSで解く | [Adapter向けのPreparation](../tutorial/prepare_instance_for_adapter.md) | Solution |
| OpenJijで複数の候補を得る | [小さな問題をサンプリングする](../how_to/sample_with_openjij_adapter) | SampleSet |

[インストール手順](../getting_started/install.md)で、選んだAdapterだけ追加します。
`Solution`は一つの候補の評価結果、`SampleSet`は複数の候補と出現回数などを扱う結果です。
返り値の種類が同じでも、探索方法や最適性の保証は同じとは限りません。

## 選ぶときの四つの確認

1. **直接渡せる入力**：Adapterの`INPUT_CLASS`は、その実装が直接扱えるInstanceの条件です。
   単に「線形」「QUBO」という通称だけでなく、変数の種類・式の表現・制約族・senseを確認します。
2. **必要なPreparation**：直接受け取れない場合は、Adapterの推奨Policyを出発点にします。
   対応しない制約や必要な有限boundによっては、変換できない場合もあります。
3. **ユーザーが決める設定**：ペナルティの重み、許容誤差、探索回数・時間制限などです。
   推奨Policyを取得しただけで、問題ごとに適切な設定がすべて決まるわけではありません。
4. **結果から分かること**：報告目的値、実行可能性、候補の数に加え、backendが返す終了理由や
   bound・gap・最適性情報を確認します。利用可能なdiagnosticsもAdapterごとに異なります。

`INPUT_CLASS`への所属は表現条件の確認であり、変換前との数学的な同値性やbackendの
最適性保証を示すものではありません。これらは別々に判断します。
[InstanceClass](instance_class.md)、[PreparationPolicy](preparation_policy.md)、
[変換の保証](transformation_guarantees.md)で各判断の意味を説明します。

## ソルバー名と問題クラスは一対一ではない

例えばOpenJij SA Adapterの直接入力は、Binary変数の多項式最小化とactive制約なしという
条件で、二次式だけに限りません。「OpenJijだから最初にQUBOへ変換する」とは考えず、
実際の`INPUT_CLASS`と推奨Policyを使います。重みの明示が必要な例は
[OpenJij How-to](../how_to/sample_with_openjij_adapter)を参照してください。

逆に、同じInstanceを受け取れるAdapterでも、設定・探索方法・終了条件は異なります。
[複数のAdapterの比較](../how_to/compare_adapters.md)では同じ問題の報告目的値と実行可能性を
比較し、[diagnostics](../how_to/inspect_adapter_diagnostics.md)ではbackend側の観測を調べます。
初期候補を持っている場合は[初期候補のHow-to](../how_to/use_initial_state.md)、
処理のどこに時間がかかるかを見たい場合は[トレースのHow-to](../how_to/trace_execution)へ進みます。
いずれもExperimentの学習は前提ではありません。

## パッケージと契約の確認先

同一リポジトリで開発するAdapterは以下です。Bookの実行例の基準は公開`3.0.0b7`です。
リンク先の実装も同じタグを指します。正確な引数と対応範囲は各APIを確認してください。

| パッケージ | 対応backend | b7の実装 |
|---|---|---|
| `ommx-pyscipopt-adapter` | SCIP / PySCIPOpt | [実装](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/python/ommx-pyscipopt-adapter) |
| `ommx-highs-adapter` | HiGHS | [実装](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/python/ommx-highs-adapter) |
| `ommx-openjij-adapter` | OpenJij SA / SQA | [実装](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/python/ommx-openjij-adapter) |
| `ommx-python-mip-adapter` | Python-MIP | [実装](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/python/ommx-python-mip-adapter) |

このBookのAPI ReferenceはBookと同じ開発リビジョンから生成されます。
公開パッケージと差がある機能は、[Referenceのバージョン案内](../reference/index.md)に従って
公開タグの契約も確認してください。

別リポジトリで提供するAdapterには、[Gurobi](https://github.com/Jij-Inc/ommx-gurobipy-adapter)、
[D-Wave](https://github.com/Jij-Inc/ommx-dwave-adapter)、
[Fixstars Amplify](https://github.com/Jij-Inc/ommx-fixstars-amplify-adapter)、
[Fujitsu DA4](https://github.com/Jij-Inc/ommx-da4-adapter)、
[Kipu Iskay](https://github.com/Jij-Inc/ommx-kipu-iskay-adapter)、
[Q-CTRL QAOA](https://github.com/Jij-Inc/ommx-qctrl-qaoa-adapter)などがあります。
各プロジェクトのSDK互換性、実行環境、認証・ライセンス、結果の契約を個別に確認してください。
この一覧は、それらがBookのb7環境で実行確認済みであることを意味しません。

Adapter自体を作る場合は[Developer Guide](../developer_guide/adapter.md)を参照してください。
