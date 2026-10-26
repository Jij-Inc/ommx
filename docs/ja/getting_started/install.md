# Python SDKとAdapterのインストール

最初の3つのAdapter Tutorialでは、**OMMX Python SDK v3のプレビュー版**を使います。このページの手順で、SDKとAdapterのバージョンをそろえた環境を用意してください。

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

以降のインストールとサンプルコードの実行には、この仮想環境を使ってください。Notebookを使う場合も、この仮想環境のPythonを実行カーネルに指定します。

## 2. SDKとAdapterをインストールする

```sh
python -m pip install "ommx==3.0.0b7" "ommx-pyscipopt-adapter==3.0.0b7" "ommx-highs-adapter==3.0.0b7"
```

この環境で、次の順にTutorialを実行できます。

1. [PySCIPOpt Adapterで0-1ナップサック問題を解く](../tutorial/solve_with_ommx_adapter)：変換せずに求解し、結果を読む。
2. [PySCIPOpt Adapterで特殊制約をそのまま解く](../tutorial/solve_special_constraints_with_pyscipopt_adapter.md)：IndicatorとSOS1を使う。
3. [Adapter向けにInstanceを準備する](../tutorial/prepare_instance_for_adapter.md)：HiGHS向けのPreparationを使う。

その先の[Preparationで変換する制約族を選ぶ](../how_to/customize_preparation.md)も同じ環境で実行できます。[複数のAdapterで結果を比較する](../how_to/compare_adapters.md)では、グラフの描画用にmatplotlibも追加してください。

```sh
python -m pip install matplotlib
```

```{note}
このBookはv3の開発版に対応しており、API Referenceなどには未公開の機能が含まれる場合があります。上記のバージョンで実行を確認した範囲は、このページから案内する3つのTutorialと2つのHow-toです。ほかの機能や外部ツールの導入条件は、それぞれのページを確認してください。
```
