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

巡回セールスマン問題（TSP）は一人のセールスマンが複数の都市を順番に巡る方法を求める問題です。都市間の移動コストが与えられたときコストが最小になる経路を求めます。ここでは次の都市の配置を考えましょう

```{code-cell} ipython3
# From ulysses16.tsp in TSPLIB
ulysses16_points = [
    (38.24, 20.42),
    (39.57, 26.15),
    (40.56, 25.32),
    (36.26, 23.12),
    (33.48, 10.54),
    (37.56, 12.19),
    (38.42, 13.11),
    (37.52, 20.44),
    (41.23, 9.10),
    (41.17, 13.05),
    (36.08, -5.21),
    (38.47, 15.13),
    (38.15, 15.35),
    (37.51, 15.17),
    (35.49, 14.32),
    (39.36, 19.56),
]
```

都市の位置をプロットしてみましょう

```{code-cell} ipython3
%matplotlib inline
from matplotlib import pyplot as plt

x_coords, y_coords = zip(*ulysses16_points)
plt.scatter(x_coords, y_coords)
plt.xlabel('X Coordinate')
plt.ylabel('Y Coordinate')
plt.title('Ulysses16 Points')
plt.show()
```

コストとして単純に移動距離を考えましょう。$i$番目の都市と$j$番目の都市の距離 $d(i, j)$を計算しておきます。

```{code-cell} ipython3
def distance(x, y):
    return ((x[0] - y[0])**2 + (x[1] - y[1])**2)**0.5

# 都市の数
N = len(ulysses16_points)
# 各都市間の距離
d = [[distance(ulysses16_points[i], ulysses16_points[j]) for i in range(N)] for j in range(N)]
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
上で作成したTSPインスタンスには制約があるため、有限のペナルティ重みを指定して
明示的に準備します。その後、`prepared.input` の `Instance` をAdapterへ渡し、
得られたsampleを変換元モデルに対して明示的に評価します。

```{code-cell} ipython3
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

prepared = OMMXOpenJijSAAdapter.prepare(
    instance,
    uniform_penalty_weight=20.0,
)

prepared_samples = OMMXOpenJijSAAdapter.sample(
    prepared.input,
    num_reads=16,
)
sample_set = prepared.evaluate_source(prepared_samples)
sample_set.summary
```

{py:meth}`~ommx_openjij_adapter.OMMXOpenJijSAAdapter.sample` は
{py:class}`~ommx.SampleSet` を返します。これは決定変数のサンプル値に加えて、
評価した目的関数値と制約違反を保持します。`SampleSet.summary` はこの情報の要約を
表示します。`prepared.evaluate_source()` が準備済み入力のsampleを変換元モデルに
対して評価するため、その `feasible` 列は変換元の制約付き問題に対する実行可能性を
示します。

`prepare` に渡すペナルティ重みはOpenJij backend samplerのパラメータではなく、
明示的な準備に対する指定です。有限ペナルティは実行可能なサンプルを得やすく
しますが、すべてのサンプルが変換元の問題に対して実行可能になることを保証しません。

### 準備内容の確認

`check_preparation` はインスタンスを変更せずに、変換元モデルと準備オプションを
検査します。`prepare` は検査した変換を実行し、監査用レポートを
`prepared.report` に保存します。

```{code-cell} ipython3
report = prepared.report
final = report.prepared_input_applicability
{
    "source_membership": report.source_check.input_membership.is_member,
    "preconditions": report.source_check.precondition_violations,
    "steps": [
        (step.operation, step.semantics.value)
        for step in report.steps
    ],
    "prepared_input_applicability": final.is_applicable if final else False,
}
```

レポートは次の3つの問いを区別します。

- `source_check` は、準備元のclassへのmembershipとAdapter固有の準備前提条件を
  記録します。
- `steps` は、各変換とその意味上の効果を記録します。
- `prepared_input_applicability` は、生成された `Instance` がAdapterのinput classに属し、
  Adapter固有の前提条件を満たすかを示します。

各stepは、厳密な書き換えなら `Exact`、厳密な変換ができず離散的な不等式slack
などの近似を使う場合は `Approximate`、制約を有限の目的関数ペナルティで置き換える
場合は `FinitePenalty` です。`FinitePenalty` は制約付き入力を直接、または厳密に
サポートしているという意味ではありません。

変数boundから不等式が実行不可能だと証明できた場合、`check_preparation` と
`prepare` は {py:class}`~ommx.adapter.InfeasibleDetected` を送出します。これは
モデル自体の性質であり、Adapter applicabilityの失敗ではありません。

使用されるInteger変数ごとに最大53個の補助bitという条件を検査しますが、これは
OMMXのInteger-to-Binary log encodingの前提条件です。OpenJij Adapterのinput classの
性質ではなく、serialized semanticsをforward compatibilityのためにreaderが安全に
解釈できるかを管理する `ommx.v2.Feature` とも別物です。OpenJijが直接受け付ける
Spin入力を含むSpin変数のサポートは
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
    xs = [ulysses16_points[i][0] for i in path] + [ulysses16_points[path[0]][0]]
    ys = [ulysses16_points[i][1] for i in path] + [ulysses16_points[path[0]][1]]
    ax.plot(xs, ys, marker='o')
    ax.set_title(f"Sample {s}, objective={sample_set.objectives[s]:.2f}")

plt.tight_layout()
plt.show()
```
