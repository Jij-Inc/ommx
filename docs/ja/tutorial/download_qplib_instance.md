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
`dataset.qplib` は、公開済みの
`ghcr.io/jij-inc/ommx/v2.8/qplib:{numeric-tag}` 配布を使用します
（[パッケージ](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fv2.8%2Fqplib)）。
453件の Artifact は二次係数を修正した OMMX 2.8.0 で再生成されており、
v3 SDK でも読み込めます。配布バージョンはインストール済み SDK の
バージョンとは独立しています。

配布フォーマットや数学的モデルを変更する場合は、SDK の minor または major
リリースと新しい `/v{major}.{minor}/` 名前空間が必要です。patch リリースでは
採用済みの配布を維持し、公開済みのパス・タグは上書きしません。
旧バージョンなし Artifact がキャッシュされていても、修正済みの配布を選択します。
別途保存したインスタンスへ係数の修正を反映するには、読み込み直してください。

[v2.8 の配布記録](https://github.com/Jij-Inc/ommx/blob/b4cffe9f1ce5323c15de67eca55b05d9f87285a0/rust/dataset/distributions/v2.8/README.md)
に、元アーカイブ、モデルの比較結果、公開済みのダイジェストを記載しています。

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

## 公開されている解を評価する

[QPLIBのWebサイト](https://qplib.zib.de/QPLIB_0018.html)から同じ問題の
`.qplib` と `.sol` をダウンロードすると、ソルバーを実行せずに公開解を評価できます。

```python
from ommx import Instance, State

instance = Instance.load_qplib("QPLIB_0018.qplib")
state = State.load_qplib_solution(
    "QPLIB_0018.sol", num_variables=len(instance.decision_variables)
)
solution = instance.evaluate(state, atol=1e-8)
print(solution.objective, solution.feasible)
```

元のQPLIB問題の変数数を渡すことで、ファイルで省略された変数の値を0で補います。
`State.load_qplib_solution` はQPLIB公式の `.sol` ファイルで使われる標準の変数名に
対応しています。ファイル中の `objvar` は決定変数ではありません。
`Instance.evaluate` が読み込んだStateから目的値と実行可能性を計算します。
ファイル形式とエラーの扱いは {meth}`~ommx.State.load_qplib_solution` を参照してください。

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
