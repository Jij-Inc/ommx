# Python SDK v1 to v2 Migration Guide

このドキュメントは、OMMX Python SDKをProtocol Bufferベース（v1）からRust-PyO3ベース（v2）にマイグレーションするための包括的なガイドです。

## 概要

OMMX Python SDKのPhase 4完了により、コアSDKは新しいRust-PyO3実装に移行されました。この変更により、Protocol Bufferに依存するアダプター（solver adapters）も新しいAPIに更新する必要があります。

## 対象範囲

このガイドは、Protocol Bufferベース（v1）からRust-PyO3ベース（v2）へのアダプター移行に適用されます。

## 主要な変更点

### 1. インポートの変更

**旧方式 (v1)**:
```python
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1 import Instance, DecisionVariable
```

**新方式 (v2)**:
```python
from ommx.v1 import Instance, DecisionVariable, Constraint
from ommx._ommx_rust import Function, Linear, Sense, Equality
```

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
# Rustベースファクトリーメソッド
Function.from_scalar(constant)
linear = Linear(terms=terms, constant=constant)
Function.from_linear(linear)
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
# 直接コンストラクタで作成（_ommx_rust.Function と _ommx_rust.Equality を受け取り可能）
constraint = Constraint(
    id=id,
    function=function,  # _ommx_rust.Function を直接使用可能
    equality=Equality.EqualToZero,  # _ommx_rust.Equality を直接使用可能
    name=name,
)
```

### 5. Enum 定数

**旧方式**:
```python
Instance.MAXIMIZE
Instance.MINIMIZE
DecisionVariable.BINARY
Constraint.EQUAL_TO_ZERO
```

**新方式**:
```python
Sense.Maximize
Sense.Minimize  
DecisionVariable.Kind.Binary
Equality.EqualToZero
```

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
# Rust as_linear メソッド
linear_func = function.as_linear()
if linear_func is not None:
    linear_terms = linear_func.linear_terms()  # dict[int, float]
    constant = linear_func.constant_term()     # float
```

### 7. 属性アクセス

**旧方式**:
```python
# Protocol Buffer フィールドアクセス
ommx_instance = model_to_instance(model).raw
decision_var.kind == DecisionVariable.CONTINUOUS
constraint.equality == Constraint.EQUAL_TO_ZERO
```

**新方式**:
```python
# Rust wrapper プロパティアクセス
ommx_instance = model_to_instance(model)  # .raw不要
decision_var.kind == DecisionVariable.Kind.Continuous
constraint.equality == Equality.EqualToZero
```

## 新しく利用可能なメソッド

### Function クラス
```python
# 型変換・検査
function.as_linear() -> Optional[Linear]
function.as_quadratic() -> Optional[Quadratic]

# 情報取得
function.degree() -> int      # 関数の次数
function.num_terms() -> int   # 項数
```

## マイグレーション手順

### ステップ 1: インポートの更新
1. Protocol Buffer直接インポートを削除
2. `ommx.v1`と`ommx._ommx_rust`からの適切なインポートに変更

### ステップ 2: ファクトリーメソッドの更新
1. `DecisionVariable.of_type()`を型別メソッドに変更
2. `Function`と`Constraint`の直接作成をファクトリーメソッドに変更

### ステップ 3: Enum定数の更新
1. 古いクラス定数を新しいEnum値に変更
2. `Sense`と`Equality`の適切なインポート

### ステップ 4: Protocol Buffer API除去
1. `.HasField()`呼び出しを`.as_linear()`等に変更
2. `.raw`属性アクセスを直接アクセスに変更
3. フィールド直接アクセスをメソッド呼び出しに変更

### ステップ 5: テストの更新
1. テストの期待値を新しいAPI仕様に合わせて更新
2. 属性アクセスパターンの変更

## 一般的な問題と解決策

### 問題 1: `'int' object has no attribute 'kind'`
**原因**: DecisionVariableがラッパーでなく生IDを返している
**解決**: インスタンス作成方法とアクセス方法を確認

### 問題 2: `AttributeError: 'builtins.Function' object has no attribute 'HasField'`
**原因**: 新しいFunctionクラスにProtocol Bufferメソッドがない
**解決**: `.as_linear()`メソッドを使用

### 問題 3: `ImportError: cannot import name 'Sense' from 'ommx.v1'`
**原因**: EnumがRustから直接エクスポートされている
**解決**: `ommx._ommx_rust`からインポート

### 問題 4: `AttributeError: type object 'Function' has no attribute 'from_scalar'`
**原因**: Python Functionクラスでなく_ommx_rust.Functionを使う必要
**解決**: 正しいインポートパスを使用

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

## 技術的知見集

このセクションでは、マイグレーション作業中に発見された重要な技術的知見をまとめています。

### 1. DecisionVariable データ構造の変化
**発見**: 新しいAPIでは`DecisionVariable`は単なるintでなくラッパーオブジェクト  
**影響**: `.kind`属性アクセスパターンが変更  
**解決策**: 整数定数での比較を継続使用

**修正パターン**:
```python
# 間違った記述
if var.kind == DecisionVariable.Kind.Binary:  # エラー

# 正しい記述  
if var.kind == DecisionVariable.BINARY:  # 正常動作
```

### 2. Function 検査の新パラダイム
**発見**: `.HasField("linear")`の代替として`.as_linear()`が利用可能  
**利点**: より直感的なAPI、型安全性の向上、パフォーマンス向上（Rust実装）

**実装例**:
```python
# 旧方式
if obj.HasField("linear"):
    terms = obj.linear.terms
    
# 新方式 
linear_obj = obj.as_linear()
if linear_obj is not None:
    terms = linear_obj.linear_terms()
```

### 3. Import階層の整理
**発見**: Rust-based型は`_ommx_rust`から直接インポート必要  
**理由**: Protocol Buffer生成コードと区別するため

**パターン**:
```python
# Core wrappers
from ommx.v1 import Instance, DecisionVariable, Constraint

# Rust native types  
from ommx._ommx_rust import Function, Linear, Sense, Equality
```

### 4. エラーパターンと診断方法

**パターン A**: `'int' object has no attribute 'kind'`
- **原因**: `for var in instance.raw.decision_variables:`でキー（int）を取得している
- **修正**: `for var_id, var in instance.raw.decision_variables.items():`

**パターン B**: `'builtins.Function' object has no attribute 'HasField'`
- **原因**: 新しいFunctionクラスにProtocol Bufferメソッドなし
- **解決**: `.as_linear()`等の新メソッド使用

### 5. API構造の変化理解
**発見**: Instance.decision_variablesの戻り値型が変化
- **旧API (.raw)**: `dict[int, DecisionVariable]` - kind は整数定数
- **新API**: `DataFrame` - kind は文字列 ('binary', 'integer', 'continuous')

### 6. Constraint 作成パターンの改善
**発見**: `Constraint()` コンストラクタが `_ommx_rust.Function` と `_ommx_rust.Equality` を直接受け取り可能
- **利点**: `from_raw()` による変換が不要、よりシンプルなAPI
- **型対応**: Protocol Buffer値とRust enum値の両方をサポート

**実装パターン**:
```python
# シンプルなコンストラクタパターン
constraint = Constraint(
    id=id,
    function=function,  # _ommx_rust.Function を直接使用
    equality=Equality.EqualToZero,  # _ommx_rust.Equality を直接使用
    name=name,
)
```

### 7. Function APIアクセス方法
**発見**: `instance.objective.as_linear()` は不可、`.raw` 経由でアクセス必要

**修正パターン**:
```python
# 間違ったアクセス
linear_func = instance.objective.as_linear()  # AttributeError

# 正しいアクセス方法
linear_func = instance.raw.objective.as_linear()  # 正常動作
```

### 8. 変数ID一致の重要性
**発見**: Function内で使用する変数IDは決定変数リストと厳密に一致する必要
- **エラー例**: `RuntimeError: Undefined variable ID is used: VariableID(1)`

### 9. Pyright型エラー修正とAPI改善
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

この改善により、他のアダプターでも`_ommx_rust.Function`を直接使用可能になりました。

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

---

このガイドは実際のマイグレーション作業から得られた知見に基づいており、他のアダプターでも同様の問題を効率的に解決するために活用できます。