---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: .venv
  language: python
  name: python3
---

# OMMX Python SDK 1.6.0

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_1.6.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-1.6.0)

- 限定的なQUBOサポートが追加されました。
  - [OpenJij](https://github.com/OpenJij/OpenJij) を使った新しいアダプターパッケージ [ommx-openjij-adapter](https://pypi.org/project/ommx-openjij-adapter/) が追加されました。
  - 使い方は新しい [チュートリアルページ](https://jij-inc.github.io/ommx/ja/tutorial/tsp_sampling_with_openjij_adapter.html) をご覧ください。
  - `ommx.v1.Instance` をQUBO形式に変換するためのいくつかのAPIが追加されました。上記のチュートリアルをご覧ください。
- Python 3.8のサポートはEOLのため終了しました。
