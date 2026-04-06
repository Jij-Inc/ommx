---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx
  language: python
  name: python3
---

# QPLIBインスタンスをダウンロードする

OMMXリポジトリでは、QPLIBの二次計画問題ベンチマークインスタンスをOMMX Artifact形式のデータとして提供しています。

```{note}
より詳細な説明：QPLIBインスタンスに対応するOMMX ArtifactはOMMXリポジトリのGitHub コンテナーレジストリ ([link](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fqplib))で管理されています。

QPLIBは二次計画問題のインスタンスライブラリです。QPLIBの詳細については [QPLIB website](http://qplib.zib.de/) を参照してください。

GitHub コンテナーレジストリについては[こちら](https://docs.github.com/ja/packages/working-with-a-github-packages-registry/working-with-the-container-registry)を参照してください。
```

これらのインスタンスはOMMX SDKで簡単にダウンロードでき、OMMX Adapterの入力としてそのまま利用できます。
例えば、QPLIB_3514インスタンス ([参照](http://qplib.zib.de/QPLIB_3514.html)) をPySCIPOptで解くには、以下の2ステップで実行できます：

1. OMMX Python SDKの`dataset.qplib`関数で、3514インスタンスをダウンロードする。
2. ダウンロードしたインスタンスを、OMMX PySCIPOpt Adapterを介してPySCIPOptで解く。

具体的なPythonコードは以下の通りです：

```{code-cell} ipython3
# OMMX Python SDK
from ommx import dataset
# OMMX PySCIPOpt Adapter
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

# ステップ1: QPLIBの3514インスタンスをダウンロードする
instance = dataset.qplib("3514")

# ステップ2: OMMX PySCIPOpt Adapterを介してPySCIPOptで解く
solution = OMMXPySCIPOptAdapter.solve(instance)
```

この機能により、同一のQPLIBインスタンスを使用した二次計画問題ソルバーのベンチマーク測定を効率よく実行できます。

+++

## 補足：インスタンスに付随するアノテーション

ダウンロードしたインスタンスには各種アノテーションが含まれており、`annotations` プロパティを使って全てのアノテーションにアクセスできます：

```{code-cell} ipython3
import pandas as pd
# アノテーションを pandas を使って表形式で表示する
pd.DataFrame.from_dict(instance.annotations, orient="index", columns=["Value"]).sort_index()
```

インスタンスには、データセット共通のアノテーションとデータセット固有のアノテーションの2種類があります。

データセット共通のアノテーションには以下の7つがあり、それぞれに専用のプロパティが用意されています：

| アノテーション | プロパティ | 説明 |
|--------------|------------|------|
| `org.ommx.v1.instance.authors` | `authors` | そのインスタンスの作者 |
| `org.ommx.v1.instance.constraints` | `num_constraints` | そのインスタンスにある制約条件の数 |
| `org.ommx.v1.instance.created` | `created` | そのインスタンスがOMMX Artifact形式で保存された日時 |
| `org.ommx.v1.instance.dataset` | `dataset` | そのインスタンスが属するデータセット名 |
| `org.ommx.v1.instance.license` | `license` | そのデータセットのライセンス |
| `org.ommx.v1.instance.title` | `title` | そのインスタンスの名前 |
| `org.ommx.v1.instance.variables` | `num_variables` | そのインスタンスにある決定変数の総数 |

## QPLIBアノテーション

QPLIBインスタンスには、二次計画問題の数学的特性を記述する包括的なアノテーションが含まれています。これらのアノテーションは公式のQPLIB仕様に基づいており、`org.ommx.qplib.*` プレフィックスを持ちます。

利用可能なすべてのQPLIBアノテーションとその意味の詳細については、[公式QPLIBドキュメント](https://qplib.zib.de/doc.html)を参照してください。

例として、QPLIBインスタンスの問題の種類と目的関数の曲率を確認できます：

```{code-cell} ipython3
# QPLIB固有のアノテーション
print(f"問題の種類: {instance.annotations['org.ommx.qplib.probtype']}")
print(f"目的関数の種類: {instance.annotations['org.ommx.qplib.objtype']}")
print(f"目的関数の曲率: {instance.annotations['org.ommx.qplib.objcurvature']}")
print(f"変数の数: {instance.annotations['org.ommx.qplib.nvars']}")
print(f"制約の数: {instance.annotations['org.ommx.qplib.ncons']}")
```
