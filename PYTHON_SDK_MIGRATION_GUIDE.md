# Python SDK v1 to v2 Migration Guide

このドキュメントは、OMMX Python SDKをProtocol Bufferベース（v1）からRust-PyO3ベース（v2）にマイグレーションするための包括的なガイドです。

## 概要

OMMX Python SDKのPhase 4完了により、コアSDKは新しいRust-PyO3実装に移行されました。この変更により、Protocol Bufferに依存するアダプター（solver adapters）も新しいAPIに更新する必要があります。

## マイグレーション対象

### 完了済み
- ✅ **Core Python SDK** (`python/ommx/`) - Phase 4で完了
- ✅ **OMMX OpenJij Adapter** 
- ✅ **OMMX PySCIPOpt Adapter**
- ✅ **OMMX HiGHS Adapter**

### 進行中
- 🔄 **OMMX Python-MIP Adapter** - 部分的に完了

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
# ファクトリーメソッド
Constraint.equal_to_zero(id=id, function=function, name=name)
Constraint.less_than_or_equal_to_zero(id=id, function=function, name=name)
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

## Python-MIP Adapter 詳細移行計画

### 現在の状況 (2024年12月)

#### ✅ 完了済みファイル
- `python_mip_to_ommx.py` - 完全に新API対応
- `test_model_to_instance.py` - 完全に新API対応

#### 🔄 進行中の問題
主なエラーパターン：
```
AttributeError: 'int' object has no attribute 'kind'
AttributeError: 'builtins.Function' object has no attribute 'HasField'
RuntimeError: Undefined variable ID is used: VariableID(1)
```

### _ommx_rust 修正必要性の評価

**結論: _ommx_rustレベルでの修正は不要**

現在までの調査により、すべての問題はPython SDKレベルで解決可能であることが判明：

| 問題カテゴリ | 解決方法 | _ommx_rust修正 |
|-------------|----------|---------------|
| コードパターンエラー | ループ・アクセス方法修正 | ❌ 不要 |
| インポートエラー | パス変更 | ❌ 不要 |
| API仕様理解不足 | 適切な比較方法選択 | ❌ 不要 |
| 機能不足 | 既存API活用 | ❌ 不要 |

**具体例**:
- ✅ `Function.as_linear()` - 既に実装済み
- ✅ Enum定数 - 利用可能
- ✅ DecisionVariable構造 - 正常動作
- ✅ Instance API - 両方式対応

### 段階的修正計画

#### Phase 1: adapter.py の修正 (高優先度)

**ファイル**: `python/ommx-python-mip-adapter/ommx_python_mip_adapter/adapter.py`

**具体的なタスク**:

1. **インポート修正**:
   ```python
   # 現在の問題があるインポート
   from ommx.v1 import Instance, DecisionVariable
   
   # 修正後
   from ommx.v1 import Instance, DecisionVariable, Constraint
   from ommx._ommx_rust import Sense, Equality
   ```

2. **DecisionVariable.kind 問題の修正** (Line 312付近):
   ```python
   # 旧コード (エラーの原因)
   if var.kind == DecisionVariable.BINARY:
   
   # 新コード
   if var.kind == DecisionVariable.Kind.Binary:
   ```

3. **Instance enum 定数の修正**:
   ```python
   # 旧コード
   sense=Instance.MAXIMIZE
   sense=Instance.MINIMIZE
   
   # 新コード  
   sense=Sense.Maximize
   sense=Sense.Minimize
   ```

4. **Protocol Buffer API呼び出しの除去**:
   ```python
   # 旧コード
   if function.HasField("linear"):
   
   # 新コード
   linear_func = function.as_linear()
   if linear_func is not None:
   ```

#### Phase 2: テストファイルの修正 (中優先度)

**ファイル**: 
- `tests/test_adapter.py`
- `tests/test_integration.py`
- `tests/test_constant_constraint.py`

**共通修正パターン**:
1. インポートの統一
2. `.raw`アクセスの除去
3. Enum定数の更新
4. DecisionVariable属性アクセスの修正

#### Phase 3: Doctest の修正 (低優先度)

**ファイル**: `adapter.py` 内のdocstring

**修正対象**:
- 古いAPI使用例の更新
- インポート文の修正
- 期待値の調整

### 作業で得た知見の蓄積

#### 知見 1: DecisionVariable データ構造の変化
**発見**: 新しいAPIでは`DecisionVariable`は単なるintでなくラッパーオブジェクト
**影響**: `.kind`属性アクセスパターンが変更
**解決策**: `DecisionVariable.Kind.Binary`形式の新しいenum使用

#### 知見 2: Function 検査の新パラダイム
**発見**: `.HasField("linear")`の代替として`.as_linear()`が利用可能
**利点**: 
- より直感的なAPI
- 型安全性の向上
- パフォーマンス向上（Rust実装）

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

#### 知見 3: Import階層の整理
**発見**: Rust-based型は`_ommx_rust`から直接インポート必要
**理由**: Protocol Buffer生成コードと区別するため
**パターン**:
```python
# Core wrappers
from ommx.v1 import Instance, DecisionVariable, Constraint

# Rust native types  
from ommx._ommx_rust import Function, Linear, Sense, Equality
```

#### 知見 4: エラーパターンと診断方法
**パターン A**: `'int' object has no attribute 'kind'`
- **原因**: `for var in instance.raw.decision_variables:`でキー（int）を取得している
- **根本原因**: `.decision_variables`は辞書 `{id: DecisionVariable}`
- **修正**: `for var_id, var in instance.raw.decision_variables.items():`

**パターン B**: `'builtins.Function' object has no attribute 'HasField'`
- **原因**: 新しいFunctionクラスにProtocol Bufferメソッドなし
- **診断**: インポート元を確認（ommx.v1 vs _ommx_rust）
- **解決**: `.as_linear()`等の新メソッド使用

#### 知見 5: API構造の変化理解
**発見**: Instance.decision_variablesの戻り値型が変化
- **旧API (.raw)**: `dict[int, DecisionVariable]` - kind は整数定数
- **新API**: `DataFrame` - kind は文字列 ('binary', 'integer', 'continuous')
- **混在使用**: 両方式が利用可能、適切な比較方法を選択

**実装例**:
```python
# 旧API継続使用
for var_id, var in instance.raw.decision_variables.items():
    if var.kind == DecisionVariable.BINARY:  # 整数比較

# 新API移行
for _, row in instance.decision_variables.iterrows():
    if row['kind'] == 'binary':  # 文字列比較
```

#### 知見 6: DecisionVariable.Kind 存在せず
**発見**: `DecisionVariable.Kind.Binary` などのenumは存在しない
- **正しい形式**: `DecisionVariable.BINARY`, `DecisionVariable.INTEGER`, `DecisionVariable.CONTINUOUS`
- **整数値比較**: 新旧APIとも整数定数での比較を継続使用
- **エラー例**: `AttributeError: type object 'int' has no attribute 'Binary'`

**修正パターン**:
```python
# 間違った記述
if var.kind == DecisionVariable.Kind.Binary:  # エラー

# 正しい記述
if var.kind == DecisionVariable.BINARY:  # 正常動作
```

#### 知見 7: Constraint ファクトリーメソッド不存在
**発見**: `Constraint.equal_to_zero()` などのファクトリーメソッドは存在しない
- **直接作成**: `ommx._ommx_rust.Constraint()` から作成後 `Constraint.from_raw()` で変換
- **従来の定数**: `Constraint.EQUAL_TO_ZERO` などの定数は使用可能
- **Rust enum**: `Equality.EqualToZero` などの新しいenum値

**実装パターン**:
```python
# Rust constraint作成後にPython wrapperで包む
import ommx._ommx_rust
raw_constraint = ommx._ommx_rust.Constraint(
    id=id,
    function=function,
    equality=Equality.EqualToZero,
)
if name:
    raw_constraint.set_name(name)
constraint = Constraint.from_raw(raw_constraint)
```

#### 知見 8: イテレーション対象の修正必要性
**発見**: dictイテレーションにおけるキーと値の取得パターン変更
- **decision_variables**: `for var_id, var in instance.raw.decision_variables.items()`
- **constraints**: `for constraint_id, constraint in instance.raw.constraints.items()`
- **エラー原因**: `for var in instance.raw.decision_variables:` はキー（int）のみ取得

**修正例**:
```python
# 間違ったパターン（キーのみ取得）
for var in instance.raw.decision_variables:
    print(var.kind)  # エラー: 'int' object has no attribute 'kind'

# 正しいパターン（キーと値の両方取得）
for var_id, var in instance.raw.decision_variables.items():
    print(var.kind)  # 正常動作
```

#### 知見 9: テストでのDataFrameアクセス問題
**発見**: `instance.decision_variables[0]` のようなインデックスアクセスは不可
- **DataFrame形式**: 新APIではpandas DataFrameとして返される
- **辞書アクセス**: `.raw.decision_variables[id]` で直接アクセス可能
- **エラー例**: `KeyError: 0` - DataFrameの列名として0が存在しない

**対処方法**:
```python
# 間違ったアクセス
decision_var = instance.decision_variables[0]  # KeyError

# 正しいアクセス方法
decision_var = instance.raw.decision_variables[0]  # dict access
```

#### 知見 10: Function APIアクセス方法
**発見**: `instance.objective.as_linear()` は不可、`.raw` 経由でアクセス必要
- **新しいFunction API**: `instance.raw.objective.as_linear()` で Rust実装にアクセス
- **度数・項数アクセス**: `instance.raw.objective.degree()`, `instance.raw.objective.num_terms()`
- **constraints**: `instance.raw.constraints[id].function.as_linear()`

**修正パターン**:
```python
# 間違ったアクセス
linear_func = instance.objective.as_linear()  # AttributeError

# 正しいアクセス方法
linear_func = instance.raw.objective.as_linear()  # 正常動作
degree = instance.raw.objective.degree()
constraint_func = instance.raw.constraints[0].function.as_linear()
```

#### 知見 11: 変数ID一致の重要性
**発見**: Function内で使用する変数IDは決定変数リストと厳密に一致する必要
- **エラー例**: `RuntimeError: Undefined variable ID is used: VariableID(1)`
- **解決**: Quadratic/Function作成時の変数IDを決定変数IDと一致させる

**修正例**:
```python
# 間違った変数ID
decision_var = DecisionVariable.continuous(0)
quadratic = Quadratic(columns=[1], rows=[1], values=[2.3])  # IDが1、決定変数は0

# 正しい変数ID
decision_var = DecisionVariable.continuous(0)
quadratic = Quadratic(columns=[0], rows=[0], values=[2.3])  # IDが0で一致
```

### 検証とテスト戦略

#### 段階的検証
1. **構文チェック**: `python -m py_compile` でファイル単位確認
2. **インポートテスト**: `python -c "import ommx_python_mip_adapter"`
3. **単体テスト**: 個別テストファイル実行
4. **統合テスト**: 全体テストスイート実行

#### 回帰テスト
```bash
# 修正前後の動作比較
task python:ommx-python-mip-adapter:test > before.log 2>&1
# 修正作業
task python:ommx-python-mip-adapter:test > after.log 2>&1
diff before.log after.log
```

### リスク管理

#### 高リスク要素
1. **API互換性破壊**: 既存のユーザーコードへの影響
2. **性能回帰**: 新API使用時の予期しない性能低下
3. **機能欠落**: 旧APIでサポートされていた機能の不備

#### 軽減策
1. **段階的ロールアウト**: ファイル単位での逐次修正
2. **詳細テスト**: 各修正後の動作確認
3. **ドキュメント**: 変更点の詳細記録

### 完了判定基準

#### 必須要件
- [ ] 全テストがパス (`task python:ommx-python-mip-adapter:test`)
- [ ] importエラーなし
- [ ] 既存機能の動作保証

#### 推奨要件
- [ ] doctestの更新
- [ ] パフォーマンス測定
- [ ] ユーザー向けマイグレーションガイド更新

### 実装時間見積もり

| タスク | 見積もり時間 | 依存関係 |
|--------|-------------|----------|
| adapter.py修正 | 2-3時間 | なし |
| test_adapter.py修正 | 1-2時間 | adapter.py完了後 |
| test_integration.py修正 | 1-2時間 | adapter.py完了後 |
| doctest修正 | 30分-1時間 | 機能修正完了後 |
| 検証・文書化 | 1時間 | 全修正完了後 |
| **合計** | **5.5-9時間** | |

## 次のステップ

1. **Python-MIP Adapter完了**: 上記計画に従った修正実行
2. **他のアダプター**: 必要に応じて同様のマイグレーション
3. **ドキュメント更新**: APIリファレンスとチュートリアルの更新
4. **リリースノート**: v2移行の変更点をまとめ

## 追加リソース

- [CLAUDE.md](./CLAUDE.md) - プロジェクト開発ガイダンス
- [Phase 4 完了状況](./CLAUDE.md#migration-progress) - コアSDK移行詳細
- [Python SDK API リファレンス](./docs/api_reference/) - 新しいAPI仕様

---

このガイドは、OMMX Python SDKエコシステム全体を新しいRust-PyO3ベースのv2に移行するための包括的なリソースです。具体的な実装例や追加の技術詳細については、完了済みのアダプターコードを参考にしてください。