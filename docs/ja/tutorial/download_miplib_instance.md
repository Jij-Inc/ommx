---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# MIPLIBインスタンスをダウンロードする

OMMXリポジトリでは、MIPLIB 2017の混合整数計画問題ベンチマークインスタンスをOMMX Artifact形式のデータとして提供しています。

```{note}
`dataset.miplib2017` は、公開済みの
`ghcr.io/jij-inc/ommx/v2.7/miplib2017:{instance-name}` 配布を使用します
（[パッケージ](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fv2.7%2Fmiplib2017)）。
この Artifact は MPS の整数変数の境界を修正した OMMX 2.7.0 で生成されており、
v3 SDK でも読み込めます。配布バージョンは採用したデータセットを表し、
インストール済み SDK のバージョンとは独立しています。

配布フォーマットや数学的モデルを変更する場合は、SDK の minor または major
リリースと新しい `/v{major}.{minor}/` 名前空間が必要です。patch リリースでは
採用済みの配布を維持し、公開済みのパス・タグは上書きしません。
再現性のため旧バージョンなしリポジトリも維持しますが、ローダーはそこへ
フォールバックしません。旧 Artifact がキャッシュされている場合も同様です。

[v2.7 の配布記録](https://github.com/Jij-Inc/ommx/blob/1ec99bbe3c36696e5fce3cac2cd87616e457af49/rust/dataset/distributions/v2.7/README.md)
に、元アーカイブ、公開済みインスタンス、非対応の入力、モデルのダイジェストを記載しています。

GitHub コンテナーレジストリについては[こちら](https://docs.github.com/ja/packages/working-with-a-github-packages-registry/working-with-the-container-registry)を参照してください。
```

これらのインスタンスはOMMX SDKで簡単にダウンロードでき、OMMX Adapterの入力としてそのまま利用できます。
例えば、MIPLIB 2017のneos-1122047インスタンス ([参照](https://miplib.zib.de/instance_details_neos-1122047.html)) をPySCIPOptで解くには、以下の2ステップで実行できます：

1. OMMX Python SDKの`dataset`サブモジュールにある`miplib2017`関数で、neos-1122047インスタンスをダウンロードする。
2. ダウンロードしたインスタンスを、OMMX PySCIPOpt Adapterを介してPySCIPOptで解く。

具体的なPythonコードは以下の通りです：

```{code-cell} ipython3
# OMMX Python SDK
from ommx import dataset
# OMMX PySCIPOpt Adapter
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

# ステップ1: MIPLIB 2017のneos-1122047インスタンスをダウンロードする
instance = dataset.miplib2017("neos-1122047")

# ステップ2: OMMX PySCIPOpt Adapterを介してPySCIPOptで解く
solution = OMMXPySCIPOptAdapter.solve(instance)
```

この機能により、複数のOMMX対応ソルバーで同一のMIPLIBインスタンスを使用したベンチマーク測定を効率よく実行できます。

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

MIPLIBに特有のアノテーションは `org.ommx.miplib.*` というプレフィックスを持ちます。

例として、neos-1122047インスタンスの最適値を確認してみましょう。そのインスタンスの詳細ページ ([link](https://miplib.zib.de/instance_details_neos-1122047.html)) によれば、最適値は `161` であり、この値はキー `org.ommx.miplib.objective` で取得できます：

```{code-cell} ipython3
# アノテーションの値はすべて文字列 (str) であることに注意する！
instance.annotations["org.ommx.miplib.objective"]
```

これにより、先ほどのOMMX PySCIPOpt Adapterで得られた計算結果が、期待される最適値と一致することを検証できます：

```{code-cell} ipython3
import numpy as np

best = float(instance.annotations["org.ommx.miplib.objective"])
assert np.isclose(solution.objective, best)
```
