# OMMX Python SDK 2.6.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.6.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.6.0)
[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.6.1-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.6.1)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。

## 新機能

### `Instance.substitute` (2.6.0, [#892](https://github.com/Jij-Inc/ommx/pull/892))

Python SDK で `Instance.substitute` を公開しました。

このメソッドは、決定変数を式で置換して `Instance` を書き換えます。置換された変数は従属変数として記録されるため、解を評価するときに値を復元できます。

詳細とモデリング上の注意点は [`Instance` のユーザーガイド](../user_guide/instance.ipynb) を参照してください。

### `ParametricInstance.substitute` (2.6.1, [#898](https://github.com/Jij-Inc/ommx/pull/898))

Python SDK で `ParametricInstance.substitute` を公開しました。

このメソッドは、パラメータ参照を記号的に残したまま決定変数を置換します。置換対象は決定変数である必要があり、パラメータ ID を置換しようとするとエラーになります。

パラメータ固有の挙動は [`ParametricInstance` のユーザーガイド](../user_guide/parametric_instance.ipynb) を参照してください。

## バグ修正

### MPS 読み込み時に省略された二値変数の境界を保持 (2.6.4, [#1203](https://github.com/Jij-Inc/ommx/pull/1203))

`Instance.load_mps()` と `ommx.mps.load_file()` が、`INTORG`/`INTEND` 内にあり
`BOUNDS` の指定がない変数を、Gurobi/HiGHS の MPS 解釈に従って `[0, 1]` の
二値変数として読み込むようになりました。従来は上限のない一般整数変数として
読み込んでいました。`neos-2626858-aoos` では17変数が影響を受け、二値変数の数が
209から192に変わっていました。

明示された境界指定は引き続き優先され、`LO`/`LI 0` で指定した非負の整数変数は
上限なしのままです。`ommx.dataset` 経由で読み込むものを含め、生成済みの Artifact は
元の MPS から別途再生成する必要があります。SDK の更新だけでは保存済みの問題は修復されません。

### 置換の検証とパラメータ具体化 (2.6.1, [#898](https://github.com/Jij-Inc/ommx/pull/898))

`Instance.substitute` が、右辺で未定義の決定変数 ID を参照する置換式を拒否するようになりました。`ParametricInstance.substitute` では、右辺の参照が登録済みの決定変数またはパラメータであることを検証します。

`ParametricInstance.with_parameters` は `decision_variable_dependency` と `removed_constraints` 内のパラメータ参照も評価するようになり、生成される `Instance` のこれらのフィールドにパラメータ ID が残らないようになりました。
