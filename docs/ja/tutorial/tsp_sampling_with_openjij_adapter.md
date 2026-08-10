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

# OMMX AdapterでQUBOからサンプリングする

ここでは巡回セールスマン問題を例として、問題をQUBOに変換しサンプリングを行う方法を説明します。

```{figure} ./assets/taraimawashi_businessman.png
[たらい回しのイラスト（スーツ・男性）](https://www.irasutoya.com/2017/03/blog-post_739.html)
```

巡回セールスマン問題（TSP）は一人のセールスマンが複数の都市を順番に巡る方法を求める問題です。都市間の移動コストが与えられたときコストが最小になる経路を求めます。ここでは自己完結した例として、固定した乱数seedを使って10×10の領域に16都市を再現可能な形で生成します。

```{code-cell} ipython3
from random import Random

N = 16
rng = Random(42)
city_points = [
    (rng.uniform(0.0, 10.0), rng.uniform(0.0, 10.0))
    for _ in range(N)
]
```

都市の位置をプロットしてみましょう

```{code-cell} ipython3
%matplotlib inline
from matplotlib import pyplot as plt

x_coords, y_coords = zip(*city_points)
plt.scatter(x_coords, y_coords)
plt.xlabel('X Coordinate')
plt.ylabel('Y Coordinate')
plt.title('ランダム生成した都市配置')
plt.show()
```

コストとして単純に移動距離を考えましょう。$i$番目の都市と$j$番目の都市の距離 $d(i, j)$を計算しておきます。

```{code-cell} ipython3
def distance(x, y):
    return ((x[0] - y[0])**2 + (x[1] - y[1])**2)**0.5

# 各都市間の距離
d = [[distance(city_points[i], city_points[j]) for i in range(N)] for j in range(N)]
```

これを使って次のような最適化問題としてTSPを定式化します。まずある時刻 $t$ に都市 $i$ にいるかどうかをバイナリ変数 $x_{t, i}$ で表します。このとき、以下の制約を満たすような $x_{t, i}$ を求めます。するとセールスマンが移動する距離は次で与えられます：

$$
\sum_{t=0}^{N-1} \sum_{i, j = 0}^{N-1} d(i, j) x_{t, i} x_{(t+1 \% N), j}
$$

ただし $x_{t, i}$ は自由に取れるわけではなく、各時刻 $t$ において一箇所の都市にしかいられないという制約と各都市について一度だけ訪れるという制約

$$
\sum_{i=0}^{N-1} x_{t, i} = 1, \quad \sum_{t=0}^{N-1} x_{t, i} = 1
$$

を満たす必要があります。これらを合わせてTSPは制約付き最適化問題として定式化できます

$$
\begin{aligned}
\min \quad & \sum_{t=0}^{N-1} \sum_{i, j = 0}^{N-1} d(i, j) x_{t, i} x_{(t+1 \% N), j} \\
\text{s.t.} \quad & \sum_{i=0}^{N-1} x_{t, i} = 1 \quad (\forall t = 0, \ldots, N-1) \\
\quad & \sum_{t=0}^{N-1} x_{t, i} = 1 \quad (\forall i = 0, \ldots, N-1)
\end{aligned}
$$

これに対応する `ommx.Instance` は次のように作成できます

```{code-cell} ipython3
from ommx import DecisionVariable, Instance

x = [[
        DecisionVariable.binary(
            i + N * t,  # 決定変数のID
            name="x",           # 決定変数の名前、解を取り出すときに使う
            subscripts=[t, i])  # 決定変数の添字、解を取り出すときに使う
        for i in range(N)
    ]
    for t in range(N)
]

objective = sum(
    d[i][j] * x[t][i] * x[(t+1) % N][j]
    for i in range(N)
    for j in range(N)
    for t in range(N)
)
place_constraint = {
    t: (sum(x[t][i] for i in range(N)) == 1)
        .set_name("place")
        .add_subscripts([t])
    for t in range(N)
}
time_constraint = {
    i + N: (sum(x[t][i] for t in range(N)) == 1)
        .set_name("time")
        .add_subscripts([i])
    for i in range(N)
}

instance = Instance.from_components(
    decision_variables=[x[t][i] for i in range(N) for t in range(N)],
    objective=objective,
    constraints={**place_constraint, **time_constraint},
    sense=Instance.MINIMIZE
)
```

バイナリの決定変数の作成時 `DecisionVariable.binary` に追加した決定変数の名前と添字は後で得られたサンプルを解釈する際に使います。

+++


## OpenJijによるサンプリング

`ommx-openjij-adapter` のinput classに属するのは、任意次数の多項式目的関数を
持つバイナリ変数のみの制約なし最小化問題です。
上で作成したTSPインスタンスには制約があるため、Adapterの推奨Policyを取得し、
有限のpenalty weightを選択して、同じinstanceをin-placeでprepareしてからsampleします。

```{code-cell} ipython3
from ommx import FixedPenaltyPreparation
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

input_class = OMMXOpenJijSAAdapter.INPUT_CLASS
assert input_class is not None
policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
policy.fixed_penalty = (
    FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(
        weight=20.0,
    )
)
instance.prepare(input_class, policy)
OMMXOpenJijSAAdapter.require_applicable(instance)

sample_set = OMMXOpenJijSAAdapter.sample(
    instance,
    num_reads=16,
)
sample_set.summary
```

{py:meth}`~ommx_openjij_adapter.OMMXOpenJijSAAdapter.sample` は
{py:class}`~ommx.SampleSet` を返します。これは決定変数のサンプル値に加えて、
評価した目的関数値と制約違反を保持します。`SampleSet.summary` はこの情報の要約を
表示します。in-placeでprepareされたinstanceはremoved constraintと変数dependencyを
評価用に保持するため、`feasible` 列にはTSP制約の違反も引き続き表示されます。

`policy` のpenalty weightはOpenJij backend samplerのパラメータではなく、明示的な
OMMX preparationに対する指定です。有限penaltyは実行可能なsampleを得やすくしますが、
すべてのsampleが実行可能になることを保証しません。

### 推奨Policyの編集

推奨Policyでは、OpenJijで一般に必要となるmodel変換、すなわちIndicator、OneHot、
SOS1制約のlowering、optimization senseの正規化、Integer slack、使用中の全Integer
変数のlog encodingを有効にします。finite penaltyの値はapplication固有なので、
推奨Policyでは無効のままです。呼び出すたびに新しいPolicyが返るため、編集しても
共有されたAdapter設定は変わりません。

Integer slackはまず、最大range 32で各inequalityをequalityへ厳密に変換しようとします。
推奨Policyは、厳密な変換が利用できない場合に、上限32のInteger slackを追加したまま
inequalityとして残すことも許可します。これはsimulated annealingで一般に有用な選択です。
applicationがequalityを必須とする場合は `policy.integer_slack` を明示的に変更します。

{py:meth}`~ommx.Instance.prepare` はinstanceがAdapter input classに含まれた場合だけ
returnします。OMMXの各model operationで発生した既存exceptionをそのまま送出し、
先に完了したoperationを後のerrorでrollbackしません。signed IDとfinite coefficient
というOpenJij固有の条件は構造的input classに含まれないため、通常のAdapter
applicability checkは引き続き必要です。

Integer log encodingのlimitはoperation availabilityの条件であり、OpenJij Adapterの
input classや `ommx.v2.Feature` の性質ではありません。OpenJijが直接受け付けるSpin
入力を含むSpin変数のサポートは
[OMMX issue #1082](https://github.com/Jij-Inc/ommx/issues/1082) で別途管理しています。

各制約条件毎のfeasibilityを見るには `summary_with_constraints` プロパティを使います。

```{code-cell} ipython3
sample_set.summary_with_constraints
```

より詳しい情報は `SampleSet.decision_variables_df()` 及び `SampleSet.constraints_df()` メソッドを使って取得できます。

```{code-cell} ipython3
sample_set.decision_variables_df().head(2)
```

```{code-cell} ipython3
sample_set.constraints_df().head(2)
```

得られたサンプルを取得するには `SampleSet.extract_decision_variables` メソッドを使います。これは `ommx.DecisionVariables` を作る時に登録した `name` と `subscripts` を使ってサンプルを解釈します。例えば `sample_id=1` の `x` という名前の決定変数の値を取得するには次のようにすると `dict[subscripts, value]` の形で取得できます。

```{code-cell} ipython3
sample_id = 1
x = sample_set.extract_decision_variables("x", sample_id)
t = 2
i = 3
x[(t, i)]
```

$x_{t, i}$に対するサンプルが得れたのでこれをTSPのパスに変換します。これは今回の定式化自体に依存するので自分で処理を書く必要があります。

```{code-cell} ipython3
def sample_to_path(sample: dict[tuple[int, ...], float]) -> list[int]:
    path = []
    for t in range(N):
        for i in range(N):
            if sample[(t, i)] == 1:
                path.append(i)
    return path
```

これを表示してみましょう。まず元の問題に対してfeasibleであるサンプルのIDを取得します。

```{code-cell} ipython3
feasible_ids = sample_set.summary.query("feasible == True").index
feasible_ids
```

これらについて最適化された経路を表示しましょう

```{code-cell} ipython3
fig, axie = plt.subplots(3, 3, figsize=(12, 12))

for i, ax in enumerate(axie.flatten()):
    if i >= len(feasible_ids):
        break
    s = feasible_ids[i]
    x = sample_set.extract_decision_variables("x", s)
    path = sample_to_path(x)
    xs = [city_points[i][0] for i in path] + [city_points[path[0]][0]]
    ys = [city_points[i][1] for i in path] + [city_points[path[0]][1]]
    ax.plot(xs, ys, marker='o')
    ax.set_title(f"Sample {s}, objective={sample_set.objectives[s]:.2f}")

plt.tight_layout()
plt.show()
```
