# OMMX Python SDK 2.6.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.6.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.6.0)

詳細な変更点は上のGitHub Releaseをご覧ください。以下に主な変更点をまとめます。

## 新機能

### `Instance.substitute` (2.6.0, [#892](https://github.com/Jij-Inc/ommx/pull/892))

Python SDK で `Instance.substitute` を公開しました。

このメソッドは、決定変数を式で置換して `Instance` を書き換えます。置換された変数は従属変数として記録されるため、解を評価するときに値を復元できます。
