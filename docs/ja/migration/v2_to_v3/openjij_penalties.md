# OpenJijに渡していたペナルティを準備方針に移す

```{index} sample_qubo_sa (v2), uniform_penalty_weight (v2), penalty_weights (v2), inequality_integer_slack_max_range (v2)
```

v2で `uniform_penalty_weight` を渡していた場合や暗黙の重み1に依存していた場合は、元の目的関数の大きさに合わせて重みを選びます。この例では、許される最良の選択に比べて両方を選ぶと目的値が2だけ増えるので、重み4でその利得を上回ります。重みを選び直せるよう、元のモデルをコピーしてから準備します。実行にはOpenJij Adapterが必要です。

```python
from copy import copy

from ommx import FixedPenaltyPreparation, Instance, Sense
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

source = Instance.maximize()
x, y = source.new_binary("x"), source.new_binary("y")
source.objective = 3 * x + 2 * y
source.add_constraint(x + y == 1)
prepared = copy(source)
policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
policy.fixed_penalty = (
    FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=4.0)
)
prepared.prepare(OMMXOpenJijSAAdapter.INPUT_CLASS, policy)
samples = OMMXOpenJijSAAdapter.sample_without_preparation(prepared, num_reads=2, seed=42)
assert samples.sense == Sense.Maximize
```

サンプリング結果では元の目的関数を読めます。また、双対値を利用するコードでは、準備済み入力に出力用の目的関数が明示されているとHiGHSとPython-MIPは双対値を付けない点も確認してください。

## サンプリング結果を受け取る側を更新する

v2の `sample_qubo_sa` が返したのは生のサンプルです。v3のAdapterは評価済みの {class}`~ommx.SampleSet` を返します。手元のOpenJijのレスポンスをデコードするだけのコードなら、`response_to_samples` を `decode_to_samples` に変更します。重みを変更するときは元モデルから再度準備してください。すでに入力条件を満たすモデルを準備し直しても、目的関数に組み込まれたペナルティは置き換わりません。
