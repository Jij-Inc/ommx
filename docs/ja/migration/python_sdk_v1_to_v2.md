(python-sdk-v1-to-v2-migration-guide)=
# Python SDK v1 から v2 へのマイグレーションガイド

この節では、Protocol Buffer ベースの v1 から Rust + PyO3 ベースの v2 への移行について説明します。

## ⚠️ 重要: `raw` 属性の非推奨化

v2 では、移行済みのすべてのクラス (`Instance`、`Solution`、`SampleSet` など) で `raw` 属性が非推奨になりました。`.raw` への直接アクセスは避けてください。代わりに、各クラス自身が提供する method や property を使います。

**例:**
```python
# ❌ 非推奨: raw 経由でアクセスしない
solution.raw.evaluated_constraints[0].evaluated_value
instance.raw.decision_variables
sample_set.raw.samples

# ✅ 推奨: 直接 property / method を使う
solution.get_constraint_value(0)
instance.decision_variables  # list を返す property になりました
sample_set.get(sample_id)
```

これらの直接 method には次の利点があります。

- native Rust 実装による性能向上
- 型安全性の改善
- より簡潔で直感的な API

後方互換性のため `raw` 属性はまだ利用できますが、将来のバージョンで削除される予定です。

## import の変更

**旧方式 (v1):**
```python
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1 import Instance, DecisionVariable
```

**新方式 (v2) - 推奨:**
```python
# すべて ommx.v1 から統一的に import する
from ommx.v1 import (
    Instance, DecisionVariable, Constraint,
    Function, Linear, Quadratic, Polynomial,
    Solution, State, SampleSet,
    # 新しい evaluated 型 (v2.0.0rc3+)
    EvaluatedDecisionVariable, EvaluatedConstraint,
    SampledDecisionVariable, SampledConstraint
)
```

## DecisionVariable の factory method

**引き続き利用できる method:**
```python
# of_type method (引き続き利用可能)
DecisionVariable.of_type(
    DecisionVariable.BINARY, var.idx,
    lower=var.lb, upper=var.ub, name=var.name
)
```

**新しく追加された method:**
```python
# より簡潔な type-specific factory method
DecisionVariable.binary(var.idx, name=var.name)
DecisionVariable.integer(var.idx, lower=var.lb, upper=var.ub, name=var.name)
DecisionVariable.continuous(var.idx, lower=var.lb, upper=var.ub, name=var.name)
```

## Function の作成

**旧方式:**
```python
# Protocol Buffer object を直接作る
Function(constant=constant)
Function(linear=Linear(terms=terms, constant=constant))
```

**新方式:**
```python
# 統一された constructor
Function(constant)  # scalar value から作成
Function(linear)    # Linear object から作成
Function(quadratic) # Quadratic object から作成

# Linear object の作成
linear = Linear(terms=terms, constant=constant)
```

## Constraint の作成

**旧方式:**
```python
# Protocol Buffer object を直接作る
Constraint(
    id=id,
    equality=Equality.EQUALITY_EQUAL_TO_ZERO,
    function=function,
    name=name,
)
```

**新方式:**
```python
# 直接 constructor で作成する (ommx.v1.Function を使う)
constraint = Constraint(
    id=id,
    function=function,  # ommx.v1.Function を使う
    equality=Constraint.EQUAL_TO_ZERO,  # Python SDK の定数を使う
    name=name,
)
```

## Function の検査と変換

**旧方式:**
```python
# Protocol Buffer の HasField check
if function.HasField("linear"):
    linear_terms = function.linear.terms
    constant = function.linear.constant
```

**新方式:**
```python
# Function.degree() で多項式次数を確認し、直接 property access を使う
degree = function.degree()
if degree == 0:
    # 定数関数
    constant = function.constant_term
elif degree == 1:
    # 線形関数 - 直接 property access
    linear_terms = function.linear_terms      # dict[int, float]
    constant = function.constant_term         # float
elif degree == 2:
    # 二次関数 - 直接 property access
    quadratic_terms = function.quadratic_terms  # dict[tuple[int, int], float]
    linear_terms = function.linear_terms        # dict[int, float]
    constant = function.constant_term           # float

# 実際の adapter 利用例 (PySCIPOpt):
def _make_linear_expr(self, f: Function) -> pyscipopt.Expr:
    return (
        pyscipopt.quicksum(
            coeff * self.varname_map[str(id)]
            for id, coeff in f.linear_terms.items()
        )
        + f.constant_term
    )

def _make_quadratic_expr(self, f: Function) -> pyscipopt.Expr:
    # 二次項
    quad_terms = pyscipopt.quicksum(
        self.varname_map[str(row)] * self.varname_map[str(col)] * coeff
        for (row, col), coeff in f.quadratic_terms.items()
    )
    # 線形項
    linear_terms = pyscipopt.quicksum(
        coeff * self.varname_map[str(var_id)]
        for var_id, coeff in f.linear_terms.items()
    )
    return quad_terms + linear_terms + f.constant_term
```

## 移行手順

1. **import を更新する**: Protocol Buffer からの直接 import (`*_pb2`) を削除し、`ommx.v1` からの統一 import に変更する。
2. **Function の検査方法を変更する**: `.HasField()` を `.degree()` check と直接 property access に置き換える。
3. **新しい method を利用する**: より簡潔な type-specific factory method (`binary()`、`integer()`、`continuous()`) を利用できます。

## よくある問題と解決策

- **`AttributeError: 'builtins.Function' object has no attribute 'HasField'`**: `.degree()` check の後に直接 property access (`.linear_terms`、`.constant_term` など) を使います。
- **`TypeError: 'float' object is not callable`**: `function.constant_term` は method ではなく property としてアクセスします。`function.constant_term()` ではありません。
- **`.raw` 属性の利用**: `raw` 属性は非推奨です。性能と型安全性のため、各クラスが直接提供する method (`solution.get_constraint_value()`、`instance.decision_variables` など) を使ってください。

## 重要な注意点

- すべて `ommx.v1` から統一的に import し、Protocol Buffer からの直接 import を避けてください。
- constraint type を判定する場合は、次数の低い順 (constant → linear → quadratic) に確認してください。

## 新しく利用できる method

### Function class
```python
# 情報取得
function.degree() -> int      # Function の次数
function.num_terms() -> int   # 項数

# 直接 property access (推奨)
function.constant_term      # float - 定数項
function.linear_terms       # dict[int, float] - 線形項の係数
function.quadratic_terms    # dict[tuple[int, int], float] - 二次項の係数

# 評価
function.evaluate(state: State | dict[int, float]) -> float
function.partial_evaluate(state: State | dict[int, float]) -> Function
```

### Solution class (v2.0.0rc3+)
```python
# 新しい list-based property (Instance と一貫)
solution.decision_variables  # list[EvaluatedDecisionVariable] - ID 順に sort 済み
solution.constraints        # list[EvaluatedConstraint] - ID 順に sort 済み

# 新しい個別 access method
solution.get_decision_variable_by_id(variable_id: int) -> EvaluatedDecisionVariable
solution.get_constraint_by_id(constraint_id: int) -> EvaluatedConstraint
```

### SampleSet class (v2.0.0rc3+)
```python
# 新しい list-based property (Instance と一貫)
sample_set.decision_variables # list[SampledDecisionVariable] - ID 順に sort 済み
sample_set.constraints       # list[SampledConstraint] - ID 順に sort 済み

# 新しい個別 access method
sample_set.get_sample_by_id(sample_id: int) -> Solution  # get() の alias
sample_set.get_decision_variable_by_id(variable_id: int) -> SampledDecisionVariable
sample_set.get_constraint_by_id(constraint_id: int) -> SampledConstraint
```

## 推奨される実装 pattern

```python
# 統一 import (v2.0.0rc3+)
from ommx.v1 import (
    Instance, DecisionVariable, Constraint,
    Function, Linear, Solution, State, SampleSet,
    # 一貫した API access のための新しい evaluated 型
    EvaluatedDecisionVariable, EvaluatedConstraint,
    SampledDecisionVariable, SampledConstraint
)

# DecisionVariable の作成 (新しい factory method)
var1 = DecisionVariable.binary(0, name="x1")
var2 = DecisionVariable.integer(1, lower=0, upper=10, name="x2")

# Function の検査 (直接 property access)
if objective.degree() == 1:
    terms = objective.linear_terms      # dict[int, float]
    constant = objective.constant_term  # float
elif objective.degree() == 2:
    linear_terms = objective.linear_terms        # dict[int, float]
    quadratic_terms = objective.quadratic_terms  # dict[tuple[int, int], float]
    constant = objective.constant_term           # float

# 全 class で一貫した API pattern (v2.0.0rc3+)
# 3 つの class はすべて同じ pattern に従います。

# Instance
for var in instance.decision_variables:  # list[DecisionVariable]
    process_variable(var.id, var)
var = instance.get_decision_variable_by_id(var_id)  # DecisionVariable

# Solution
for var in solution.decision_variables:  # list[EvaluatedDecisionVariable]
    process_evaluated_variable(var.id, var.value)
var = solution.get_decision_variable_by_id(var_id)  # EvaluatedDecisionVariable

# SampleSet
for var in sample_set.decision_variables:  # list[SampledDecisionVariable]
    process_sampled_variable(var.id, var.samples)
var = sample_set.get_decision_variable_by_id(var_id)  # SampledDecisionVariable
```

## State constructor の変更 (PyO3 移行)

**改善**: `State(entries=...)` constructor は、`dict[int, float]` と `Iterable[tuple[int, float]]` の両方を受け取れるように拡張されました。

**以前 (Protobuf):**
```python
# protobuf State ではこれらが動作していた
state = State(entries=zip(variables, values))  # ✅ 動作
state = State(entries=[(1, 0.5), (2, 1.0)])   # ✅ 動作
```

**以後 (PyO3) - 拡張 constructor:**
```python
# これらの pattern はすべて拡張 PyO3 State constructor で動作する
state = State(entries=zip(variables, values))        # ✅ iterable で動作
state = State(entries=[(1, 0.5), (2, 1.0)])         # ✅ iterable で動作
state = State(entries=dict(zip(variables, values)))  # ✅ dict で動作
state = State(entries={1: 0.5, 2: 1.0})             # ✅ dict で動作
```

**Adapter code 例:**
```python
# adapter code 内 (例: ommx-openjij-adapter)
def decode_to_samples(response: oj.Response) -> Samples:
    # どちらの pattern も拡張 PyO3 State で動作する:
    state = State(entries=zip(response.variables, sample))           # ✅ 直接動作
    # または
    state = State(entries=dict(zip(response.variables, sample)))     # ✅ こちらも動作
```

**移行状況:**
- ✅ **完了**: `ommx.v1.State` を PyO3 `_ommx_rust.State` へ移行
- ✅ **完了**: State constructor を dict と iterable の両方を受け取れるように拡張
- ✅ **完了**: State constructor 変更に対する adapter 互換性修正
  - ✅ OpenJij adapter: `zip()` と `dict(zip())` の両 pattern に対応
  - ✅ PyScipOpt adapter: protobuf / PyO3 互換性のため `to_state()` function を拡張
  - ✅ legacy protobuf State を含むように `ToState` type alias を拡張
- ✅ **完了**: `ommx.v1.Solution` を PyO3 `_ommx_rust.Solution` へ移行
- ✅ **完了**: `ommx.v1.SampleSet` を PyO3 `_ommx_rust.SampleSet` へ移行

## Solution API の変更

**改善**: `Solution` は constraint と dual variable にアクセスする直接 method を提供するようになりました。

### Constraint value へのアクセス

**以前 (Protobuf):**
```python
# evaluated_constraints list への直接 access
solution.raw.evaluated_constraints[0].evaluated_value  # ❌ 利用不可
```

**以後 (PyO3):**
```python
# 新しい getter method を使う
solution.get_constraint_value(0)  # ✅ ID で constraint value を取得
```

### Dual variable の管理

**以前 (Protobuf):**
```python
# constraint object を直接変更
for constraint in solution.raw.evaluated_constraints:
    constraint.dual_variable = dual_variables[constraint.id]  # ❌ 利用不可
```

**以後 (PyO3):**
```python
# 新しい setter / getter method を使う
solution.set_dual_variable(constraint_id, dual_value)  # ✅ ID で dual variable を設定
solution.get_dual_variable(constraint_id)             # ✅ ID で dual variable を取得
```

### Constraint ID へのアクセス

**以前 (Protobuf):**
```python
# evaluated_constraints を iterate する
for constraint in solution.raw.evaluated_constraints:
    id = constraint.id  # ❌ 利用不可
```

**以後 (PyO3):**
```python
# constraint_ids property を使う
for constraint_id in solution.constraint_ids:  # ✅ constraint ID の set を返す
    value = solution.get_constraint_value(constraint_id)
```

### Adapter 実装例

**HiGHS Adapter:**
```python
# 旧方式
for constraint in solution.raw.evaluated_constraints:
    if constraint.id < row_dual_len:
        constraint.dual_variable = row_dual[constraint.id]

# 新方式
for constraint_id in solution.constraint_ids:
    if constraint_id < row_dual_len:
        solution.set_dual_variable(constraint_id, row_dual[constraint_id])
```

**Python-MIP Adapter:**
```python
# 旧方式
for constraint in solution.raw.evaluated_constraints:
    id = constraint.id
    if id in dual_variables:
        constraint.dual_variable = dual_variables[id]

# 新方式
for constraint_id, dual_value in dual_variables.items():
    solution.set_dual_variable(constraint_id, dual_value)
```

### 完全な Solution API reference

```python
# Properties
solution.objective           # float - objective value
solution.constraint_ids      # set[int] - すべての constraint ID
solution.decision_variable_ids  # set[int] - すべての decision variable ID
solution.feasible           # bool - feasibility status
solution.feasible_relaxed   # bool - relaxed feasibility status

# 新しい list-based property (v2.0.0rc3+)
solution.decision_variables  # list[EvaluatedDecisionVariable] - ID 順に sort 済み
solution.constraints        # list[EvaluatedConstraint] - ID 順に sort 済み

# Methods
solution.get_constraint_value(constraint_id: int) -> float
solution.get_dual_variable(constraint_id: int) -> Optional[float]
solution.set_dual_variable(constraint_id: int, value: Optional[float]) -> None
solution.extract_decision_variables(name: str) -> dict[tuple[int, ...], float]
solution.extract_constraints(name: str) -> dict[tuple[int, ...], float]

# 新しい個別 access method (v2.0.0rc3+)
solution.get_decision_variable_by_id(variable_id: int) -> EvaluatedDecisionVariable
solution.get_constraint_by_id(constraint_id: int) -> EvaluatedConstraint

# State access (後方互換)
solution.state              # variable value を持つ State object
solution.state.entries      # dict[int, float] - variable ID から value への mapping
```

### Solution API の一貫性改善 (v2.0.0rc3+)

**改善**: Solution API は、一貫性のため Instance と同じ pattern に従うようになりました。

**新しい property:**
```python
# List-based access (Instance と一貫)
solution.decision_variables  # list[EvaluatedDecisionVariable] - ID 順に sort 済み
solution.constraints        # list[EvaluatedConstraint] - ID 順に sort 済み

# ID による個別 access (Instance と一貫)
solution.get_decision_variable_by_id(variable_id: int) -> EvaluatedDecisionVariable
solution.get_constraint_by_id(constraint_id: int) -> EvaluatedConstraint
```

**移行 pattern:**
```python
# Before: constraint value への直接 access
solution.get_constraint_value(constraint_id)
solution.get_dual_variable(constraint_id)

# After: constraint object 経由で access (代替 pattern)
constraint = solution.get_constraint_by_id(constraint_id)
value = constraint.evaluated_value
dual_var = constraint.dual_variable

# どちらの pattern も後方互換性のため support されています
```

## SampleSet API reference

`SampleSet` class は、sample へのアクセスと data 抽出のための直接 method を提供するようになりました。

```python
# Properties
sample_set.sample_ids         # set[int] - すべての sample ID
sample_set.feasible_ids       # set[int] - feasible な sample ID
sample_set.best_feasible_id   # Optional[int] - 最良 feasible sample の ID
sample_set.best_feasible      # Optional[Solution] - 最良 feasible solution

# 新しい list-based property (v2.0.0rc3+)
sample_set.decision_variables # list[SampledDecisionVariable] - ID 順に sort 済み
sample_set.constraints       # list[SampledConstraint] - ID 順に sort 済み

# Methods
sample_set.get(sample_id: int) -> Solution
sample_set.extract_decision_variables(name: str, sample_id: int) -> dict[tuple[int, ...], float]
sample_set.extract_constraints(name: str, sample_id: int) -> dict[tuple[int, ...], float]

# 新しい個別 access method (v2.0.0rc3+)
sample_set.get_sample_by_id(sample_id: int) -> Solution  # get() の alias
sample_set.get_decision_variable_by_id(variable_id: int) -> SampledDecisionVariable
sample_set.get_constraint_by_id(constraint_id: int) -> SampledConstraint
```

### SampleSet API の一貫性改善 (v2.0.0rc3+)

**改善**: SampleSet API は、一貫性のため Instance と Solution と同じ pattern に従うようになりました。

**新しい property:**
```python
# List-based access (Instance と Solution と一貫)
sample_set.decision_variables # list[SampledDecisionVariable] - ID 順に sort 済み
sample_set.constraints       # list[SampledConstraint] - ID 順に sort 済み

# ID による個別 access (Instance と Solution と一貫)
sample_set.get_sample_by_id(sample_id: int) -> Solution  # 既存の get() の alias
sample_set.get_decision_variable_by_id(variable_id: int) -> SampledDecisionVariable
sample_set.get_constraint_by_id(constraint_id: int) -> SampledConstraint
```

**API 一貫性の達成:**
3 つの core class はすべて同じ pattern に従うようになりました。

- **Instance**: `decision_variables` → `list[DecisionVariable]`、`get_decision_variable_by_id()` → `DecisionVariable`
- **Solution**: `decision_variables` → `list[EvaluatedDecisionVariable]`、`get_decision_variable_by_id()` → `EvaluatedDecisionVariable`
- **SampleSet**: `decision_variables` → `list[SampledDecisionVariable]`、`get_decision_variable_by_id()` → `SampledDecisionVariable`

同じ pattern が constraint と他の access method にも適用されます。

## Adapter API の変更 (v2.0-rc.4)

**破壊的変更**: Instance API の method が property に変わり、返り値の型も dictionary から list に変わりました。

### Instance API の変更

**Before:**
```python
# dictionary を返す method
for var_id, var in instance.decision_variables().items():
    process_variable(var_id, var)

for constraint_id, constraint in instance.constraints().items():
    process_constraint(constraint_id, constraint)

# iteration のための raw access
for var_id, var in instance.raw.decision_variables.items():
    process_variable(var_id, var)
```

**After:**
```python
# list を返す property (ID 順)
for var in instance.decision_variables:
    process_variable(var.id, var)

for constraint in instance.constraints:
    process_constraint(constraint.id, constraint)

# raw access は不要 - property を直接使う
for var in instance.decision_variables:
    process_variable(var.id, var)
```

### Instance sense への access

**Before:**
```python
if instance.raw.sense == Instance.MAXIMIZE:
    # maximize を処理
elif instance.raw.sense == Instance.MINIMIZE:
    # minimize を処理
```

**After:**
```python
if instance.sense == Instance.MAXIMIZE:
    # maximize を処理
elif instance.sense == Instance.MINIMIZE:
    # minimize を処理
```

### Adapter 実装の更新

**Python-MIP Adapter:**
```python
# Before
def _set_decision_variables(self):
    for var_id, var in self.instance.raw.decision_variables.items():
        # variable を処理

# After
def _set_decision_variables(self):
    for var in self.instance.decision_variables:
        # var.id と var property を使って variable を処理
```

**State 作成 pattern:**
```python
# Before
return State(entries={
    var_id: data.var_by_name(str(var_id)).x
    for var_id, var in self.instance.raw.decision_variables.items()
})

# After
return State(entries={
    var.id: data.var_by_name(str(var.id)).x
    for var in self.instance.decision_variables
})
```

### 新しい Instance method

**個別 access:**
```python
# ID で特定の item を取得する (ID がない場合は KeyError)
var = instance.get_decision_variable_by_id(variable_id)  # 個別 variable access
constraint = instance.get_constraint_by_id(constraint_id)  # 個別 constraint access
removed_constraint = instance.get_removed_constraint(constraint_id)  # 個別 removed constraint access
```

### Adapter で必要な変更

1. **`.raw` access を置き換える**: `.raw.decision_variables.items()` の代わりに直接 property を使う。
2. **iteration pattern を更新する**: `dict.items()` から list の直接 iteration に変更する。
3. **個別 ID に access する**: dict key の代わりに各 object の `.id` property を使う。
4. **sense access を更新する**: `instance.raw.sense` ではなく `instance.sense` を使う。
5. **新しい個別 access method を使う**: `get_decision_variable_by_id()` と `get_constraint_by_id()` を使って個別 item に access する。

### Adapter 向け移行 checklist

- [ ] `instance.raw.decision_variables.items()` を `instance.decision_variables` に置き換える。
- [ ] `instance.raw.constraints.items()` を `instance.constraints` に置き換える。
- [ ] `instance.raw.sense` を `instance.sense` に置き換える。
- [ ] variable access を `(var_id, var)` から `var` に更新する (`var.id` を使う)。
- [ ] constraint access を `(constraint_id, constraint)` から `constraint` に更新する (`constraint.id` を使う)。
- [ ] test assertion を `len(instance.decision_variables())` から `len(instance.decision_variables)` に更新する。
- [ ] 個別 variable access には `instance.get_decision_variable_by_id(id)` を使う。
- [ ] 個別 constraint access には `instance.get_constraint_by_id(id)` を使う。
