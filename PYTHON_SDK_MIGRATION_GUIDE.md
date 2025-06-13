# Python SDK v1 to v2 Migration Guide

このドキュメントは、OMMX Python SDKをProtocol Bufferベース（v1）からRust-PyO3ベース（v2）にマイグレーションするための包括的なガイドです。

## 概要

OMMX Python SDKのPhase 4完了により、コアSDKは新しいRust-PyO3実装に移行されました。この変更により、Protocol Bufferに依存するアダプター（solver adapters）も新しいAPIに更新する必要があります。

## 対象範囲

このガイドは、Protocol Bufferベース（v1）からRust-PyO3ベース（v2）へのアダプター移行に適用されます。

## 重要な方針

### Raw APIの非推奨とPython SDKの拡張

v2への移行では、以下の方針を推奨します：

1. **`_ommx_rust`モジュールの直接使用を避ける**: 内部実装の詳細に依存することを防ぐため
2. **`ommx.v1`モジュールの統一されたAPIを使用**: 安定したパブリックAPIを利用
3. **必要なAPIがない場合はPython SDKに追加**: raw APIを使うのではなく、適切なラッパーメソッドを追加

この方針により、将来的な内部実装の変更に対して堅牢なコードを維持できます。

## 主要な変更点

### 1. インポートの変更

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

**注意**: `_ommx_rust`モジュールからの直接インポートは避けてください。

### 2. DecisionVariable ファクトリーメソッド

**旧方式**:
```python
# 古いof_typeファクトリーメソッド
DecisionVariable.of_type(
    DecisionVariable.BINARY, var.idx, 
    lower=var.lb, upper=var.ub, name=var.name
)
```

**新方式**:
```python
# 新しい型別ファクトリーメソッド
DecisionVariable.binary(var.idx, name=var.name)
DecisionVariable.integer(var.idx, lower=var.lb, upper=var.ub, name=var.name)  
DecisionVariable.continuous(var.idx, lower=var.lb, upper=var.ub, name=var.name)
```

### 3. Function 作成

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

### 4. Constraint 作成

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

### 5. Enum 定数

**変更不要**: 以下の定数は互換性が保たれているため、そのまま使用可能です：
- `Instance.MAXIMIZE` / `Instance.MINIMIZE`
- `Constraint.EQUAL_TO_ZERO` / `Constraint.LESS_THAN_OR_EQUAL_TO_ZERO`
- `DecisionVariable.BINARY` / `DecisionVariable.INTEGER` / `DecisionVariable.CONTINUOUS`

### 6. Function 検査・変換

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

### 7. 属性アクセス

**主な変更点**:
- `.raw`属性アクセスが不要に（一部のケースを除く）
- 多くの属性はそのまま使用可能

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

# 型変換メソッド（通常は不要）
function.as_linear() -> Optional[Linear]      # degree()とプロパティアクセスの使用を推奨
function.as_quadratic() -> Optional[Quadratic]  # degree()とプロパティアクセスの使用を推奨
```

## マイグレーション手順

### ステップ 1: インポートの更新
1. Protocol Buffer直接インポート（`*_pb2`）を削除
2. `_ommx_rust`からの直接インポートを避ける
3. すべて`ommx.v1`からインポートするように変更
4. `Sense`と`Equality`のインポートは不要（Python SDK定数を使用）

### ステップ 2: ファクトリーメソッドの更新
1. `DecisionVariable.of_type()`を型別メソッドに変更
2. `Function`と`Constraint`の直接作成をファクトリーメソッドに変更

### ステップ 3: Enum定数の確認
- Enum定数は互換性が保たれているため変更不要

### ステップ 4: Protocol Buffer API除去
1. `.HasField()`呼び出しを`.degree()`チェックと直接プロパティアクセスに変更
2. `.raw`属性アクセスを直接アクセスに変更
3. フィールド直接アクセスをプロパティアクセスに変更

### ステップ 5: テストの更新
1. テストの期待値を新しいAPI仕様に合わせて更新
2. 属性アクセスパターンの変更

## 一般的な問題と解決策

### 問題 1: `'int' object has no attribute 'kind'`
**原因**: DecisionVariableがラッパーでなく生IDを返している
**解決**: インスタンス作成方法とアクセス方法を確認

### 問題 2: `AttributeError: 'builtins.Function' object has no attribute 'HasField'`
**原因**: 新しいFunctionクラスにProtocol Bufferメソッドがない
**解決**: `.degree()`でチェック後、直接プロパティアクセス（`.linear_terms`, `.constant_term`など）



### 問題 3: `TypeError: 'float' object is not callable`
**原因**: `function.constant_term()`をメソッドとして呼び出している
**解決**: プロパティとしてアクセス（`function.constant_term`）

## パフォーマンス向上

新しいv2 APIの利点：
- **高速な数学演算**: Rustでの最適化実装
- **メモリ効率**: Protocol Bufferオーバーヘッドの削除
- **型安全性**: PyO3による堅牢な型システム
- **現代的API**: よりPythonicなインターフェース

## 移行検証

マイグレーション完了後の検証方法：
```bash
# 各アダプターのテスト実行
task python:ommx-python-mip-adapter:test
task python:ommx-pyscipopt-adapter:test
task python:ommx-highs-adapter:test
task python:ommx-openjij-adapter:test

# 全体テスト
task python:test
```

## 重要な注意事項

### Function 検査の変更
`.HasField("linear")`の代わりに`.degree()`チェックと直接プロパティアクセスを使用

### Import方針
すべて`ommx.v1`から統一的にインポートし、`_ommx_rust`からの直接インポートは避ける

### 制約処理順序
制約の種類判定では次数の小さいものから順にチェック：

```python
if constraint_func.degree() == 0:              # 定数制約
    # 定数制約バリデーション
elif constraint_func.degree() == 1:            # 線形制約
    expr = self._make_linear_expr(constraint_func)
elif constraint_func.degree() == 2:            # 二次制約
    expr = self._make_quadratic_expr(constraint_func)
```


## 検証戦略
**発見**: `instance.objective.as_linear()` は不可、`.raw` 経由でアクセス必要

**修正パターン**:
```python
# 間違ったアクセス
linear_func = instance.objective.as_linear()  # AttributeError

# 正しいアクセス方法
linear_func = instance.raw.objective.as_linear()  # 正常動作
```

### 14. 変数ID一致の重要性
**発見**: Function内で使用する変数IDは決定変数リストと厳密に一致する必要
- **エラー例**: `RuntimeError: Undefined variable ID is used: VariableID(1)`

### 15. Pyright型エラー修正とAPI改善
**発見**: 型システム間の変換とPyrightエラーの適切な対処方法

**重要な改善**: `Instance.from_components()` の型アノテーションと実装を修正
```python
# ommx/v1/__init__.py の修正
def from_components(
    *,
    objective: int | float | DecisionVariable | Linear | Quadratic | Polynomial | Function | _Function | _ommx_rust.Function,  # ← 追加
    # ...
):
    if isinstance(objective, _ommx_rust.Function):
        objective = Function.from_raw(objective)
    # ...
```

この改善により、他のアダプターでも`ommx.v1.Function`を直接使用可能になりました。

### 16. 制約処理の順序（重要なバグパターン）

**発見**: PySCIPOptアダプターで発見された重要なバグ - 制約処理の順序が重要

**問題**: 線形制約検出が定数制約検証より先に実行されるため、定数制約の妥当性チェックがスキップされる

**修正前（バグ）**:
```python
if constraint_func.degree() >= 1:                  # 線形以上の次数をチェック
    expr = self._make_linear_expr(constraint_func)  # 線形として処理
elif constraint_func.degree() == 0:                # 定数の場合は到達しない
    # 妥当性検証ロジック（実行されない）
```

**修正後（正しい）**:
```python
if constraint_func.degree() == 0:                  # 定数を最初にチェック
    # 適切な定数制約妥当性検証
elif constraint_func.degree() == 1:                # 線形関数
    expr = self._make_linear_expr(constraint_func)
elif constraint_func.degree() == 2:                # 二次関数
    expr = self._make_quadratic_expr(constraint_func)
```

**影響**: このバグにより、数学的に不可能な問題（`-1 = 0`など）が適切に検証されずにソルバーに渡される可能性があった。

### 17. 浮動小数点表現の一貫性

**発見**: テスト期待値での浮動小数点表現の違い（`-0.0` vs `0.0`）

**対処法**:
```python
# doctestでの期待値修正
>>> state.entries
{1: -0.0}  # HiGHSが返す実際の値に合わせる
```

### 18. エラーメッセージの更新

**発見**: v2 APIでエラーメッセージが変更されている

**修正例**:
```python
# 旧: "The function must be either `constant` or `linear`."
# 新: "HiGHS Adapter currently only supports linear problems"
assert "HiGHS Adapter currently only supports linear problems" in str(e.value)
```

### 19. 不要なテストファイルの判別基準

**判断基準**: 以下の条件を満たすテストファイルは削除対象
1. `_ommx_rust`を直接使用（ベストプラクティス違反）
2. 上位APIテストで間接的にカバー済み
3. ユーザーが使用しない内部実装詳細をテスト
4. メンテナンス負荷が価値を上回る

**例**: `test_instance_wrapper.py` - PyO3バインディングの低レベルテスト
- 削除理由: 上位Instance APIテストで間接的にテスト済み、内部実装の詳細

### 20. Raw APIからPython SDKへの移行

**重要な変更**: `_ommx_rust`モジュールの直接使用を避け、必要なAPIはPython SDKに追加

**実装例**: Python MIP Adapterでの実装
```python
# 統一されたommx.v1 APIを使用
from ommx.v1 import Function, Linear, Instance, DecisionVariable, Constraint

# Python SDKに追加されたメソッド
function.degree()          # 関数の次数
function.num_terms()       # 項数
function.as_linear()       # 線形関数への変換
linear.constant_term       # 定数項（プロパティ）
linear.linear_terms        # 線形項（プロパティ）
```

**メリット**:
- 内部実装の変更に対して堅牢
- 一貫性のあるAPI設計
- 型安全性の向上

## 検証戦略

### 段階的検証
1. **構文チェック**: `python -m py_compile` でファイル単位確認
2. **インポートテスト**: `python -c "import adapter_module"`
3. **単体テスト**: 個別テストファイル実行
4. **統合テスト**: 全体テストスイート実行

### 回帰テスト
```bash
# 修正前後の動作比較
task python:adapter:test > before.log 2>&1
# 修正作業
task python:adapter:test > after.log 2>&1
diff before.log after.log
```

## まとめ

### 推奨されるベストプラクティス

1. **統一されたインポート**: すべて`ommx.v1`から
2. **Raw APIの回避**: `_ommx_rust`の直接使用を避けPython SDK経由でアクセス
3. **Python SDKの拡張**: 必要なAPIはPython SDKに追加
4. **型安全性**: PyO3 Enumとプロパティアクセスで型安全性を実現

### 推奨実装パターン
```python
# 統一されたインポート
from ommx.v1 import (
    Instance, DecisionVariable, Constraint,
    Function, Linear, Kind, Solution, State
)

# DecisionVariable作成 (新しいファクトリーメソッド)
var1 = DecisionVariable.binary(0, name="x1")
var2 = DecisionVariable.integer(1, lower=0, upper=10, name="x2")

# Function作成 (プロパティアクセス)
linear = Linear(terms={0: 1.0, 1: 2.0}, constant=3.0)
objective = Function(linear)

# Function検査 (直接プロパティアクセス)
if objective.degree() == 1:
    terms = objective.linear_terms      # dict[int, float] - プロパティ
    constant = objective.constant_term  # float - プロパティ
elif objective.degree() == 2:
    linear_terms = objective.linear_terms        # dict[int, float]
    quadratic_terms = objective.quadratic_terms  # dict[tuple[int, int], float]
    constant = objective.constant_term           # float

# Constraint作成
constraint = Constraint(
    id=0,
    function=objective,
    equality=Constraint.EQUAL_TO_ZERO,
    name="my_constraint"
)

# Instance作成
instance = Instance.from_components(
    decision_variables=[var1, var2],
    objective=objective,
    constraints=[constraint],
    sense=Instance.MINIMIZE
)
```


## マイグレーション完了状況 (December 2024)

### ✅ 完了済みアダプター
1. **Python-MIP Adapter**: 完全移行完了、ベストプラクティス確立
2. **PySCIPOpt Adapter**: 完全移行完了、重要なバグ修正含む
3. **HiGHS Adapter**: API移行完了、包括的ドキュメント追加

### 🎉 主な成果
- **API統一**: すべてのアダプターで `ommx.v1` 統一インポート
- **型安全性向上**: PyO3 enumsとプロパティアクセス
- **パフォーマンス向上**: Rust実装による高速化
- **メンテナンス性**: `_ommx_rust` 直接使用の撤廃
- **ドキュメント**: 包括的移行ガイドと仕様書

### 確立されたベストプラクティス
1. **Import Standards**: Protocol Buffer直接インポートの廃止
2. **API Extension**: 必要機能のPython SDK追加パターン
3. **Test Patterns**: 不要な低レベルテストの削除基準
4. **Error Handling**: 制約処理順序の重要性
5. **Documentation**: 具体的使用例とAPI仕様の明記

---

このガイドは実際のマイグレーション作業から得られた知見に基づいており、今後のOMMMX開発において同様の問題を効率的に解決するために活用できます。特に、raw APIを使わずPython SDKの統一されたAPIを使用することで、メンテナンス性と将来の互換性を確保できます。

**重要**: v2 APIマイグレーションは完了しています。このガイドは主に歴史的記録と将来の類似作業のための参考資料として保持されています。