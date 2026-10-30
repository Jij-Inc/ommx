# Python SDKとAdapterのインストール

**3.0.0 stable releaseまでは、BookとAPI Referenceはともに`main`の内容を説明します。** 導入方法は、公開済みの最新prereleaseと、`main`のソースから選べます。

| 導入方法 | 選ぶ場面 |
|---|---|
| 最新prerelease | 公開パッケージですぐに使い始めたい場合。対応するwheelがあれば、SDKを手元でビルドする必要はありません。 |
| `main` | 未リリースの変更が必要な場合や、現在のドキュメントと同じソースを使いたい場合。 |

prereleaseには、ドキュメントに記載された変更がまだ含まれていない場合があります。必要なAPIが導入した版にない場合は、`main`を使ってください。まずSDKを用意し、ソルバーを使う場合だけ必要なAdapterを追加します。

[uv](https://docs.astral.sh/uv/getting-started/installation/)を用意してください。以下ではPython 3.12を使い、uvが必要に応じて用意します。

(install-python-sdk)=
## 1. Python SDKをインストールする

### 最新prerelease

OMMXリポジトリの外に作業用プロジェクトを作ります。

```sh
uv init --bare --python 3.12 ommx-work
cd ommx-work
uv add --python 3.12 --prerelease=allow ommx
```

prereleaseを含む、互換性のある最新の公開版が選ばれ、依存関係は`pyproject.toml`と`uv.lock`に記録されます。こちらを選んだ場合、以降のコマンドは`ommx-work`で実行してください。

### main

[Git](https://git-scm.com/)、[Rustのstable toolchain](https://www.rust-lang.org/tools/install)と、利用OSのC/C++ビルドツールも用意してください。こちらはSDKのRust拡張をソースからビルドします。

```sh
git clone --branch main https://github.com/Jij-Inc/ommx.git
cd ommx
uv sync --locked --package ommx --no-default-groups --python 3.12
```

こちらを選んだ場合、以降のコマンドはcheckoutのルートで実行してください。uvはcheckoutのSDKと、リポジトリの`uv.lock`に記録された依存関係を導入します。Adapterも同じcheckoutから追加します。

### 環境を確認する

どちらの方法でも`.venv`が作られます。選んだ方法の作業ディレクトリで、次を実行してください。

```sh
uv run --no-sync python -c "from importlib.metadata import version; from ommx import Instance; print(version('ommx')); print(Instance.minimize())"
```

サンプルをスクリプトとして保存したら、`uv run --no-sync python example.py`で実行できます。{meth}`~ommx.Instance.prepare`による問題の変換、{class}`~ommx.InstanceClass`への所属判定、[既存の問題と候補解の評価](../tutorial/evaluate_existing_candidate)にはSDKだけを使います。[既存問題の入手](../how_to/load_existing_instance.md)で公開データを初めて取得するときは、ネットワーク接続が必要です。

(install-adapters)=
## 2. ソルバーを使う場合だけAdapterを追加する

SDKと同じ導入方法のコマンドを使ってください。prereleaseでは、`uv add`が作業用プロジェクト内でSDKとAdapterの依存関係をまとめて解決します。`main`では、`--inexact`がすでに追加した別のAdapterやNotebook用パッケージを残します。

### PySCIPOpt Adapter

最新prerelease：

```sh
uv add --prerelease=allow ommx-pyscipopt-adapter
```

main：

```sh
uv sync --locked --package ommx-pyscipopt-adapter --no-default-groups --inexact
```

[0-1ナップサック問題を解く例](../tutorial/solve_with_ommx_adapter)と[特殊制約をそのまま解く例](../tutorial/solve_special_constraints_with_pyscipopt_adapter.md)を参照してください。独自の変換方針を使う手順は、Referenceの{class}`明示的なPreparationと求解の例 <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>`にあります。

(install-highs-adapter)=
### HiGHS Adapter

最新prerelease：

```sh
uv add --prerelease=allow ommx-highs-adapter
```

main：

```sh
uv sync --locked --package ommx-highs-adapter --no-default-groups --inexact
```

[Adapter向けにInstanceを準備する例](../tutorial/prepare_instance_for_adapter.md)を参照してください。この例はモデルをその場で作るため、前のTutorialの実行状態やPySCIPOpt Adapterは不要です。[複数のAdapterで結果を比較する](../how_to/compare_adapters.md)場合は、両方のAdapterと、後述の手順でmatplotlibを用意します。

(install-openjij-adapter)=
### OpenJij Adapter

最新prerelease：

```sh
uv add --prerelease=allow ommx-openjij-adapter
```

main：

```sh
uv sync --locked --package ommx-openjij-adapter --no-default-groups --inexact
```

OpenJij本体も依存関係として導入されます。ほかのAdapterやソルバーのライセンス設定は不要です。[小さな問題のサンプリング](../how_to/sample_with_openjij_adapter)を参照してください。任意の応用例である[TSPのNotebook](../tutorial/tsp_sampling_with_openjij_adapter)では、経路の描画にmatplotlibを使います。

## 3. 必要に応じてNotebook・描画用ツールを追加する

prereleaseのプロジェクトでは、追加するツールも依存関係として記録します。

```sh
uv add ipykernel
```

`main`のcheckoutでは、その環境に追加します。

```sh
uv pip install ipykernel
```

Jupyter対応エディターで、選んだ作業ディレクトリの`.venv`をカーネルに指定してください。既存のJupyter環境にカーネルを登録する場合は、次を実行し、Notebook側で「OMMX Book」を選びます。

```sh
uv run --no-sync python -m ipykernel install --user --name ommx-book --display-name "OMMX Book"
```

グラフ描画には、prereleaseのプロジェクトでは`uv add matplotlib`、`main`のcheckoutでは`uv pip install matplotlib`を使います。

## 環境を更新する

最新prereleaseでは、プロジェクトに記録したSDKとすべてのAdapterをまとめて更新します。

```sh
uv sync --upgrade --prerelease=allow
```

`main`では、次を実行します。

```sh
git pull --ff-only
uv sync --locked --package ommx --no-default-groups --inexact
```

利用するAdapterの`main`向け追加コマンドも再実行し、SDKとAdapterを同じcheckoutに揃えます。どちらの環境でも、更新後は使っているNotebookカーネルを再起動してください。

(install-cli)=
## 任意：Artifactを管理するCLI

[保存したArtifactの調査・整理](../how_to/manage_local_registry.md)には、SDKに同梱されるCLIを使えます。どちらの環境でも、次を実行できます。

```sh
uv run --no-sync ommx --version
uv run --no-sync ommx --help
```

Python SDKを使わずCLIだけを導入する場合は、上記の手順で`main`のソースを取得し、checkoutのルートでRustからビルドします。

```sh
cargo install --locked --path rust/ommx --features cli
ommx --version
ommx --help
```

CLIはRust SDKのバージョンを表示します。Rust SDKとPython SDKのバージョンは独立して管理されています。
