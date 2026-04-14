# OMMX Python SDK 3.0.x

## 3.0.0 Alpha 1

```{note}
これはプレリリースバージョンです。APIは最終的な3.0.0リリースまでに変更される可能性があります。
```

このリリースのテーマは **Python/Protocol BuffersからRust/PyO3への完全な移行** です。2.0.0ではコア実装がRustで書き直されましたが、互換性のためにPythonラッパークラスが残されていました。3.0.0ではそれらのPythonラッパーを完全に削除し、`ommx.v1` の全型がRustからの直接再エクスポートとなり、`protobuf` Pythonランタイム依存も排除されます。

[Claude Code](https://www.anthropic.com/claude-code) などのAIアシスタントでの利用を想定したマイグレーションガイドを [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md) にまとめてあります。

### `ommx.v1` 型の完全なRust再エクスポート ([#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771), [#774](https://github.com/Jij-Inc/ommx/pull/774), [#775](https://github.com/Jij-Inc/ommx/pull/775))

`ommx.v1` の全クラス — `Linear`, `Quadratic`, `Polynomial`, `Function`, `DecisionVariable`, `Parameter`, `Constraint`, `RemovedConstraint`, `NamedFunction`, `Instance`, `ParametricInstance`, `Solution`, `SampleSet` — が完全にネイティブ実装になりました。Pythonのラッパー層がなくなり、4,500行以上のPythonコードが削除されています。基礎となるprotobufオブジェクトへのアクセスを提供していた `.raw` 属性も廃止されました。

演算子、制約ID管理、DataFrameプロパティ、QUBO/HUBOパイプライン、緩和、サンプリングヘルパーはすべてネイティブ実装の一部です。`State` 型はPythonから `dict`、`Mapping`、`Iterable` を受け付けます。

### Protocol Buffers Python依存の削除 ([#776](https://github.com/Jij-Inc/ommx/pull/776))

28個の `*_pb2.py` / `*_pb2.pyi` ファイルをすべて削除し、`protobuf` ランタイム依存を除去しました。すべてのPython型はネイティブバインディングのみで提供され、protobufワイヤ形式へのシリアライズ/デシリアライズは内部で処理されます。

### アノテーションシステムの再実装 ([#772](https://github.com/Jij-Inc/ommx/pull/772))

`Instance`、`ParametricInstance`、`Solution`、`SampleSet` の `annotations` フィールドがネイティブ実装になりました。Python側の `UserAnnotationBase` クラスと `annotation.py` のディスクリプタは削除されました。

### Artifact/ArtifactBuilder の再実装 ([#782](https://github.com/Jij-Inc/ommx/pull/782))

`Artifact` と `ArtifactBuilder`（Archive/Dir/Builder を含む）がネイティブ実装に移行し、`ommx.artifact` モジュールが自動生成されます。6つのPython専用サブクラスは削除・統合されました。

### `ommx/v1/__init__.py` の自動生成 ([#779](https://github.com/Jij-Inc/ommx/pull/779))

手書きの361行の `v1/__init__.py` が自動生成に置き換えられました。ファイルは `pyo3-stub-gen` の `generate-init-py` 機能により `task python:stubgen` で生成されます。

### pandas DataFrame ロジックの統合 ([#778](https://github.com/Jij-Inc/ommx/pull/778))

内部のDataFrame変換ロジックが統合され、null値の処理が `pandas.NA` に統一されました。

### API リファレンスを pyo3-stub-gen docgen に切り替え ([#780](https://github.com/Jij-Inc/ommx/pull/780))

`ommx.v1` 型のAPIドキュメントが `sphinx-autoapi` から `pyo3-stub-gen` の `doc-gen` 機能による生成に切り替えられました。純Python製のアダプターパッケージは引き続き `sphinx-autoapi` を使用します。

### ドキュメントをSphinxに移行 ([#785](https://github.com/Jij-Inc/ommx/pull/785))

ドキュメントをJupyter Book（GitHub Pages）からSphinx + `myst-nb`（ReadTheDocs）に移行しました。テーマは `sphinx-book-theme` に切り替え、数式レンダリングにKaTeXを使用します。英語・日本語のドキュメントは個別のRTDプロジェクトとして、共有のAPI Referenceとともにホストされます。
