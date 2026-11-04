(python-sdk-v2-to-v3-migration-guide)=
# Python SDK v2 から v3 へのマイグレーションガイド

移行元は [Python SDK 2.9.4](https://pypi.org/project/ommx/2.9.4/)、移行先はこのドキュメントと同じ `main` です。[導入手順](../getting_started/install.md)に従い、既存のv2環境と移行先を分けてください。代表的なモデルと候補を手元に残しておくと、移行後も同じ目的値と実行可能性が得られるかを比較できます。

## 自分のコードで確認する箇所

- **モデルを作るコード**では、外部の制約番号を引き継ぐなら[番号を指定する移行例](#migration-compare-model)、プログラム内で番号を振っていただけなら[モデルに採番を任せる例](#migration-automatic-ids)を参照してください。
- **結果を集計するコード**では、符号付きの制約式の値と非負の違反量を区別します。DataFrameはメソッドとして呼び、制約の種類を指定します。詳しくは{class}`~ommx.Solution`と{class}`~ommx.SampleSet`を参照してください。
- **ソルバーを呼ぶコード**では、推奨の準備をAdapterに任せるか、自分で変換を指定するかを決めます。[Adapter一覧](../api/adapters)から使用中のAdapterへ進めます。
- **保存済みデータを読むコード**では、v2.9.4の `Instance.to_bytes()` が出力したバイト列を `Instance.from_v1_bytes(data)` で読みます。`.ommx` アーカイブの読込みには[Artifactの移行手順](#13-artifact-api-archive-becomes-an-exchange-format)を使います。

### 2.9.4ですでに使える機能

`Function`、`Constraint`、`DecisionVariable`、`Instance`、`ParametricInstance`、`Solution`、`SampleSet` は2.9.4でも `ommx` から直接importできます。先にこれらのimportを切り替え、制約や結果の扱いはv3環境で更新できます。SCIPに変数名と添字で初期候補を渡す `initial_state` も引き続き使えます。

2.9.4はモデルと結果のバイト列に注釈も保存します。v3では `annotations` に直接書き込んでいた箇所を `add_user_annotation` や `title` などに置き換えてください。2.9.4とモデルを交換し続ける場合は、受信側が扱える表現も選びます。たとえば2.9.4のprotobufには有限集合を値域に持つ変数のフィールドがありますが、2.xのSDKはその変数の読込みを拒否します。v3での値域の指定は{meth}`~ommx.DecisionVariable.finite_domain`を参照してください。

## 1. import の変更

### 1.1 protobuf submodule は削除

`ommx.v1.*_pb2` module と `ommx.v1.annotation` は削除されました。SDK class は top-level `ommx` から import します。

```python
# v2.9.4
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1.solution_pb2 import State

# v3
from ommx import Constraint, Equality, Function, Linear, Sense, State
```

`.from_protobuf()` / `.to_protobuf()` は、protobuf object と一緒に削除されています。`Instance` / `Solution` / `SampleSet` など全体をシリアライズする場合は、protobuf version を名前に含む bytes API を使います。新しく byte 列を作る場合は `to_v2_bytes()` / `from_v2_bytes(...)` を使い、legacy な v1 payload との互換が必要な場合だけ `to_v1_bytes()` / `from_v1_bytes(...)` を使ってください。

### 1.2 constraint hint helper は first-class constraint type へ

`ConstraintHints`、`OneHot`、`Sos1`、`Parameters` wrapper は export されなくなりました。

legacy v1 の `ConstraintHints` は、v3 で読み込む際も advisory metadata として扱われます。`Instance.from_v1_bytes(...)` と `ParametricInstance.from_v1_bytes(...)` は hint を無視して通常制約を保持し、first-class 特殊制約へ自動昇格しません。特殊制約として扱う必要がある場合は、無視された hint だけを根拠にせず、信頼できる modeling input から対応する first-class constraint を構築してください。

```python
# v2.9.4
from ommx.v1 import OneHot, Sos1, ConstraintHints, Parameters

# v3
from ommx import OneHotConstraint, Sos1Constraint, IndicatorConstraint

# Parameters wrapper の代わりに plain dict を渡します
parametric_instance.with_parameters({parameter_id: 1.0})
```

## 2. `.raw` と bridge method の削除

v3 の各型は直接 Rust 実装を持つため、別の underlying object はありません。`.raw`、`from_raw`、`from_protobuf`、`to_protobuf` を使っていた箇所は、公開 property / method に置き換えます。

```python
# v2.9.4
linear.raw.linear_terms
instance.raw.sense
solution.raw.optimality = Optimality.Optimal
Constraint.from_protobuf(pb_constraint)
dv.to_protobuf()

# v3
linear.linear_terms
instance.sense
solution.optimality = Optimality.Optimal
instance.to_v2_bytes()
```

`Constraint` や `Function` などの要素単体は、ID や host context を持てないため、原則として単体で bytes round-trip しません。`Instance` / `Solution` / `SampleSet` のような所有者単位でシリアライズしてください。

## 3. 制約IDは `Constraint` ではなく host 側が所有

### 3.1 `id` / `set_id()` / `id=` は削除

v2では `Constraint(id=7, ...)` や `(x <= 1).set_id(7)` で通常制約に番号を付けました。v3では作成直後の制約は番号を持たず、`Instance.from_components` に渡す辞書のキーで番号を指定します。既存のモデルに追加するときは §3.3 の方法でモデルに採番を任せられます。

OneHotやSOS1でも、制約番号は種類別の辞書のキーで指定します。制約の `variables` は変数ID・`DecisionVariable`・`AttachedDecisionVariable` を受け取ります。参照する変数はモデルの `decision_variables` に含めてください。

(migration-compare-model)=
### 3.2 同じモデルと候補を両方のSDKで比較する

バイナリ変数に上限を課し、候補 $x=0$ を評価します。

**v2.9.4**

```python
from ommx import DecisionVariable, Instance

x = DecisionVariable.binary(0)
limit = (x <= 1).set_id(7).add_name("limit")
model = Instance.from_components(
    decision_variables=[x], objective=x, constraints=[limit], sense=Instance.MAXIMIZE,
)
result = model.evaluate({0: 0})
assert result.extract_constraints("limit")[()] == -1.0
```

**v3 (`main`)**

```python
from ommx import ConstraintID, DecisionVariable, Instance, Sense

x = DecisionVariable.binary(0)
model = Instance.from_components(
    decision_variables=[x], objective=x, constraints={7: x <= 1}, sense=Sense.Maximize,
)
model.constraints[ConstraintID(7)].set_name("limit")
result = model.evaluate({0: 0})
assert result.extract_constraints("limit")[()] == 0.0
assert result.get_constraint_value(ConstraintID(7)) == -1.0
```

両方とも制約番号は7です。v2では式に番号を付け、v3ではモデル構築時に辞書のキーで渡します。候補は同じ上限を満たしています。v2で得た $x-1=-1$ は符号付きの値、v3で得た $0$ は違反量です。式の値を使う集計には {meth}`~ommx.Solution.get_constraint_value` を選んでください。

(migration-automatic-ids)=
### 3.3 変数と制約のIDをモデルに割り当てさせる

v2で変数の作成時に連番を渡したり、制約のグローバルカウンタをリセットしたりしていた場合は、v3ではモデルを先に作り、変数と制約を順に追加できます。`Instance.minimize()` または `Instance.maximize()` で始めます。次の例では、追加時の戻り値から変数IDと制約IDを取得し、そのまま候補の評価に使います。

```python
from ommx import Instance

model = Instance.maximize()
x = model.new_binary("x")
y = model.new_binary("y")
model.objective = x + y
limit = model.add_constraint(x + y <= 1, "limit")

result = model.evaluate({x.id: 1, y.id: 1})
assert result.constraint_violation(limit.constraint_id) == 1.0
```

`Instance` には `new_binary` のほかに `new_integer`、`new_continuous`、`new_semi_integer`、`new_semi_continuous`、`new_finite_domain` があり、いずれも変数IDを自動で割り当てます。すでに番号のある変数を追加する {meth}`~ommx.Instance.add_decision_variable` は、その番号を使います。

`Instance` と `ParametricInstance` では、通常制約の `add_constraint` と特殊制約の `add_indicator_constraint`、`add_one_hot_constraint`、`add_sos1_constraint` がモデル内で未使用の番号を割り当てます。v2の `next_constraint_id()` や `set_constraint_id_counter()` は不要です。既存データが特定の制約番号を参照しているときは、前節の {meth}`~ommx.Instance.from_components` でその番号を明示してください。

## 4. container type の変更

一つのモデル内では、削除済みを含めて全種類の制約が同じ番号空間を使います。種類別に番号を振り直すコードは、モデル全体で一意に採番してください。コンストラクタとprotobuf読込みは種類間の重複をエラーにします。

### 4.1 `Instance.from_components(constraints=...)` は `dict[int, Constraint]`

制約系の引数はすべて ID を key にした `dict` です。`decision_variables` は `Sequence[DecisionVariable]` のままです。

```python
# v2.9.4
Instance.from_components(
    sense=Instance.MINIMIZE,
    objective=obj,
    decision_variables=[x0, x1],
    constraints=[c0, c1],
    constraint_hints=ConstraintHints(),
)

# v3
Instance.from_components(
    sense=Sense.Minimize,
    objective=obj,
    decision_variables=[x0, x1],
    constraints={0: c0, 1: c1},
    indicator_constraints={10: ic},
    one_hot_constraints={20: oh},
    sos1_constraints={30: sc},
)
```

`ParametricInstance.from_components` も同じ shape を取ります。

(42-constraint-accessors-on-instance--parametricinstance--solution-return-dicts)=
### 4.2 制約の参照を型付きIDのコレクションへ統一

```python
# v2.9.4
for c in instance.constraints:
    print(c.id, c.function)

# v3
from ommx import ConstraintID

for cid in instance.constraints:
    if isinstance(cid, ConstraintID):
        print(cid, instance.constraints[cid].function)
```

`Instance` / `ParametricInstance` の `.constraints` は、型付きIDで参照すると `AttachedX` handle を返します。`Solution.constraints` と `SampleSet.constraints` も全種類を含む専用コレクションで、各アクセス時に評価結果のsnapshotを返します。`SampleSet.decision_variables` と `.named_functions` は `list` のままです。

## 5. rename、signature、挙動の変更

主な rename / signature 変更は次の通りです。

| v2.9.4 | v3 |
|---|---|
| `instance.write_mps(path)` | `instance.save_mps(path)` |
| `instance.used_decision_variable_ids()` | `instance.required_ids()` |
| `func.used_decision_variable_ids()` | `func.required_ids()` |
| `Parameter.new(id=..., ...)` | `Parameter(id, ...)` |
| `Parameters(entries={...})` | plain `dict[int, float]` |
| `Linear(terms=[Linear.Term(...)])` | `Linear(terms={id: coeff})` |

```python
# v2.9.4
instance.write_mps("out.mps.gz")
p = Parameter.new(id=3, name="w")
pi.with_parameters(Parameters(entries={p.id: 1.0}))

# v3
instance.save_mps("out.mps.gz")
p = Parameter(3, name="w")
pi.with_parameters({p.id: 1.0})
```

### 5.6 `to_qubo()` / `to_hubo()`は入力objectiveを評価結果に保持 (`3.0.0`, [#1167](https://github.com/Jij-Inc/ommx/pull/1167))

Driver methodは引き続き利用でき、入力をin-placeに変更します。v3では変更後の
`Instance`がQUBO/HUBO solverへ渡すminimization energyをactive objectiveとして保持し、
`evaluate()`と`evaluate_samples()`は変換前のinstanceが公開していたobjective semanticsを
保持します。Python SDK v2はactive senseを戻したうえで最終的なpenalty energyを評価しており、
solver inputとuser-facing outputを混在させていました。

したがって`Instance.objective`はsolver energyを、`Solution`と`SampleSet`は保持された入力
objectiveを表します。実行可能な事後条件は{meth}`~ommx.Instance.to_qubo`、
{meth}`~ommx.Instance.to_hubo`、{meth}`~ommx.Instance.evaluate`、
{meth}`~ommx.Instance.evaluate_samples`に記載されています。

明示的なoutput objectiveを持つ`Instance`または`ParametricInstance`は、v1 wire formatで
losslessに表現できません。この場合`to_v1_bytes()`は`RuntimeError`を送出するため、
`to_v2_bytes()`を使用してください。

同じpipelineは{meth}`~ommx.Instance.prepare`と
{meth}`~ommx.Instance.as_qubo_format`または
{meth}`~ommx.Instance.as_hubo_format`で明示的に実行できます。対応するtarget classと
編集可能なpolicyは{meth}`~ommx.InstanceClass.qubo`、
{meth}`~ommx.InstanceClass.hubo`、{meth}`~ommx.PreparationPolicy.for_qubo`、
{meth}`~ommx.PreparationPolicy.for_hubo`が提供します。

### 5.7 Adapterの`solve()` / `sample()`が入力を自動的にPrepare (`3.0.0`, [#1166](https://github.com/Jij-Inc/ommx/pull/1166))

通常は元のInstanceをそのまま`solve()`または`sample()`へ渡します。AdapterはそのInstanceを
変更せずに推奨Preparationを適用します。

```python
from ommx_highs_adapter import OMMXHighsAdapter

solution = OMMXHighsAdapter.solve(instance)
```

application固有のPreparation Policyが必要な場合は、Instanceをin-placeでPrepareしてから
preparation-free APIを使います。

```python
policy = OMMXHighsAdapter.recommended_preparation_policy()
# 必要に応じてpolicyを調整する。
instance.prepare(OMMXHighsAdapter.INPUT_CLASS, policy)
solution = OMMXHighsAdapter.solve_without_preparation(instance)
```

独自Adapterでは、`solve_without_preparation()`または`sample_without_preparation()`を実装します。
追加optionは、それを公開する各APIで明示的な型付きsignatureとして宣言し、包括的な
`**kwargs`は使いません。Preparationをまたいでも
意味が変わらないoptionはeasy methodから転送できます。exactなprepared inputに
意味が依存し、Preparation過程をまたぐtransportを定義しない場合、具体的なAdapterはそのoptionを
preparation-free methodだけに公開できます。Adapter inputに
`output_objective`がある場合、HiGHSとPython-MIPはdual valueを付与しません。

(constraint-violation-migration)=
### 5.8 制約の違反量 ([#1213](https://github.com/Jij-Inc/ommx/pull/1213))

`solution.total_violation_l1()` は {meth}`~ommx.Solution.total_violation` に改名し、
`total_violation_l2()` は削除しました。各制約に定義した非負のスカラー値を足し合わせ、
removed 制約とすべての特殊制約を含めます。通常の等式・不等式の違反量の定義は変わりません。

```python
from ommx import ConstraintKind, OneHotConstraintID
solution.total_violation()
solution.constraint_violation(OneHotConstraintID(30))
solution.constraints_df(kind=ConstraintKind.Sos1)[["feasible", "violation"]]
```

Indicator は有効時の内部制約の違反量、無効時は 0 です。OneHot は一つを 1、残りを 0 にする
絶対変更量の最小値、SOS1 は非ゼロを高々一つにする絶対変更量の最小値です。
Big-M lowering に依存する定義ではありません。

すべての制約を `violation <= atol` のときに feasible と判定します。OneHot と SOS1 では、
許容誤差を各メンバーに個別に適用する方式から、絶対変更量の合計に適用する方式へ変わります。
例えば SOS1 の値 `(1, 0.00006, 0.00006)` は違反量が `0.00012` なので、
`atol=0.0001` では infeasible です。各小成分が個別にゼロの許容誤差内であることでは
判定しません。

変数の bounds・kind の判定は別です。離散値の丸めは制約評価より前に行われ、違反量には
その後の値を使います。解の制約充足は各制約がそれぞれの閾値を満たすことで判定し、
`total_violation <= atol` で判定するわけではありません。

SampleSet の feasibility と最良実行可能解の選択も、保存された許容誤差で変数の
bounds・kind を確認し、取り出した Solution の判定と一致します。そのため、
`total_violation()` がゼロでも変数が定義域を外れていれば infeasible になります。

制約単体の判定では、許容誤差を明示的に渡します。`evaluated.feasible` は
`evaluated.is_feasible(atol=...)` に、`sampled.feasible` は
`sampled.feasible(atol=...)`（sample ID から bool への map）に置き換えてください。
Solution/SampleSet の feasibility プロパティは引き続き利用できます。
`feasibility_atol` プロパティから、取り出した制約にも同じ判定条件を渡せます。

```python
from ommx import ConstraintID
evaluated = solution.constraints[ConstraintID(1)]
evaluated.is_feasible(atol=solution.feasibility_atol)
evaluated.is_feasible(atol=1e-8)  # 保存済みの同じ違反量に対する別の問い合わせ
```

問い合わせの許容誤差を変えても、入力の丸めや活性判定はやり直しません。
許容誤差に依存する判断結果を保持する場合に、その判断条件も保持します。
通常制約の評価値は許容誤差を保持せず、特殊制約の活性判定には使用した条件を残します。
許容誤差の役割分離と評価コンテキストの保存形式は
[Issue #1180](https://github.com/Jij-Inc/ommx/issues/1180) で扱います。

v2 protobuf 形式は `feasible` のフラグ・map と、Solution/SampleSet の
`feasibility_atol` を引き続き保存します。OMMX SDK がなくても判定結果とその許容誤差を
読み取れます。protobuf スキーマは変更しません。SDK は読み込み時に、保存された変数値から
OneHot/SOS1 の違反量を再計算し、保存された許容誤差を使ってフラグの整合性を検証します。

旧 v1 形式には許容誤差とネイティブの特殊制約を保存できません。
`to_v1_bytes()` は、保存される通常制約と変数値から SDK の既定の許容誤差で
feasibility を再計算します。`from_v1_bytes()` も同じ条件を使うため、v1 の書き出しと
読み込みには同じ既定の許容誤差を使用してください。この変換で feasibility が変わる
場合があります。元の許容誤差と特殊制約を保存するには v2 を使用してください。

部分評価では、active に残る特殊制約から近似ゼロのメンバーを取り除くことで、誤差の合計に
基づく feasibility が変わりうる場合、その固定を拒否します。
正確にゼロのメンバーの除去は引き続き可能です。
この場合は完全な状態を評価するか、特殊制約を lowering してから部分評価してください。
lowering 後の違反量は、以下のとおり別の尺度になります。

lowering 前後で総和の一致は保証しません。元の removed 制約と
生成した制約が残っていれば、それぞれを集計します。数式は
{meth}`~ommx.Solution.total_violation` を参照してください。

## 6. return type の変更

通常制約の `name` と `description` は、2.9.4でもv3でも未設定なら `None` です。`constraint.name is None` で判定しているコードはそのまま使えます。

```python
name = constraint.name
if name is not None:
    print(name)
```

`Linear.terms` / `Quadratic.terms` / `Polynomial.terms` は property ではなく method です。

```python
linear.terms()
quadratic.terms()
polynomial.terms()
```

v3 alpha の初期移行では、`DecisionVariable` のコンストラクタと `kind` プロパティが
protobuf の整数コードを公開していました。現在のコンストラクタは `Kind` の値を受け取り、
detached・attached・evaluated・sampled の各変数の `.kind` も同じ型を返します。
`Kind.Binary` や `Kind.Integer` などの既知の定数と比較し、今後追加される種類を受け取った場合の処理も用意してください。
`Kind` は反復処理や `Enum.__members__` による列挙には対応しません。
`DecisionVariable.BINARY` などの既存の別名は、対応する種類を引き続き参照します。

同じ互換方針を `Constraint.EQUAL_TO_ZERO` / `LESS_THAN_OR_EQUAL_TO_ZERO` と
`Instance.MINIMIZE` / `MAXIMIZE` にも適用します。これらは型付き alias として残しますが、
新しいコードでは `Equality.*` と `Sense.*` を使います。

```python
from ommx import Bound, DecisionVariable, Kind

variable = DecisionVariable(0, Kind.Binary, Bound(0, 1))
assert variable.kind == Kind.Binary
assert DecisionVariable.BINARY == Kind.Binary
```

既知の種類は対応する protobuf の整数コードと等しく、ハッシュ値も一致します。

```python
assert Kind.Binary == 1
assert hash(Kind.Binary) == hash(1)
```

`SampleSet.sample_ids` は list property ではなく set を返す method になりました。list が必要な場合は `sample_ids_list` を使います。

```python
ids: set[int] = sample_set.sample_ids()
ids_list: list[int] = sample_set.sample_ids_list
```

`evaluate` は、必要な decision variable ID が state にない場合や atol が不正な場合に、`RuntimeError` ではなく `ValueError` を投げます。`partial_evaluate` は不足 ID を許容しますが、与えた entry が不正な場合、atol が不正な場合、または dependent variable の assertion が不整合・検証不能な場合は `ValueError` を投げます。

v2.9.4 の `Function` は常に polynomial だったため、`degree()` と
`num_terms()` は常に `int` を返していました。v3 の `Function` は絶対値、
点ごとの最小値・最大値、除算、符号付き整数べきなどの複合 scalar expression も
表現できます。この表現では、両メソッドは `None` を返します。また、polynomial の
係数 property（`terms`、`linear_terms`、`quadratic_terms`、`constant_term`）と
`content_factor()` は `TypeError` を送出します。

```python
# v2.9.4
degree: int = function.degree()
terms = function.terms

# v3
degree: int | None = function.degree()
if degree is None:
    # 係数 map として扱わず、複合式のまま評価する
    value = function.evaluate(state, atol=1e-6)
else:
    terms = function.terms
```

非負の整数べきを展開済み polynomial のまま保持したい場合は、明示的な乗算を
使ってください。`function.powi(n)` と `function**n` は、`n >= 0` の場合も意図的に
複合 power expression を構築します。

## 7. 削除された helper

次の helper は削除または置き換えられました。

- `Linear.from_object(x)` - `Linear.single_term(...)`、`Linear.constant(...)`、または arithmetic operator を使います。
- `Linear.equals_to(other)` - `linear.almost_equal(other, atol=...)` を使います。
- `instance.constraint_hints` - `one_hot_constraints` / `sos1_constraints` / `indicator_constraints` に分かれました。
- `ArtifactArchive` / `ArtifactDir` 系 - `Artifact` / `ArtifactDraft` に統合されました。
- `ommx_openjij_adapter.response_to_samples(response)` - `decode_to_samples(response)` を使用します（`3.0.0`: [#1087](https://github.com/Jij-Inc/ommx/pull/1087)）。
- `ommx_openjij_adapter.sample_qubo_sa(...)` -
  `OMMXOpenJijSAAdapter.sample(instance)`を使います。置き換え後は`instance`を変更せず、
  raw `Samples`ではなく評価済みの`SampleSet`を返します
  （`3.0.0`: [#1087](https://github.com/Jij-Inc/ommx/pull/1087)）。

v2のOpenJij Adapterでは、constructor、`sample()`、`solve()` が
`uniform_penalty_weight`、`penalty_weights`、
`inequality_integer_slack_max_range`を直接受け取っていました。v3ではこれらの選択を推奨
`PreparationPolicy`へ移します。

`INPUT_CLASS`が表す直接受け取れる範囲は{class}`~ommx.InstanceClass`、推奨Policyと呼び出し側の選択は{meth}`~ommx.Instance.prepare`を参照してください。独自のPolicyを適用する手順は{class}`明示的なPreparationと求解の例 <ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter>`にまとめています。

- `uniform_penalty_weight`と`penalty_weights`は`policy.fixed_penalty`へ移す。
- `inequality_integer_slack_max_range`は`policy.integer_slack`へ移す。

constructorを直接使う場合は自動的にPrepareされないため、あらかじめ
`OMMXOpenJijSAAdapter.INPUT_CLASS`向けにPrepareしたInstanceを渡します。

OpenJijの`initial_state`は、このexactなprepared inputのsolver変数表現に対して定義され、dictの
keyはその変数IDです。そのためv3ではeasy methodではなく、exact-input constructor、
`sample_without_preparation()`、`solve_without_preparation()`で受け取ります。

通常の`sample()` / `solve()` workflowは§5.7で説明しています。固定penaltyが必要なmodelでは、
applicationがそのmagnitudeを選ぶ必要があります。v3はv2の暗黙なuniform weight `1.0`を
使用しません。customizeしたPolicyをInstanceへ適用し、preparation-free methodを呼びます。

```python
from ommx import FixedPenaltyPreparation
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
policy.fixed_penalty = (
    FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=20.0)
)
source.prepare(OMMXOpenJijSAAdapter.INPUT_CLASS, policy)
samples = OMMXOpenJijSAAdapter.sample_without_preparation(source)
```

`prepare()`は`source`をin-placeで変更します。返される`SampleSet`のobjectiveとsenseは
Preparation前に`source`が公開していたものです（§5.6）。Policyの詳細とpenaltyの選び方は
[OpenJij tutorial](../tutorial/tsp_sampling_with_openjij_adapter)を参照してください。

## 8. DataFrame accessor

`*_df` は property ではなく method です。

```python
# v2.9.4
df = instance.constraints_df

# v3
from ommx import ConstraintKind

df = instance.constraints_df(kind=ConstraintKind.Regular)
```

kind 別や removed / active 別の DataFrame accessor は、`constraints_df(kind=..., removed=...)` に統合されています。

```python
from ommx import ConstraintKind

instance.constraints_df(kind=ConstraintKind.Regular)
instance.constraints_df(kind=ConstraintKind.OneHot)
instance.constraints_df(kind=ConstraintKind.Sos1, removed=True)
solution.constraints_df(kind=ConstraintKind.Indicator)
```

## 9. metadata と annotation

`Instance` / `Solution` / `SampleSet` は Python dataclass ではありません。metadata は dedicated property や method で扱います。

```python
instance.title = "portfolio"
instance.add_user_annotation("owner", "analytics")
instance.replace_annotations({"team": "optimization"})
```

`annotations` property は read-only projection です。直接 `obj.annotations[...] = ...` のようには変更できません。

## 10. `AttachedX` handle と snapshot

v2の `add_name` と `add_description` は、v3では `set_name` と `set_description` です。`set_parameters` はパラメータ全体を置き換え、`add_parameters` は指定したキーだけを更新します。構築前は制約ごとにメソッドを個別に呼び出し、モデルに追加した後は型付きIDで取得した行を更新してください。

`instance.constraints[id]` や `instance.decision_variables` は、host に書き戻す `AttachedX` handle を返します。

```python
from ommx import ConstraintID
c = instance.constraints[ConstraintID(5)]
c.set_name("balance")
assert instance.constraints[ConstraintID(5)].name == "balance"
```

detached snapshot が必要な場合は `detach()` を使います。

```python
from ommx import ConstraintID
snapshot = instance.constraints[ConstraintID(5)].detach()
```

### 10.1 fixed decision-variable values

detached な `DecisionVariable` は変数定義と label の snapshot であり、固定値は持ちません。`partial_evaluate(...)` や legacy protobuf の `substituted_value` 由来の固定値は、所有者である `Instance` / `ParametricInstance` に保存されます。

```python
fixed = instance.fixed_decision_variables()

attached = instance.attached_decision_variable(1)
assert attached.substituted_value == fixed.get(1)

df = instance.decision_variables_df()
print(df["substituted_value"])
```

### 10.2 `decision_variable_analysis()` の置き換え

古い analysis object shape は公開されません。必要な role / role 由来の集合を、所有者である `Instance` から直接取得してください。

```python
roles = instance.decision_variable_roles()
role = instance.decision_variable_role(1)

fixed = instance.fixed_decision_variables()
dependent = instance.dependent_decision_variable_ids()
irrelevant = instance.irrelevant_decision_variable_ids()

df = instance.decision_variables_df()
print(df["state_role"])
```

Adapter が solver input の変数だけを必要とする場合は `instance.used_decision_variables` を使います。fixed / dependent / irrelevant の分類を見ていた移行コードは、上の role helper に置き換えてください。

## 11. named function ID

Named function 系も table-owned ID model に移行しています。`NamedFunction`、`EvaluatedNamedFunction`、`SampledNamedFunction` の row object 自身ではなく、host 側の table key が ID の source of truth です。

Python API ではユーザーが移行しやすいように `.id` を参照できる箇所がありますが、実装上の所有者は host table です。新しいコードでは、collection を走査するときに key と value を分けて扱うことを優先してください。

## 12. 要素単体の bytes round-trip

`Function` / `Linear` / `Quadratic` / `Polynomial` / `Parameter` / `NamedFunction` family / `DecisionVariable` family の要素単体 `to_bytes()` / `from_bytes()` は削除されています。

要素を永続化したい場合は、所有者である `Instance` / `Solution` / `SampleSet` へ入れてから全体を round-trip してください。新しく byte 列を作る場合は v2 bytes API を既定にします。v1 は既存の v1 consumer / file と互換を保つ必要がある場合にだけ使ってください。

top-level root についても、protobuf version を名前に含まない `Instance.to_bytes()` / `Instance.from_bytes(...)`（および `ParametricInstance`、`Solution`、`SampleSet` の同名 API）は削除されています。通常は `to_v2_bytes()` / `from_v2_bytes(...)` に置き換え、legacy な v1 wire format が明示的に必要な場合だけ `to_v1_bytes()` / `from_v1_bytes(...)` を使ってください。

```python
instance_blob = instance.to_v2_bytes()
restored = Instance.from_v2_bytes(instance_blob)
```

(13-artifact-api-archive-becomes-an-exchange-format)=
## 13. Artifact API: archive becomes an exchange format

v3 では Artifact API が SQLite Local Registry を中心に整理され、`.ommx` file は registry から明示的に export / import する exchange format になりました。

### 13.1 `ArtifactBuilder.new_archive` / `new_archive_unnamed` は削除

`.ommx` file を作る処理は、`ArtifactDraft` を commit した後の `Artifact.save(path)` に分離されました。

```python
# v2
builder = ArtifactBuilder.new_archive("my_instance.ommx", "ghcr.io/jij-inc/ommx/demo:v1")
builder.add_instance(instance)
artifact = builder.build()

# v3
draft = ArtifactDraft.new("ghcr.io/jij-inc/ommx/demo:v1")
draft.add_instance(instance)
artifact = draft.commit()
artifact.save("my_instance.ommx")
```

### 13.2 anonymous archive は `new_anonymous`

`new_archive_unnamed(path)` は `ArtifactDraft.new_anonymous()` に置き換わりました。v3 の anonymous Artifact も Local Registry 内では image name を持つため、`artifact.image_name is None` を前提にしたコードは見直してください。

```python
draft = ArtifactDraft.new_anonymous()
draft.add_instance(instance)
artifact = draft.commit()
artifact.save("my_instance.ommx")
```

`ommx prune-anonymous` は削除候補を表示します。内容を確認して `--delete` を付けると対象の参照を削除できます。参照されなくなったblobの回収は別の `ommx gc` 操作です。[Local Registryの管理](../how_to/manage_local_registry.md)を参照してください。

### 13.3 `Artifact.load_archive` は `import_archive` / `inspect_archive` に分割

v2 の `Artifact.load_archive(file)` は、v3 では目的別に分かれました。

- `Artifact.import_archive(file)` - archive を Local Registry に import し、全 layer を読める `Artifact` handle を返します。
- `Artifact.inspect_archive(file)` - registry に書き込まず、manifest / layer descriptor だけを読む read-only path です。

```python
# archive を使う
artifact = Artifact.import_archive("my_instance.ommx")

# 中身を確認するだけ
manifest = Artifact.inspect_archive("my_instance.ommx")
```

### 13.4 CLI flow

archive file を直接 push する flow は廃止されました。いったん load / import して Local Registry に入れてから、image name で push します。

```bash
ommx load my_instance.ommx
ommx push ghcr.io/jij-inc/ommx/demo:v1
```

### 13.5 Artifact migration checklist

- [ ] `ArtifactBuilder.new_archive(path, image_name).build()` を `ArtifactDraft.new(image_name).commit()` + `artifact.save(path)` に置き換える。
- [ ] `ArtifactBuilder.new_archive_unnamed(path).build()` を `ArtifactDraft.new_anonymous().commit()` + `artifact.save(path)` に置き換える。
- [ ] `Artifact.load_archive(file)` を、用途に応じて `Artifact.import_archive(file)` または `Artifact.inspect_archive(file)` に置き換える。
- [ ] `ommx push <archive-file>` を `ommx load <file>` + `ommx push <image_name>` に置き換える。
- [ ] 匿名Artifactの整理を自動化する場合は、候補の表示と削除を区別する。

## v2 から v3 へのチェックリスト

- [ ] `ommx.v1.*_pb2` import と SDK domain class の `ommx.v1` import を、top-level `ommx` からの import に置き換える。
- [ ] `.raw` / `from_raw` / `from_protobuf` / `to_protobuf` を削除する。新しく byte 列を作る場合は top-level root の `to_v2_bytes()` / `from_v2_bytes(...)` を使い、legacy v1 互換が必要な場合だけ `to_v1_bytes()` / `from_v1_bytes(...)` を使う。
- [ ] `Constraint.id` / `set_id()` / `id=` を削除し、host dict の key で ID を渡す。
- [ ] `constraints=[...]` を `constraints={id: constraint}` に置き換える。
- [ ] v1のhintを使っていた箇所を点検する。元の通常制約を保持し、特殊制約が必要な場合は信頼できるモデル定義から構築する。
- [ ] Adapterの推奨Preparationを使う場合は`solve()` / `sample()`を直接呼ぶ。Policyをcustomizeする場合は`Instance.prepare()`後に`solve_without_preparation()` / `sample_without_preparation()`を呼ぶ。独自Adapterにはpreparation-free methodを追加し、Adapter固有optionは公開するAPIごとに明示的な型付きsignatureとして宣言して、包括的な`**kwargs`は使わない。exactなprepared inputに依存するoptionのtransportを定義しない場合は、preparation-free methodだけに公開できる。
- [ ] `*_df` accessor に `()` を付ける。
- [ ] `RuntimeError` を捕捉していた `evaluate` / `partial_evaluate` 周辺を `ValueError` に変える。
- [ ] `Function.degree()` / `num_terms()` の型注釈を `None` も受け取る形に変更し、複合式の可能性がある場合は polynomial 係数 property への access を分岐する。
- [ ] `decision_variable_analysis()` を `decision_variable_roles()` / `decision_variable_role(id)` / `fixed_decision_variables()` / `dependent_decision_variable_ids()` / `irrelevant_decision_variable_ids()` / `decision_variables_df()["state_role"]` に置き換える。
- [ ] element-level `to_bytes()` / `from_bytes()` を、所有者全体の round-trip に置き換える。新規 payload は `to_v2_bytes()`、legacy v1 互換または evaluate 用 DTO では `to_v1_bytes()` を使う。
- [ ] Artifact archive API を `ArtifactDraft` / `Artifact.save` / `Artifact.import_archive` / `Artifact.inspect_archive` に移行する。

## 型付き制約IDと所有元を参照するコレクション

4種類の所有元すべてで、`.constraints` は全種類の制約を扱う読み取り専用コレクションです。
`instance.constraints[5]` は `instance.constraints[ConstraintID(5)]` に変更します。
特殊制約には `IndicatorConstraintID`・`OneHotConstraintID`・`Sos1ConstraintID` を使います。
種類別プロパティは `.constraints` と `.removed_constraints` に統合されます。
`get_constraint_by_id`・`get_removed_constraint_by_id`と、結果の`constraint_ids`は
添字アクセスとキー列挙に置き換えます。
`get_constraints` メソッドはありません。

```python
from ommx import ConstraintKind

row = instance.constraints.of_kind(ConstraintKind.Regular)[5]  # AttachedConstraint
snapshot = row.detach()  # 独立したConstraintのコピーを明示的に取得
removed = instance.removed_constraints.of_kind(ConstraintKind.Regular)[7]  # RemovedConstraintのスナップショット
```

メタデータの`constraint_context_df`・`constraint_parameters_df`・`constraint_provenance_df`・
`constraint_removed_reasons_df`も、`constraints_df`と同じくキーワード引数`kind`を必須にします。
省略時に通常制約を選ぶ動作は廃止します。`Instance`・`ParametricInstance`・`Solution`・
`SampleSet`に共通です。

`SampleSet.summary_with_constraints`は廃止します。サンプルごとの目的値と全体の
実行可能性には`samples.summary`を、制約別の結果には`samples.constraints`または
`samples.constraints_df(kind=...)`を使います。

プロパティ取得時には制約を収集せず、モデルへのアクセスは `Attached*`、結果へのアクセスは
個別のスナップショットを返します。保持したコレクションは所有元の変更を反映します。
`dict(owner.constraints)` は辞書をコピーしますが、モデルから取得した行は所有元に接続されたままです。独立したコピーを残すときは各行で `detach()` を呼びます。1種類だけ調べる場合は、
`constraints.of_kind(ConstraintKind.Sos1)`で絞ります。`constraints_df(*, kind=...)` はすべての所有元で
`kind` が必須です。モデルは有効な行を選び、`removed=True` で削除済みも含めます。
結果は保持された削除済みも含めます。詳細は{class}`コレクションの意味 <ommx.ConstraintCollection>`を参照してください。

種類を横断して列挙する型には、現在の種類のUnionではなく `ConstraintIDBase` と
`ConstraintView` を使います。特殊制約の種類が追加されても、これらの公開返り値型は変わりません。
具体的なIDによる添字アクセスは、具体的な返り値型を維持します。
モデルを変換・求解するコードでは未対応種類を明示的に拒否してください。
単に除外すると問題を緩和してしまいます。

種類が決まるメソッド引数や `from_components` のキーは引き続き整数を自動昇格し、別種類のIDを拒否します。
種類を横断するコレクションの添字アクセス・所属判定・`get`には型付きIDが必須です。
`of_kind`で絞ったビューは、その種類の整数IDを自動昇格します。
返り値・診断・provenance・DataFrameのインデックスも型付きIDを保持します。
`int()` や整数インデックスへの変換はできません。`constraint_violation(id)` はIDの型から種類を判断し、
`kind` 引数を取りません。変数IDとサンプルIDは引き続き整数です。

`extract_constraints(name)`は全種類を検索します。`Instance`・`ParametricInstance`では
添字タプルからAttachedオブジェクトへの辞書、`Solution`・`SampleSet`では違反量への辞書を
返します。**通常制約も符号付きの式の値から非負の違反量に変わります。** 決定変数の
抽出と同様にparametersは無視し、種類をまたぐ添字の重複は`ValueError`にします。

通常制約の符号付き評価値には`solution.get_constraint_value(id)`か`row.evaluated_value`を使います。詳しくは{meth}`名前と添字による取得 <ommx.Solution.extract_constraints>`を参照してください。

## 変数・制約の種類の拡張

`Kind`、`ConstraintKind`、`ProvenanceKind` は、既知の定数を持つ不変な値クラスです。
型ヒントは全種類を列挙する Enum を表しません。現在の全定数を処理した場合も、
未対応の値を明示的に拒否する分岐を残してください。`assert_never` では対応の網羅性を保証できません。
比較・hash・整数への変換と `Kind.from_pb()` / `to_pb()` は引き続き利用できます。
未知の protobuf kind は拒否します。

共通の制約コレクションは `ConstraintIDBase` と `ConstraintView` を返し、
具体的な family の ID による取得では具体的な戻り値型を維持します。
種類や family が追加されても、既存の adapter や変換がその意味に対応するとは限りません。
V3 の拡張契約は数値スカラーのドメインを対象とし、非スカラー値の格納・評価は別途設計します。

通常制約も含めたすべてのfamilyに `ConstraintKind` を使います。
従来の `SpecialConstraintKind` と文字列selectorは、
`collection.of_kind(ConstraintKind.Sos1)` や
`model.constraints_df(kind=ConstraintKind.Sos1)` に置き換えてください。
既知の定数が持つID型によって、familyビューの戻り値型の精度を維持します。
通常、利用側で型引数を記述する必要はありません。
`active_constraint_kinds` は `Regular` を含み、`active_special_constraint_kinds` は含みません。
`lower_special_constraints` に `Regular` を指定すると、変換を適用する前に拒否します。
