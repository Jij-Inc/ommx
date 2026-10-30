# Python SDKとAdapterのインストール

**3.0.0 stable releaseまでは、BookとAPI Referenceはともに`main`の内容を説明します。** SDKとAdapterも同じcheckoutから導入します。まずSDKを用意し、ソルバーを使う場合だけ必要なAdapterを追加してください。

## 1. mainのソースを取得する

[Git](https://git-scm.com/)、[uv](https://docs.astral.sh/uv/getting-started/installation/)、[Rustのstable toolchain](https://www.rust-lang.org/tools/install)と、利用OSのC/C++ビルドツールを用意してください。SDKのRust拡張をソースからビルドします。以下ではPython 3.12を使い、uvが必要に応じて用意します。

```sh
git clone --branch main https://github.com/Jij-Inc/ommx.git
cd ommx
```

以降のコマンドは、このリポジトリのルートで実行します。

(install-python-sdk)=
## 2. Python SDKをインストールする

```sh
uv sync --locked --package ommx --no-default-groups --python 3.12
uv run --no-sync python -c "from ommx import Instance; print(Instance.minimize())"
```

uvは`.venv`を作り、checkoutのSDKと`uv.lock`に記録された依存関係を導入します。`--package ommx`でSDKだけを選び、Adapterや開発用ツールは必要になった時点で追加します。サンプルをスクリプトとして保存したら、`uv run --no-sync python example.py`でこの環境から実行できます。

{meth}`~ommx.Instance.prepare`による問題の変換、{class}`~ommx.InstanceClass`への所属判定、[既存の問題と候補解の評価](../tutorial/evaluate_existing_candidate)にはSDKだけを使います。[既存問題の入手](../how_to/load_existing_instance.md)で公開データを初めて取得するときは、ネットワーク接続が必要です。

### Notebookを使う場合

同じ環境に実行カーネルを追加します。

```sh
uv pip install ipykernel
```

Jupyter対応エディターで、このcheckoutの`.venv`をカーネルに選んでください。既存のJupyter環境にカーネルを登録する場合は、次を実行し、Notebook側で「OMMX Book」を選びます。

```sh
uv run --no-sync python -m ipykernel install --user --name ommx-book --display-name "OMMX Book"
```

(install-adapters)=
## 3. ソルバーを使う場合だけAdapterを追加する

次のコマンドは同じcheckoutのAdapterを追加します。`--inexact`は、すでに追加した別のAdapterやNotebook用パッケージを残します。

### PySCIPOpt Adapter

```sh
uv sync --locked --package ommx-pyscipopt-adapter --no-default-groups --inexact
```

[0-1ナップサック問題を解く例](../tutorial/solve_with_ommx_adapter)と[特殊制約をそのまま解く例](../tutorial/solve_special_constraints_with_pyscipopt_adapter.md)を実行できます。独自の変換方針を使う手順は、Referenceの{class}`明示的なPreparationと求解の例 <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>`にあります。

(install-highs-adapter)=
### HiGHS Adapter

```sh
uv sync --locked --package ommx-highs-adapter --no-default-groups --inexact
```

[Adapter向けにInstanceを準備する例](../tutorial/prepare_instance_for_adapter.md)を実行できます。この例はモデルをその場で作るため、前のTutorialの実行状態やPySCIPOpt Adapterは不要です。

[複数のAdapterで結果を比較する](../how_to/compare_adapters.md)場合は、両方のAdapterと、グラフ描画用のmatplotlibを用意します。

```sh
uv pip install matplotlib
```

(install-openjij-adapter)=
### OpenJij Adapter

```sh
uv sync --locked --package ommx-openjij-adapter --no-default-groups --inexact
```

[小さな問題のサンプリング](../how_to/sample_with_openjij_adapter)を実行できます。OpenJij本体も依存関係として導入されます。ほかのAdapterやソルバーのライセンス設定は不要です。

任意の応用例である[TSPのNotebook](../tutorial/tsp_sampling_with_openjij_adapter)で経路を描画する場合は、matplotlibも追加します。

```sh
uv pip install matplotlib
```

## mainの更新を取り込む

```sh
git pull --ff-only
uv sync --locked --package ommx --no-default-groups --inexact
```

利用するAdapterの追加コマンドも再実行し、SDKとAdapterを同じcheckoutに揃えます。ソース更新後は、使っているNotebookカーネルを再起動してください。

(install-cli)=
## 任意：Artifactを管理するCLI

[保存したArtifactの調査・整理](../how_to/manage_local_registry.md)には、SDKに同梱されるCLIを使えます。

```sh
uv run --no-sync ommx --version
uv run --no-sync ommx --help
```

Python SDKを使わずCLIだけを導入する場合は、同じcheckoutからRustでビルドします。

```sh
cargo install --locked --path rust/ommx --features cli
ommx --version
ommx --help
```

CLIはRust SDKのバージョンを表示します。Rust SDKとPython SDKのバージョンは独立して管理されています。
