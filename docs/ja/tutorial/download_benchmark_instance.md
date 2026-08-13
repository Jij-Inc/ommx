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

# ベンチマークInstanceをダウンロードする

OMMX Python SDKの`dataset`サブモジュールから、[MIPLIB 2017](https://miplib.zib.de/)と[QPLIB](https://qplib.zib.de/)のベンチマーク問題を{class}`~ommx.Instance`として取得できます。このチュートリアルでは、各データセットから1問ずつ取得し、PySCIPOpt Adapterで解きます。

## 必要なライブラリのインストール

```
pip install ommx-pyscipopt-adapter
```

## MIPLIB 2017

MIPLIB 2017は混合整数線形計画問題のベンチマークです。OMMX形式のArtifactは[GitHub Container Registry](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fmiplib2017)で公開されています。

`dataset.miplib2017()`に問題名を渡すとダウンロードできます。ここで使う[`neos-1122047`](https://miplib.zib.de/instance_details_neos-1122047.html)はPySCIPOpt Adapterがそのまま受け取れるため、Preparationなしで解けます。

```{code-cell} ipython3
from ommx import dataset
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

miplib_instance = dataset.miplib2017("neos-1122047")
miplib_solution = OMMXPySCIPOptAdapter.solve(miplib_instance)
```

MIPLIB固有のアノテーション`org.ommx.miplib.objective`には既知の最適値が文字列で格納されています。Adapterが返した目的値と比較できます。

```{code-cell} ipython3
import math

miplib_best = float(miplib_instance.annotations["org.ommx.miplib.objective"])
assert math.isclose(miplib_solution.objective, miplib_best)
miplib_solution.objective
```

## QPLIB

QPLIBは二次計画問題のベンチマークです。OMMX形式のArtifactは[GitHub Container Registry](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fqplib)で公開されています。

`dataset.qplib()`にQPLIBの番号を渡します。[`QPLIB_3514`](https://qplib.zib.de/QPLIB_3514.html)もPySCIPOpt Adapterに直接渡せます。

```{code-cell} ipython3
qplib_instance = dataset.qplib("3514")
qplib_solution = OMMXPySCIPOptAdapter.solve(qplib_instance)
qplib_solution.objective
```

## アノテーションを確認する

ダウンロードしたInstanceの`annotations`には、タイトル、作者、ライセンス、データセット名などの共通メタデータと、データセット固有のメタデータが入っています。

```{code-cell} ipython3
import pandas as pd

pd.DataFrame.from_dict(
    miplib_instance.annotations,
    orient="index",
    columns=["value"],
).sort_index()
```

QPLIB固有のアノテーションは`org.ommx.qplib.*`というprefixを持ちます。例えば、元のQPLIBデータに記録された問題・目的関数の種類と曲率を取得できます。

```{code-cell} ipython3
qplib_annotations = {
    "problem_type": qplib_instance.annotations["org.ommx.qplib.probtype"],
    "objective_type": qplib_instance.annotations["org.ommx.qplib.objtype"],
    "objective_curvature": qplib_instance.annotations["org.ommx.qplib.objcurvature"],
    "source_variables": qplib_instance.annotations["org.ommx.qplib.nvars"],
    "source_constraints": qplib_instance.annotations["org.ommx.qplib.ncons"],
}
pd.Series(qplib_annotations, name="value")
```

`org.ommx.qplib.ncons`は元のQPLIBに記録された制約数です。
