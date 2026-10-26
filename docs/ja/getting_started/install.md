# Python SDKとAdapterのインストール

ここから案内する例では、**OMMX Python SDK v3のプレビュー版**を使います。まずSDKを導入し、ソルバーを使う場合だけ必要なAdapterを追加してください。

バージョンを指定せずにインストールすると、安定版のv2が選ばれるため、Tutorialのv3 APIを使えません。ここでは、公開済みの`3.0.0b7`で実行を確認した手順を示します。

## 1. 仮想環境を作る

Python 3.12を用意し、作業用のディレクトリで次を実行します。動作確認環境はmacOS（Apple Silicon）・Python 3.12.12です。

```{note}
`3.0.0b7`にはIntel Mac向けのwheelがありません。配布対象は[PyPIのファイル一覧](https://pypi.org/project/ommx/3.0.0b7/#files)で確認できます。
```

```sh
python -m venv .venv
```

macOS・Linuxでは、次のコマンドで仮想環境を有効にします。

```sh
source .venv/bin/activate
```

WindowsのPowerShellでは、代わりに次を実行します。

```powershell
.\.venv\Scripts\Activate.ps1
```

以降のインストールとサンプルコードの実行には、この仮想環境を使ってください。

Notebookを使う場合は、この仮想環境に実行カーネルも追加します。

```sh
python -m pip install ipykernel
```

お使いのJupyter対応エディターで、この仮想環境のPythonを実行カーネルに指定してください。既存のJupyter環境へカーネルを登録する場合は、有効化した仮想環境で次を実行し、Notebook側で「OMMX Book」を選びます。

```sh
python -m ipykernel install --user --name ommx-book --display-name "OMMX Book"
```

(install-python-sdk)=
## 2. Python SDKをインストールする

```sh
python -m pip install "ommx==3.0.0b7"
```

[Instanceを変換して結果を確認する](../how_to/transform_instance_without_solver.md)場合は、ここまでで準備が完了です。[InstanceClassの説明](../user_guide/instance_class.md)のmembership例もSDKだけで実行できます。

(install-adapters)=
## 3. ソルバーを使う場合だけAdapterを追加する

### PySCIPOpt Adapter

```sh
python -m pip install "ommx-pyscipopt-adapter==3.0.0b7"
```

このAdapterで、最初の二つのTutorialと[独自Policyの操作ガイド](../how_to/customize_preparation.md)を実行できます。

1. [PySCIPOpt Adapterで0-1ナップサック問題を解く](../tutorial/solve_with_ommx_adapter)：変換せずに求解し、結果を読む。
2. [PySCIPOpt Adapterで特殊制約をそのまま解く](../tutorial/solve_special_constraints_with_pyscipopt_adapter.md)：IndicatorとSOS1を使う。

### HiGHS Adapter

```sh
python -m pip install "ommx-highs-adapter==3.0.0b7"
```

このAdapterで、3つ目のTutorial[Adapter向けにInstanceを準備する](../tutorial/prepare_instance_for_adapter.md)を実行できます。ページ内でモデルを作り直すため、前のTutorialの実行状態やPySCIPOpt Adapterは必要ありません。

[複数のAdapterで結果を比較する](../how_to/compare_adapters.md)では、両方のAdapterと、グラフ描画用のmatplotlibを用意してください。

```sh
python -m pip install matplotlib
```

(install-openjij-adapter)=
### OpenJij Adapter

手順1・2で用意したSDKの`3.0.0b7`の環境に、同じバージョンのAdapterを追加します。

```sh
python -m pip install "ommx-openjij-adapter==3.0.0b7"
```

[OpenJij Adapterで小さな問題をサンプリングする](../how_to/sample_with_openjij_adapter)ための環境です。OpenJij本体も依存関係として導入されるため、別途の導入やソルバーのライセンス設定は必要ありません。PySCIPOptやHiGHSなど、ほかのAdapterは使いません。

任意の応用例である[TSPのNotebook](../tutorial/tsp_sampling_with_openjij_adapter)では、図を描くためにmatplotlibも追加します。

```sh
python -m pip install matplotlib
```

```{note}
このBookはv3の開発版に対応しており、API Referenceなどには未公開の機能が含まれる場合があります。上記のバージョンで実行を確認した範囲は、ここから案内するInstance変換How-toとmembership例、3つのAdapter Tutorialと2つのAdapter How-to、およびOpenJijの小さなサンプリングHow-toとTSPのNotebookです。ほかの機能や外部ツールの導入条件は、それぞれのページを確認してください。
```
