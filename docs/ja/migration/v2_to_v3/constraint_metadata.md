# メタデータの編集先を確認する

```{index} Constraint.add_name (v2), Constraint.add_description (v2), Instance.get_constraint_by_id (v2), Instance.decision_variable_analysis (v2)
```

2.9.4では、`get_constraint_by_id` で取り出した制約の名前を変更しても、変更先は取り出したコピーだけでした。このコードを移行するときは、モデルの名前を更新したかったのか、表示用のコピーだけを編集したかったのかを確認してください。v3では次のように書き分けます。

```python
from ommx import Instance

model = Instance.minimize()
x = model.new_binary("x")
live = model.add_constraint(x <= 1, "original")
local = live.detach()
local.set_name("display")
assert model.constraints[live.constraint_id].name == "original"
live.set_name("capacity")
assert model.constraints[live.constraint_id].name == "capacity"
```

`dict(model.constraints)` で辞書を作っても、値は引き続きモデルに接続されています。独立させたい有効な行ごとに `detach()` を呼んでください。切り離した制約のsetterは呼出し元を変更してコピーを返すため、一つのオブジェクトに複数の変更を加えるときは、そのオブジェクトに対して個別に呼びます。

## 変数の役割を読む処理を置き換える

一変数の役割を判定するなら {meth}`~ommx.Instance.decision_variable_role`、複数の変数を帳票にまとめるなら `decision_variables_df()` の `state_role`・`substituted_value` 列を使います。Adapterでソルバーに渡す変数だけが必要なら `used_decision_variables` を使います。固定値や従属変数を候補の評価用に保持している場合、これらの問いの答えは異なります。
