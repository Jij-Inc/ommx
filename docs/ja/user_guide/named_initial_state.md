# 名前と添字で初期値を指定する

SCIP には、変数の名前と添字をキーとする初期値を渡せます。
アダプターが対象インスタンスの変数 ID に変換するため、モデルの再生成で
ID が変わっても同じ入力を再利用できます。

```python markdown-code-runner
from ommx import DecisionVariable, Instance, NamedState
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

x = DecisionVariable.integer(31, lower=0, upper=5, name="x", subscripts=[0])
y = DecisionVariable.integer(107, lower=0, upper=5, name="x", subscripts=[1])
instance = Instance.from_components(
    decision_variables=[x, y], objective=2 * x + y,
    constraints=[x + y <= 5], sense=Instance.MAXIMIZE,
)
initial_values: NamedState = {"x": {(0,): 1.0, (1,): 4.0}}
solution = OMMXPySCIPOptAdapter.solve(instance, initial_state=initial_values)
assert solution.state.entries == {x.id: 5.0, y.id: 0.0}
```

`State` が必要な場合は、`Instance.state_from_named_values()` で明示的に変換できます。

```python markdown-code-runner
state = instance.state_from_named_values(initial_values)
assert state.entries == {x.id: 1.0, y.id: 4.0}
```

添字のキーはタプルにします。スカラーは `()`、1次元は `(i,)`、2次元は `(i, j)` です。
`Solution.extract_all_decision_variables()` の出力も、別のインスタンスの ID に変換できます。

指定した名前・添字が見つからない場合は `KeyError`、複数の変数に一致する場合は
`ValueError` になります。`parameters` と `description` は照合に使いません。
同じ名前・添字の変数を区別するには ID をキーとする `State` を使ってください。
一部の変数だけを指定することもでき、省略した値は補いません。
省略した初期値の扱いはアダプターとソルバーによって異なります。
変換時には有限性、上下限、変数の種類、固定値、依存関係を検証しません。
これらの検証には評価処理を使ってください。インスタンスと入力は変更しません。
