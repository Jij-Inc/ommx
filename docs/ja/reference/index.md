# Reference：APIとデータ交換の契約

正確な引数、適用条件、数式、エラー、変更されるデータを確認するための入口です。
概念の学習はUser Guide、最初の操作はTutorial、独立した作業手順はHow-toに分けています。

## Python SDKとAdapter

- [生成Python API Reference](../api/index)：Bookと同じ開発リビジョンのAPIです。
- [公開b7のSDK・Adapter実装](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/python)：
  公開`3.0.0b7`で実行する場合の版を固定した参照先です。
- {class}`FunctionのAPI Reference <ommx.Function>`：演算・表現・数値評価の契約。
- [Adapterの実装契約](../developer_guide/adapter.md)：exact input、encode/decode、評価対象。
- [Attachment Codecの公開b7 Protocol](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/python/ommx/ommx/experiment/attachments.py)：
  外部型をbytesへ変換し、同じMedia Typeで復元するための呼び出し契約です。

例えば`Instance.prepare()`の正確な変換順・条件・エラーは
{py:meth}`~ommx.Instance.prepare`、SOS1 Big-Mの数式と条件は
{py:meth}`~ommx.Instance.convert_sos1_to_constraints`で確認します。
公開例と開発APIが混在しないよう[互換性と実行環境](../getting_started/compatibility.md)も
確認してください。

## Rust SDK

[Rust SDK `3.0.0-beta.7`のAPI](https://docs.rs/ommx/3.0.0-beta.7/ommx/)を参照してください。
Python SDKとRust SDKの版は独立しています。Rust利用者はPython Tutorialを経由せず、
[連携のDeveloper Guide](../developer_guide/integration.md)から共通概念とRust APIへ進めます。
開発版の実装を対象にする場合は、依存するcommitと同じソースを参照します。

## 言語に依存しない形式

| 対象 | 公開b7に対応する仕様・定義 |
|---|---|
| OMMX Message | [Protocol Buffers定義](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/proto/ommx)。問題・候補・結果のwire schema |
| Functionのwire表現 | [v1 Function定義](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/proto/ommx/v1/function.proto)。v2の問題も利用する式の定義 |
| OMMX Artifact | [Artifact仕様](https://github.com/Jij-Inc/ommx/blob/python-3.0.0b7/rust/ommx/doc/artifact_design.md)。Manifest、Media Type、archiveとregistryの相互運用契約 |
| Experiment | [公開b7のExperiment実装・Config契約](https://github.com/Jij-Inc/ommx/tree/python-3.0.0b7/rust/ommx/src/experiment)。Artifact内の記録構造 |

wire schemaの`v1`や`v2`は、Python SDKのimport namespaceやパッケージ版ではありません。
またArtifactのMedia Typeにある版番号と、収録されたInstance/Solution payloadの版も別です。
別言語でファイルを扱う場合は、Pythonのチュートリアルコードを形式定義の代用にせず、
対象Media Typeと上記仕様に従ってください。
