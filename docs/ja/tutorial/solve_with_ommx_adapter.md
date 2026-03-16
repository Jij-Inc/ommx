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

# OMMX Adapterで最適化問題を解く

OMMXでは、既存の数理最適化ツールと相互連携するためのソフトウェアとしてOMMX Adapterを提供しています。OMMX Adapterを使うことで、OMMXが規定するスキーマで表現された最適化問題を既存の数理最適化ツールに入力可能にしたり、既存の数理最適化ツールから得られた情報をOMMXが規定するスキーマに変換したりすることができます。

ここでは、0-1ナップサック問題をOMMX PySCIPOpt Adapterを介して解く方法を紹介します。


## 必要なライブラリのインストール

まず、OMMX PySCIPOpt Adapterを準備しましょう。以下のコマンドでインストールできます。

```
pip install ommx-pyscipopt-adapter
```

+++

## 最適化計算を実行するための2つのステップ

```{figure} ./assets/solve_with_ommx_adapter_01.png
:alt: 0-1ナップサック問題をOMMX PySCIPOpt Adapterで解くフロー

0-1ナップサック問題をOMMX PySCIPOpt Adapterで解くフロー。
```

OMMX PySCIPOpt Adapterを介して0-1ナップサック問題を解くためには、次の2つのステップを踏む必要があります：

1. 0-1ナップサック問題のインスタンスを用意する
2. OMMX Adapterを介して最適化計算を実行する

ステップ1.では、OMMX MessageのInstanceスキーマで定義された `ommx.v1.Instance` オブジェクトを作成します。このオブジェクトを作成する方法は複数ありますが、ここではOMMX Python SDKを使用して直接記述する方法を採用します。

```{tip}
`ommx.v1.Instance` オブジェクトを用意する方法は4つあります：

1. OMMX Python SDKを使って `ommx.v1.Instance` を直接記述する
2. OMMX Python SDKを使ってMPSファイルを `ommx.v1.Instance` に変換する
3. 数理最適化ツールで記述した問題インスタンスをOMMX Adapterで `ommx.v1.Instance` に変換する
4. JijModelingを使って `ommx.v1.Instance` を出力する
```

ステップ2.では、 `ommx.v1.Instance` オブジェクトをPySCIPOptの `Model` オブジェクトに変換し、SCIPによる最適化計算を実行します。計算結果は、OMMX MessageのSolutionスキーマで定義された `ommx.v1.Solution` オブジェクトとして取得できます。

### ステップ1: 0-1ナップサック問題のインスタンスを用意する

0-1ナップサック問題は以下のように定式化されます：

$$
\begin{align*}
\mathrm{maximize} \quad & \sum_{i=0}^{N-1} v_i x_i \\
\mathrm{s.t.} \quad & \sum_{i=0}^{n-1} w_i x_i - W \leq 0, \\
& x_{i} \in \{ 0, 1\} 
\end{align*}
$$

+++

ここでは、この数理モデルのパラメータに以下のデータを設定することとします:

```{code-cell} ipython3
# 0-1ナップサック問題のデータ
v = [10, 13, 18, 31, 7, 15]   # 各アイテムの価値
w = [11, 25, 20, 35, 10, 33] # 各アイテムの重さ
W = 47  # ナップサックの耐荷重
N = len(v)  # アイテムの総数
```

この数理モデルとデータに基づいて、OMMX Python SDKを使用して問題インスタンスを記述するコードは次のようになります：

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable

# 決定変数を定義する
x = [
    # バイナリ変数 x_i を定義する
    DecisionVariable.binary(
        # 決定変数のIDを指定する
        id=i,
        # 決定変数の名前を指定する
        name="x",
        # 決定変数の添え字を指定する
        subscripts=[i],
    )
    # バイナリ変数をアイテムの個数だけ用意する
    for i in range(N)
]

# 目的関数を定義する
objective = sum(v[i] * x[i] for i in range(N))

# 制約条件を定義する
constraint = (sum(w[i] * x[i] for i in range(N)) <= W).add_name("重量制限")

# インスタンスを作成する
instance = Instance.from_components(
    # インスタンスに含まれる全ての決定変数を登録する
    decision_variables=x,
    # 目的関数を登録する
    objective=objective,
    # 制約条件を登録する
    constraints=[constraint],
    # 最大化問題であることを指定する
    sense=Instance.MAXIMIZE,
)
```

### ステップ2: OMMX Adapterを使って最適化計算を実行する

ステップ1.で用意したインスタンスを最適化するには、次のようにOMMX PySCIPOpt Adapterを介して最適化計算を実行します:

```{code-cell} ipython3
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

# PySCIPOptのModelを介してommx.v1.Solutionを取得する
solution = OMMXPySCIPOptAdapter.solve(instance)
```

ここで得られた変数 `solution` は、SCIPによる最適化計算の結果が格納された `ommx.v1.Solution` オブジェクトになっています。

+++

## 結果を分析する

ステップ2. で得られた計算結果から

- 最適解（アイテムの価値の合計が最も高くなるようなアイテムの選び方）
- 最適値（最も高いアイテムの価値の合計）
- 制約条件（重量制限に対するアイテムの重さの合計の余裕）

を確認・分析するためには、`ommx.v1.Solution` クラスに実装されているプロパティを使用します。

### 最適解の分析

`decision_variables` プロパティは、決定変数のID、種類、名前、値などの情報を含む `pandas.DataFrame` オブジェクトを返します：

```{code-cell} ipython3
solution.decision_variables_df
```

この `pandas.DataFrame` オブジェクトを使うことで、例えば「アイテムをナップサックに入れるかどうか」という判断をまとめた表を pandas で簡単に作成できます：

```{code-cell} ipython3
import pandas as pd

df = solution.decision_variables_df
pd.DataFrame.from_dict(
    {
        "アイテムの番号": df.index,
        "ナップサックに入れるか？": df["value"].apply(lambda x: "入れる" if x == 1.0 else "入れない"),
    }
)
```

この分析結果から、ナップサックの重量制限を満たしながらアイテムの価値の合計を最大化するためには、0番目と3番目のアイテムを選択すればよいことが分かります。

### 最適値の分析

`objective` プロパティには最適値が格納されています。今回のケースでは、0番目と3番目のアイテムの価値の合計値が格納されているはずです：

```{code-cell} ipython3
import numpy as np
# 期待される値は0番目と3番目のアイテムの価値の合計値である
expected = v[0] + v[3]
assert np.isclose(solution.objective, expected)
```

### 制約条件の分析

`constraints` プロパティは、制約条件の等号不等号、左辺の値 (`"value"`)、名前などの情報を含む `pandas.DataFrame` オブジェクトを返します：

```{code-cell} ipython3
solution.constraints_df
```

特に `"value"` は制約条件にどの程度の余裕があるのかを知るために便利です。今回のケースでは、0番目のアイテム $w_0$ の重さが `11`、3番目のアイテムの重さ $w_3$ が `35` であり、ナップサックの耐荷重 $W$ は `47` なので、重量制約

$$
\begin{align*}
\sum_{i=0}^{n-1} w_i x_i - W \leq 0
\end{align*}
$$

の左辺の値 `"value"` は `-1` となり、重量制限に対して `1` だけ余裕があることがわかります。
