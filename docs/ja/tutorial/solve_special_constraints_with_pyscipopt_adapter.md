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

# PySCIPOpt Adapterで特殊制約をそのまま解く

OMMXでは、ソルバーが専用の機能で扱える制約を「特殊制約」として表現できます。対応するAdapterを使えば、特殊制約を通常の制約へ変換せず、そのままソルバーへ渡せます。

このチュートリアルでは、次の2種類を含む小さなモデルを作り、PySCIPOpt Adapterで解きます。

- **Indicator制約**: 指定したBinary変数が1のときだけ有効になる制約
- **SOS1制約**: 指定した変数のうち、高々1つだけが非ゼロになる制約

## 必要なライブラリのインストール

```
pip install ommx-pyscipopt-adapter
```

## 特殊制約を持つInstanceを作る

次のモデルには4つのBinary変数があります。`enabled = 1` のときだけ `option_a + option_b <= 1` を課し、さらに `option_b` と `option_c` のうち高々1つだけを選べるようにします。

```{code-cell} ipython3
from ommx import Instance, Sos1Constraint

source = Instance.maximize()
enabled = source.new_binary("enabled")
option_a = source.new_binary("option_a")
option_b = source.new_binary("option_b")
option_c = source.new_binary("option_c")

source.objective = (
    10 * enabled
    + 6 * option_a
    + 5 * option_b
    + 4 * option_c
)

# enabled = 1 のときだけ option_a + option_b <= 1
source.add_indicator_constraint(
    (option_a + option_b <= 1).with_indicator(enabled)
)

# option_b と option_c のうち、高々1つだけが非ゼロ
source.add_sos1_constraint(
    Sos1Constraint(
        variables=[option_b, option_c],
        name="exclusive_options",
    )
)

assert len(source.indicator_constraints) == 1
assert len(source.sos1_constraints) == 1
source
```

`Instance.maximize()` で空の最大化問題を作り、`new_binary()` と `add_*_constraint()` でモデルを順に組み立てています。

## PySCIPOpt Adapterで解く

PySCIPOpt Adapterは、線形なIndicator制約とSOS1制約を直接受け取れます。このモデルには事前の変換が必要ないので、`source` をそのまま `solve()` に渡します。

```{code-cell} ipython3
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

solution = OMMXPySCIPOptAdapter.solve(source)

assert solution.feasible
assert abs(solution.objective - 20.0) < 1e-8
solution.decision_variables_df()
```

この解では `enabled = 1`、`option_a = 1`、`option_b = 0`、`option_c = 1` となり、Indicator制約とSOS1制約をどちらも満たします。呼び出し前に通常制約へ書き換える必要はありませんでした。

同じモデルを特殊制約に直接対応していないAdapterで解く場合は、次の[Adapter向けにInstanceを準備する](./prepare_instance_for_adapter.md)に進んでください。各特殊制約の意味と、個別APIの参照先は[特殊制約型](../user_guide/special_constraints.md)にまとめています。
