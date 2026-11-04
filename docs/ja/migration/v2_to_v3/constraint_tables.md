# 制約IDで結合する集計を移行する

```{index} Instance.constraints_df (v2), Instance.removed_constraints_df (v2), SampleSet.summary_with_constraints (v2)
```

v2では `instance.constraints_df` のインデックス `id` を、整数をキーとする業務側の表に結合できました。v3で通常制約のインデックス名は `regular_constraint_id`、値は `ConstraintID` です。結合前に、業務側の表の番号を対応する型付きIDに変換します。

```python
import pandas as pd
from ommx import ConstraintID, ConstraintKind, DecisionVariable, Instance, Sense

x = DecisionVariable.binary(0)
model = Instance.from_components(
    decision_variables=[x], objective=x, constraints={7: x <= 1}, sense=Sense.Maximize,
)
rows = model.constraints_df(kind=ConstraintKind.Regular, include=[])
owners = pd.Series({ConstraintID(7): "planning"}, name="owner")
report = rows.join(owners)
assert report.loc[ConstraintID(7), "owner"] == "planning"
```

ラベルとパラメータを列に含めるなら `include=["label", "parameters"]`、キーと値の組を1行ずつ結合するなら `constraint_parameters_df(kind=...)` を使います。変換履歴は `constraint_provenance_df(kind=...)` に由来のステップごとに記録されます。モデル側の `removed=True` は有効な行も含むので、削除済み制約だけの旧帳票を再現するには `removed_reason` が欠損していない行を選んでください。

## 許容誤差付近で実行可能性を照合する

判定では制約ごとの違反量に許容誤差を適用します。SOS1の値が `(1, 0.00006, 0.00006)` のとき、非ゼロ値を一つだけ残すには、合計 `0.00012` の変更が必要です。小さい二値はそれぞれ `0.0001` 未満でも、許容誤差 `0.0001` でこの制約は実行不可能です。帳票を照合するときは、評価時に保存された許容誤差を使い、同じ種類の制約を比較してください。問合せ時の許容誤差を変えても、離散値の正規化はやり直されません。定義は {meth}`~ommx.Solution.total_violation` にあります。
