---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

# 公開ベンチマークを解いて結果を比較する

{mod}`~ommx.dataset` のローダーは{class}`~ommx.Instance`を返します。Python SDKとPySCIPOpt Adapterをインストールしてください。初回の取得には公開レジストリへの接続が必要です。どちらのデータセットも同じソルバーで解けます。

```{code-cell} ipython3
from math import isclose
from ommx import dataset
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

mip = dataset.miplib2017("neos-1122047")
result = OMMXPySCIPOptAdapter.solve(mip)
published = float(mip.annotations["org.ommx.miplib.objective"])
assert result.feasible
assert isclose(result.objective, published, abs_tol=1e-6)
```

MIPLIBの注釈には、その問題の目的値が文字列で記録されています。数値に変換してから求解結果と比較します。QPLIBでは、`org.ommx.qplib.*` というキーで問題の種類や目的関数の曲率を確認できます。

```{code-cell} ipython3
quadratic = dataset.qplib("3514")
print(quadratic.annotations["org.ommx.qplib.probtype"])
result = OMMXPySCIPOptAdapter.solve(quadratic)
assert result.feasible
```

手元に候補の値がある場合は、{func}`QPLIBの候補を読み込んで評価する例 <ommx.qplib.load_solution>`で、求解せずに候補を検査できます。

(scip-start-comparison)=
## 初期候補の有無でSCIPの結果を比較する

{download}`比較スクリプト <../../examples/compare_scip_start.py>`と{download}`初期候補の値 <../../examples/roll3000-start.json>`を同じディレクトリに保存します。SDKとPySCIPOpt Adapterをインストールした環境で `python compare_scip_start.py` を実行してください。MIPLIBのroll3000を読み込み、初期候補を渡す場合と渡さない場合の両方を30秒の制限で解き、探索時間・終了理由・得られた最良の目的値を表示します。
