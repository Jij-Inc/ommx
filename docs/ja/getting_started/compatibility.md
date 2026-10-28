# バージョンと実行環境

このBookのPython実行例は、公開済みの**`ommx==3.0.0b7`**を基準にしています。
開発版だけの機能は、対応する環境をすでに持つ読者向けの任意例として区別します。
まず[インストール手順](install.md)のSDKだけを用意し、目的に必要な追加パッケージを選びます。
公開b7例の確認環境はmacOS arm64・Python 3.12.12です。以下の表は目的別の依存を示し、
Book全ページを同じ環境で実行確認したことを示すものではありません。他のOSやアーキテクチャでの
wheelの有無は[公開ファイル一覧](https://pypi.org/project/ommx/3.0.0b7/#files)を確認してください。

## 目的別に追加するもの

| 目的 | SDKに加えるもの・入力 | 最初の成功に不要なもの |
|---|---|---|
| 問題・候補の構築と評価、Instanceの変換 | このBook内の小さなモデル | Adapter・ソルバー・Experiment |
| 既存候補の評価 | QPLIB_0018の問題・候補ファイル。初回取得はネットワークが必要 | モデルの自作・Adapter |
| MIPLIB/QPLIBの入手 | 公開レジストリへのネットワーク接続 | 求解backend |
| 最初の求解、特殊制約、初期候補、SCIP diagnostics | `ommx-pyscipopt-adapter==3.0.0b7` | ほかのAdapter |
| HiGHSでのPreparation・求解 | `ommx-highs-adapter==3.0.0b7` | PySCIPOptの導入や前のTutorialの実行状態 |
| Adapter比較 | 上の両Adapter、matplotlib | Experiment |
| 小さなOpenJijサンプリング | `ommx-openjij-adapter==3.0.0b7`。重みは例中で明示 | SCIP・HiGHS |
| 任意のTSP応用 | OpenJijの環境、matplotlib | ほかのTutorialの実行状態 |
| トレースの最初の例 | SDKとNotebook kernel。スクリプト用APIもある | Experiment・Adapter |
| Experimentの記録・保存・再読込 | SDK、例内で作る問題・候補、ファイル保存先 | Adapter・独自Codec・リモートレジストリ |
| Artifactの受け渡し | SDK、例内で作るデータ。受信は新しいプロセスと空Local Registry | ソルバー・送信側のRegistry・認証 |
| 保存済みArtifactのCLI調査・整理 | 単体Rust CLI `3.0.0-beta.7`、調査するArtifactのref | Python SDK・Adapter |

Notebookを実行するときは、この環境のPythonをkernelに指定してください。表や図が主題の
ページでは実行結果を`.ipynb`にも保存しています。表示済み出力を読むだけであれば、
Notebook環境を用意する必要はありません。

確認時の主な追加パッケージは、pandas 2.3.3、PyArrow 22.0.0、matplotlib 3.11.2、
OpenJij 0.12.0、PySCIPOpt 6.3.0、highspy 1.15.1です。これは動作確認時の記録であり、
Adapterの対応範囲をこれらのバージョンだけに限定する宣言ではありません。
セルの順序、外部データ、必要な設定は各ページで指定します。

## API Referenceと公開パッケージの差

このBookのAPI ReferenceはBookと同じ開発リビジョンから生成されるため、公開b7にない機能も
含みます。例えば名前付きStateの変換や`used_decision_variable_ids`は、公開b7の実行例とは
分けて扱います。公開APIの契約を実装に照らして確認する場合は、
[公開b7のソース](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7)を使ってください。
開発版の機能を使う場合は、SDK・Adapter・Referenceを同じリビジョンに揃えます。

## Python・Rust・wire形式は別のバージョン

| 対象 | このBookでの参照基準 |
|---|---|
| Python SDK・同一リポジトリのAdapter | 公開 `3.0.0b7`の実行例。生成APIはBookの開発リビジョン |
| Rust SDK・単体CLI | 公開 `3.0.0-beta.7`。Pythonの版番号とは独立 |
| Messageのwire schema | 対象データのMedia Typeに従う`ommx.v1`/`ommx.v2`。Python import名ではない |
| Artifact形式 | Manifest・Media Type・archiveの仕様。中のMessageの版とは別 |
| データセット配布 | MIPLIBの`v2.7`、QPLIBの`v2.8`。SDKの版ではない |

CLIを使う場合は{ref}`単体CLIの導入手順 <install-cli>`から始められます。
b7のPython環境をCLIのためだけに更新する必要はありません。認証付きレジストリ共有は
利用者自身のアカウント・権限を使う任意の操作で、このBookのローカル往復確認とは別です。
Rust/他言語の入口と版を固定した仕様は[Reference](../reference/index.md)にまとめています。
