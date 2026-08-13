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

# Adapter の exact input と Instance preparation

Adapter を使う標準的な流れは、Adapter が宣言する exact な `INPUT_CLASS` と推奨 Policy を取得し、必要に応じて Policy を編集してから、呼び出し側が所有する {class}`~ommx.Instance` に適用することです。その同じ `Instance` を、準備を暗黙には行わない厳格な Adapter API に渡します。

```{important}
覚えることは1つです。**Adapter は入力を受け入れ可能な形へ暗黙に変換しません。どの preparation を行うかを選び、実行するのは呼び出し側です。**
```

## 標準 workflow

次の例では、OneHot 制約を含むモデルを HiGHS Adapter 向けに準備します。

```{code-cell} ipython3
from ommx import DecisionVariable, Instance, OneHotConstraint
from ommx_highs_adapter import OMMXHighsAdapter

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
instance = Instance.from_components(
    decision_variables=xs,
    objective=sum((i + 1) * x for i, x in enumerate(xs)),
    constraints={},
    one_hot_constraints={0: OneHotConstraint(variables=xs)},
    sense=Instance.MAXIMIZE,
)

input_class = OMMXHighsAdapter.INPUT_CLASS
assert input_class is not None

policy = OMMXHighsAdapter.recommended_preparation_policy()
# Application 固有の選択が必要なら、ここで policy の public field を編集します。
instance.prepare(input_class, policy)

# Preparation の成功が保証するのは INPUT_CLASS membership です。
assert input_class.contains(instance)
# Adapter 固有の precondition は通常の applicability check で確認します。
OMMXHighsAdapter.require_applicable(instance)
solution = OMMXHighsAdapter.solve(instance)
assert abs(solution.objective - 3.0) < 1e-8
```

{meth}`~ommx.Instance.prepare` は `instance` を in-place に変更し、`None` を返します。完全な例は [Adapter 向けに Instance を準備する](../tutorial/prepare_instance_for_adapter.md) を参照してください。

## Exact な INPUT_CLASS と Adapter applicability

{class}`~ommx.InstanceClass` は、Adapter が**変換なしで直接受け取れる具体的な `Instance` 値の集合**です。各 {class}`~ommx.InstanceClassClause` は条件の論理積であり、`InstanceClass` はそれらの有限和です。現在使用されている変数 kind、目的関数と制約の次数、制約 relation、active な特殊制約 family、optimization sense などを、渡された値そのものについて判定します。

- `input_class.contains(instance)` は membership を `bool` で返します。
- `input_class.check_membership(instance)` は clause ごとの構造化された mismatch を返します。
- どちらも `instance` を変更せず、lowering や encoding などの preparation を行いません。

`INPUT_CLASS` membership と Adapter applicability は別の条件です。

| 条件 | 所有者 | 検査 API |
|---|---|---|
| OMMX で記述できる exact な入力構造 | `InstanceClass` | `contains()` / `check_membership()` |
| Backend 固有の制限や変換可能性 | Adapter | `check_applicability()` / `require_applicable()` |

{meth}`~ommx.adapter.SolverAdapter.check_applicability` は最初に `INPUT_CLASS` membership を調べ、member の場合だけ Adapter 固有の precondition を評価します。例えば、Backend が扱える ID の範囲や、Backend 形式へ変換した係数が有限であることは Adapter precondition です。{meth}`~ommx.adapter.SolverAdapter.require_applicable` は同じ report を使い、適用不能なら {class}`~ommx.adapter.AdapterNotApplicableError` を送出します。

直接の `solve()` / `sample()` / Adapter constructor は厳格です。入力を applicable にするための変換を実行せず、呼び出し側の `Instance` を変更しません。

## 推奨 Policy と preparation の実行は別

{meth}`~ommx.adapter.SolverAdapter.recommended_preparation_policy` は、その Adapter の `INPUT_CLASS` へ到達する際に一般的に有用な {class}`~ommx.PreparationPolicy` を返します。

- 呼び出すたびに新しい Policy を返すため、呼び出し側が安全に編集できます。
- `Instance` を参照または変更しません。
- preparation を実行しません。
- 特定の `Instance` が必ず準備できることや、Adapter 固有の precondition を満たすことを保証しません。

`INPUT_CLASS` と Policy は独立した値であり、呼び出し側が `instance.prepare(input_class, policy)` に渡します。`prepare()` は target-directed です。成功時には次が成り立ちます。

```python
assert input_class.contains(instance)
```

この postcondition に Adapter 固有の precondition は含まれません。続けて `require_applicable()` または `solve()` / `sample()` の通常の applicability check を使ってください。

## Native input と preparation 後の input

同じ特殊制約でも、どの形を exact input として受け入れるかは Adapter ごとに異なります。

| Adapter | Exact input | 推奨 preparation の例 |
|---|---|---|
| PySCIPOpt | 線形 Indicator と SOS1 を native に受け入れる | OneHot だけを通常制約へ lowering |
| HiGHS / Python-MIP | 通常の線形制約を受け入れる | Indicator、OneHot、SOS1 を通常制約へ lowering |
| OpenJij | 制約なし Binary 最小化問題を受け入れる | 特殊制約 lowering、sense 正規化、Integer slack、使用中 Integer の encoding。固定 penalty の大きさは呼び出し側が選択 |

したがって、PySCIPOpt がnativeに受け入れる線形 Indicator や SOS1 を、共通化のためだけに事前変換する必要はありません。一方、同じ source model を HiGHS に渡す場合は、HiGHS の `INPUT_CLASS` と推奨 Policy を使って別の exact input に準備できます。特殊制約の数学的定義、native support、および個別の変換 API は [特殊制約型](./special_constraints.md) を参照してください。

変換前の model も必要なら、`prepare()` の前に明示的にコピーしてください。Preparation 後の同じ `Instance` は、変数 dependency と removed constraint を保持し、Adapter が返した {class}`~ommx.Solution` や {class}`~ommx.SampleSet` を評価する owner になります。

## Advanced: phase 順序と失敗時の状態

`PreparationPolicy` の各 field は、既存の `Instance` owner operation を選択する optional phase です。`Instance.prepare()` は、選択された phase を高々1回ずつ次の固定順序で適用します。

1. 特殊制約 lowering
2. optimization sense の正規化
3. Integer slack の導入
4. 使用中 Integer 変数の encoding
5. 固定 weight penalty

`prepare()` は最初と各 selected phase の完了後に target `InstanceClass` の membership を再評価し、member になった時点で停止します。最初から member なら exact identity であり、Policy の phase は実行されません。

Preparation 全体を囲む global transaction はありません。各 phase が委譲する owner operation はそれぞれの validation と mutation contract を保ちますが、後の phase が失敗しても、それより前に完了した変更は rollback されません。Owner operation によっては、同じ phase の中でも複数の対象を順に変更するため、途中まで変更された状態で失敗することがあります。Owner operation の既存の exception はそのまま伝播します。

すべての configured phase を適用しても target に到達しなかった場合、{class}`~ommx.PreparationTargetNotReachedError` が送出されます。その `report` 属性には、partially prepared な最終状態についての {class}`~ommx.InstanceClassMembershipReport` が入っています。

## まとめ

| やりたいこと | API |
|---|---|
| Adapter が直接受け取れる exact input を宣言する | `INPUT_CLASS` / {class}`~ommx.InstanceClass` |
| Exact membership を調べる | `contains()` / `check_membership()` |
| Adapter 固有の precondition も含めて検査する | `check_applicability()` / `require_applicable()` |
| Adapter の推奨 preparation を取得する | `recommended_preparation_policy()` |
| Caller-owned な Policy を target へ適用する | {meth}`~ommx.Instance.prepare` |
| Prepared input を厳格な Adapter へ渡す | `solve()` / `sample()` / Adapter constructor |
