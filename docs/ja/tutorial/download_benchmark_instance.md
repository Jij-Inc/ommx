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

MIPLIB 2017は混合整数線形計画問題のベンチマークです。`dataset.miplib2017()`は、整数変数の境界を修正した[公開済みのv2.7配布](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fv2.7%2Fmiplib2017)を取得します。配布バージョンはSDKのバージョンとは独立しており、v3 SDKでも読み込めます。

`dataset.miplib2017()`に問題名を渡すとダウンロードできます。ここで使う[`neos-1122047`](https://miplib.zib.de/instance_details_neos-1122047.html)はPySCIPOpt Adapterがそのまま受け取れるため、モデルを変換せずに解けます。

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

QPLIBは二次計画問題のベンチマークです。`dataset.qplib()`は、二次係数を修正して再生成した[公開済みのv2.8配布](https://github.com/Jij-Inc/ommx/pkgs/container/ommx%2Fv2.8%2Fqplib)を取得します。旧バージョンなしArtifactがキャッシュされていても、修正済みの配布を選択します。

`dataset.qplib()`にQPLIBの番号を渡します。[`QPLIB_3514`](https://qplib.zib.de/QPLIB_3514.html)もPySCIPOpt Adapterに直接渡せます。

```{code-cell} ipython3
qplib_instance = dataset.qplib("3514")
qplib_solution = OMMXPySCIPOptAdapter.solve(qplib_instance)
qplib_solution.objective
```

## 公開されているQPLIBの解を評価する

[QPLIBのWebサイト](https://qplib.zib.de/QPLIB_0018.html)から同じ問題の`.qplib`と`.sol`をダウンロードすると、ソルバーを実行せずに公開解を評価できます。

```python
from ommx import Instance, State

instance = Instance.load_qplib("QPLIB_0018.qplib")
state = State.load_qplib_solution(
    "QPLIB_0018.sol", num_variables=len(instance.decision_variables)
)
solution = instance.evaluate(state, atol=1e-8)
print(solution.objective, solution.feasible)
```

元の問題の変数数を渡すと、解ファイルで省略された変数の値を0で補います。`objvar`は決定変数として扱いません。ファイル形式とエラーの扱いは{meth}`~ommx.State.load_qplib_solution`を参照してください。

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

## 配布と再現性

ローダーが採用する配布は`ghcr.io/jij-inc/ommx/v2.7/miplib2017:{instance-name}`と`ghcr.io/jij-inc/ommx/v2.8/qplib:{numeric-tag}`です。配布フォーマットや数学的モデルの変更には、SDKのminorまたはmajorリリースと新しい`/v{major}.{minor}/`名前空間を使います。patchリリースでは採用済みの配布を維持し、公開済みのパス・タグは上書きしません。ローダーは旧バージョンなし配布へフォールバックしません。

元データ、修正内容、公開済みのダイジェストは[MIPLIB v2.7の配布記録](https://github.com/Jij-Inc/ommx/blob/1ec99bbe3c36696e5fce3cac2cd87616e457af49/rust/dataset/distributions/v2.7/README.md)と[QPLIB v2.8の配布記録](https://github.com/Jij-Inc/ommx/blob/b4cffe9f1ce5323c15de67eca55b05d9f87285a0/rust/dataset/distributions/v2.8/README.md)を参照してください。別途保存した旧Instanceへ修正を反映するには、修正済み配布から読み込み直してください。
