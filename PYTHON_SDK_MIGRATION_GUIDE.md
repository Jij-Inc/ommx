# Python SDK v1 to v2 Migration Guide

このドキュメントは、OMMX Python SDKをProtocol Bufferベース（v1）からRust-PyO3ベース（v2）にマイグレーションするためのガイドです。

## インポートの変更

**旧方式 (v1)**:
```python
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1 import Instance, DecisionVariable
```

**新方式 (v2) - 推奨**:
```python
# すべてommx.v1から統一的にインポート
from ommx.v1 import (
    Instance, DecisionVariable, Constraint,
    Function, Linear, Quadratic, Polynomial,
    Solution, State
)
```

## DecisionVariable ファクトリーメソッド

**継続利用可能**:
```python
# of_type メソッド（継続利用可能）
DecisionVariable.of_type(
    DecisionVariable.BINARY, var.idx, 
    lower=var.lb, upper=var.ub, name=var.name
)
```

**新しく追加されたメソッド**:
```python
# より簡潔な型別ファクトリーメソッド
DecisionVariable.binary(var.idx, name=var.name)
DecisionVariable.integer(var.idx, lower=var.lb, upper=var.ub, name=var.name)  
DecisionVariable.continuous(var.idx, lower=var.lb, upper=var.ub, name=var.name)
```

## Function 作成

**旧方式**:
```python
# Protocol Buffer直接作成
Function(constant=constant)
Function(linear=Linear(terms=terms, constant=constant))
```

**新方式**:
```python
# 統一されたコンストラクタ
Function(constant)  # スカラー値から作成
Function(linear)    # Linearオブジェクトから作成
Function(quadratic) # Quadraticオブジェクトから作成

# Linearオブジェクトの作成
linear = Linear(terms=terms, constant=constant)
```

## Constraint 作成

**旧方式**:
```python
# Protocol Buffer直接作成
Constraint(
    id=id,
    equality=Equality.EQUALITY_EQUAL_TO_ZERO,
    function=function,
    name=name,
)
```

**新方式**:
```python
# 直接コンストラクタで作成（ommx.v1.Function を使用）
constraint = Constraint(
    id=id,
    function=function,  # ommx.v1.Function を使用
    equality=Constraint.EQUAL_TO_ZERO,  # Python SDK の定数を使用
    name=name,
)
```

## Function 検査・変換

**旧方式**:
```python
# Protocol Buffer HasField チェック
if function.HasField("linear"):
    linear_terms = function.linear.terms
    constant = function.linear.constant
```

**新方式**:
```python
# Function.degree() を使って多項式の次数を確認し、直接プロパティアクセス
degree = function.degree()
if degree == 0:
    # 定数関数
    constant = function.constant_term
elif degree == 1:
    # 線形関数 - 直接プロパティアクセス
    linear_terms = function.linear_terms      # dict[int, float]
    constant = function.constant_term         # float
elif degree == 2:
    # 二次関数 - 直接プロパティアクセス
    quadratic_terms = function.quadratic_terms  # dict[tuple[int, int], float]
    linear_terms = function.linear_terms        # dict[int, float]
    constant = function.constant_term           # float

# 実際のアダプターでの使用例（PySCIPOpt）:
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


## マイグレーション手順

1. **インポートの更新**: Protocol Buffer直接インポート（`*_pb2`）を削除し、`ommx.v1`からの統一インポートに変更
2. **Function検査の変更**: `.HasField()`を`.degree()`チェックと直接プロパティアクセスに変更
3. **新しいメソッドの活用**: より簡潔な型別ファクトリーメソッド（`binary()`, `integer()`, `continuous()`）を利用可能

## よくある問題と解決策

- **`AttributeError: 'builtins.Function' object has no attribute 'HasField'`**: `.degree()`でチェック後、直接プロパティアクセス（`.linear_terms`, `.constant_term`など）を使用
- **`TypeError: 'float' object is not callable`**: `function.constant_term()`ではなく`function.constant_term`（プロパティ）としてアクセス

## 重要な注意事項

- すべて`ommx.v1`から統一的にインポートし、Protocol Buffer直接インポートは避ける
- 制約の種類判定では次数の小さいものから順にチェック（定数 → 線形 → 二次）

## 新しく利用可能なメソッド

### Function クラス
```python
# 情報取得
function.degree() -> int      # 関数の次数
function.num_terms() -> int   # 項数

# 直接プロパティアクセス（推奨）
function.constant_term      # float - 定数項
function.linear_terms       # dict[int, float] - 線形項の係数
function.quadratic_terms    # dict[tuple[int, int], float] - 二次項の係数

# 評価
function.evaluate(state: State | dict[int, float]) -> float
function.partial_evaluate(state: State | dict[int, float]) -> Function
```

## 推奨実装パターン

```python
# 統一されたインポート
from ommx.v1 import (
    Instance, DecisionVariable, Constraint,
    Function, Linear, Solution, State
)

# DecisionVariable作成 (新しいファクトリーメソッド)
var1 = DecisionVariable.binary(0, name="x1")
var2 = DecisionVariable.integer(1, lower=0, upper=10, name="x2")

# Function検査 (直接プロパティアクセス)
if objective.degree() == 1:
    terms = objective.linear_terms      # dict[int, float]
    constant = objective.constant_term  # float
elif objective.degree() == 2:
    linear_terms = objective.linear_terms        # dict[int, float]
    quadratic_terms = objective.quadratic_terms  # dict[tuple[int, int], float]
    constant = objective.constant_term           # float
```

---

**注意**: v2 APIマイグレーションは完了済み。このガイドは歴史的記録と将来の参考資料として保持されています。