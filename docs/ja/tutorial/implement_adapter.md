---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

# OMMX Adapterを実装する

[複数のAdapterで最適化問題を解いて結果を比較する](../tutorial/switching_adapters)で触れた通り、OMMX Adapterは共通化されたAPIを持っています。この共通化されたAPIは、OMMX Python SDKが用意している抽象基底クラスを継承することで実現されています。OMMXはAdapterの性質に応じて二つの抽象基底クラスを用意しています。

- [`ommx.adapter.SolverAdapter`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/adapter/index.html#ommx.adapter.SolverAdapter): 一つの解を返す最適化ソルバーのための抽象基底クラス
- [`ommx.adapter.SamplerAdapter`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/adapter/index.html#ommx.adapter.SamplerAdapter): サンプリングベースの最適化ソルバーのための抽象基底クラス

複数の解が得られるソルバーは、特に複数得られたサンプルのうち最善のものを選択することによって、自動的に単一の解を返すソルバーと見なす事ができるため、`SamplerAdapter` は `SolverAdapter` を継承しています。Adapterを作るときにどちらを実装するか悩んだら、出力される解の数を見て、一つの解を返すなら `SolverAdapter`、複数の解を返すなら `SamplerAdapter` を継承すると良いでしょう。たとえば [PySCIPOpt](https://github.com/scipopt/PySCIPOpt) などの厳密解を1つ返す最適化ソルバーは`SolverAdapter` を使い、[OpenJij](https://github.com/OpenJij/OpenJij) などの複数の解を返すサンプラーは`SamplerAdapter` を使います。

OMMXでは `ommx.adapter.SolverAdapter` を継承したクラスを **Solver Adapter**、`ommx.adapter.SamplerAdapter` を継承したクラスを **Sampler Adapter** と呼びます。
またここでの説明のため、PySCIPOptやOpenJijのようにAdapterがラップしようとしているソフトウェアのことをバックエンドソルバーと呼びます。

## Adapterの処理の流れ

Adapterの処理は大雑把にいうと次の3ステップからなります：

1. `ommx.Instance` をバックエンドソルバーが読める形式に変換する
2. バックエンドソルバーを実行して解を取得する
3. バックエンドソルバーの解を `ommx.Solution` や `ommx.SampleSet` に変換して返す

2.はバックエンドソルバーの使い方そのものなので、これは既知としてこのチュートリアルでは扱いません。ここでは 1. と 3. をどのように実装するかを説明します。

多くのバックエンドソルバーが最適化問題を表すための必要な最小限の情報だけを、アルゴリズムに応じた形で受け取るように作られているのに比べて、`ommx.Instance` はデータ分析の一部として最適化を行うことを想定しているためより多くの情報を持っています。なのでステップ 1. では多くの情報を削ぎ落とすことになります。またOMMXでは決定変数や制約条件は連番とは限らないIDで管理されていますが、バックエンドソルバーによっては名前で管理されいたり、連番で管理されていることがあります。この対応関係は 3. の処理で必要になるのでAdapterが管理しておく必要があります。

逆にステップ 3. では `ommx.Solution` や `ommx.SampleSet` はバックエンドソルバーの出力だけからは構築できないので、まずバックエンドソルバーの返した解と 1. の時の情報から `ommx.State` あるいは `ommx.Samples` を構築し、それを `ommx.Instance` を使って `ommx.Solution` や `ommx.SampleSet` に変換します。

## Solver Adapterを実装する

ここでは PySCIPOpt を例としてSolver Adapterを実装してみましょう。なお完全な例は [ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter) を確認してください。

ここではチュートリアルということで、順番に実行しやすいように以下のように作業します。

- `ommx.Instance` から PySCIPOpt のモデルを構築するための関数を順番に実装していきます。
- 最後にこれらの関数を `OMMXPySCIPOptAdapter` クラスとしてまとめます

### カスタム例外

まずカスタム例外を定義しておくといいでしょう。これによりユーザーは例外が発生したときに、どの部分が問題を引き起こしているのかを理解しやすくなります。

```{code-cell} ipython3
class OMMXPySCIPOptAdapterError(Exception):
    pass
```

OMMX は広いクラスの最適化問題を保存できるため、Adapter が受け入れる具体的な集合は、後述する `INPUT_CLASS` で宣言します。以下の converter helper は、自身が受け取る表現を検証し、solver input の構築中に error を返すことがあります。この converter または backend の失敗は追加の applicability 条件ではありません。Adapter applicability は `INPUT_CLASS` membership だけで定義されます。

### 決定変数を設定する

PySCIPOptは決定変数を名前で管理するので、OMMXの決定変数のIDを文字列にして名前として登録します。これにより後述する `decode_to_state` においてPySCIPOptの決定変数から `ommx.State` を復元することができます。これはバックエンドソルバーの実装に応じて適切な方法が変わることに注意してください。重要なのは解を得た後に `ommx.State` に変換するための情報を保持することです。

```{code-cell} ipython3
import pyscipopt
from ommx import Instance, Solution, DecisionVariable, Constraint, State, Function

def set_decision_variables(
    model: pyscipopt.Model,  # チュートリアルのために状態を引数で受け取っているがclassで管理するのが一般的
    instance: Instance
) -> dict[str, pyscipopt.Variable]:
    """
    モデルに決定変数を追加し、変数名のマッピングを作成して返す
    """
    # OMMXの決定変数の情報からPySCIPOptの変数を作成
    for var in instance.used_decision_variables:
        if var.kind == DecisionVariable.BINARY:
            model.addVar(name=str(var.id), vtype="B")
        elif var.kind == DecisionVariable.INTEGER:
            model.addVar(
                name=str(var.id), vtype="I", lb=var.bound.lower, ub=var.bound.upper
            )
        elif var.kind == DecisionVariable.CONTINUOUS:
            model.addVar(
                name=str(var.id), vtype="C", lb=var.bound.lower, ub=var.bound.upper
            )
        else:
            # 未対応の決定変数の種類がある場合はエラー
            raise OMMXPySCIPOptAdapterError(
                f"Unsupported decision variable kind: "
                f"id: {var.id}, kind: {var.kind}"
            )        # 目的関数が2次の場合、線形化のために補助変数を追加
        if instance.objective.degree() == 2:
            model.addVar(
                name="auxiliary_for_linearized_objective", vtype="C", lb=None, ub=None
            )

    # モデルに追加された変数へアクセスするための辞書を作成
    return {var.name: var for var in model.getVars()}
```

### `ommx.Function` を `pyscipopt.Expr` に変換する

`ommx.Function` を `pyscipopt.Expr` に変換するための関数を実装します。`ommx.Function` はOMMXの決定変数のIDしか持っていないので、IDからPySCIPOpt側の変数を取得する必要があり、そのために `set_decision_variables` で作成した変数名と変数のマッピングを使います。

```{code-cell} ipython3
def make_linear_expr(function: Function, varname_map: dict) -> pyscipopt.Expr:
    """線形式を生成するヘルパー関数"""
    return (
        pyscipopt.quicksum(
            coeff * varname_map[str(id)]
            for id, coeff in function.linear_terms.items()
        )
        + function.constant_term
    )

def make_quadratic_expr(function: Function, varname_map: dict) -> pyscipopt.Expr:
    """2次式を生成するヘルパー関数"""
    quad_terms = pyscipopt.quicksum(
        varname_map[str(row)] * varname_map[str(col)] * coeff
        for (row, col), coeff in function.quadratic_terms.items()
    )

    linear_terms = pyscipopt.quicksum(
        coeff * varname_map[str(var_id)]
        for var_id, coeff in function.linear_terms.items()
    )

    constant = function.constant_term

    return quad_terms + linear_terms + constant
```

### 目的関数と制約条件を設定する

`pyscipopt.Model` に目的関数と制約条件を追加します。この部分はバックエンドソルバーが何をどのようにサポートしているのかの知識が必要になります。例えば以下のコードでは、PySCIPOptが目的関数として2次式を直接扱うことができないため、[PySCIPOptのドキュメント](https://pyscipopt.readthedocs.io/en/latest/tutorials/expressions.html#non-linear-objectives)に従って補助変数を導入しています。

```{code-cell} ipython3
import math

def set_objective(model: pyscipopt.Model, instance: Instance, varname_map: dict):
    """モデルに目的関数を設定"""
    objective = instance.objective

    if instance.sense == Instance.MAXIMIZE:
        sense = "maximize"
    elif instance.sense == Instance.MINIMIZE:
        sense = "minimize"
    else:
        raise OMMXPySCIPOptAdapterError(
            f"Sense not supported: {instance.sense}"
        )

    degree = objective.degree()
    if degree == 0:
        model.setObjective(objective.constant_term, sense=sense)
    elif degree == 1:
        expr = make_linear_expr(objective, varname_map)
        model.setObjective(expr, sense=sense)
    elif degree == 2:
        # PySCIPOptでは2次の目的関数を直接サポートしていないため、補助変数を使って線形化
        auxilary_var = varname_map["auxiliary_for_linearized_objective"]

        # 補助変数を目的関数として設定
        model.setObjective(auxilary_var, sense=sense)

        # 補助変数に対する制約を追加
        expr = make_quadratic_expr(objective, varname_map)
        if sense == "minimize":
            constr_expr = auxilary_var >= expr
        else:  # sense == "maximize"
            constr_expr = auxilary_var <= expr

        model.addCons(constr_expr, name="constraint_for_linearized_objective")
    else:
        raise OMMXPySCIPOptAdapterError(
            "The objective function must be `constant`, `linear`, `quadratic`."
        )
        
def set_constraints(model: pyscipopt.Model, instance: Instance, varname_map: dict):
    """モデルに制約条件を設定"""
    # 通常の制約条件を処理。instance.constraints は制約IDをキーとする dict[int, Constraint]。
    for constraint_id, constraint in instance.constraints.items():
        # 制約関数の種類に基づいて式を生成
        f = constraint.function
        degree = f.degree()
        if degree == 0:
            # 定数制約の場合、実行可能かどうかをチェック
            constant_value = f.constant_term
            if constraint.equality == Constraint.EQUAL_TO_ZERO and math.isclose(
                constant_value, 0, abs_tol=1e-6
            ):
                continue
            elif (
                constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
                and constant_value <= 1e-6
            ):
                continue
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Infeasible constant constraint was found: id {constraint_id}"
                )
        elif degree == 1:
            expr = make_linear_expr(f, varname_map)
        elif degree == 2:
            expr = make_quadratic_expr(f, varname_map)
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Constraints must be either `constant`, `linear` or `quadratic`."
                f"id: {constraint_id}, "
                f"degree: {degree}"
            )

        # 制約種別（等式/不等式）に基づいて制約を追加
        if constraint.equality == Constraint.EQUAL_TO_ZERO:
            constr_expr = expr == 0
        elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
            constr_expr = expr <= 0
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Not supported constraint equality: "
                f"id: {constraint_id}, equality: {constraint.equality}"
            )

        # 制約をモデルに追加
        model.addCons(constr_expr, name=str(constraint_id))
```

また、バックエンドソルバーが特殊な制約条件（例: [SOS制約](https://en.wikipedia.org/wiki/Special_ordered_set) など）をサポートしている場合は、それに対応するための関数を追加する必要があります。

以上で `ommx.Instance` から `pycscipopt.Model` が構築できるようになりました。

### 得られた解を `ommx.State` に変換する

次に、PySCIPOptのモデルを解いて得られた解を `ommx.State` に変換する関数を実装します。まず解けているかを確認します。SCIPには最適性を保証する機能や解が非有界であることを検知する機能があるので、それらを検知していたら対応した例外を投げます。これもバックエンドソルバーに依存します。

```{warning}
特に `ommx.adapter.InfeasibleDetected` は解がInfeasibleではなくて最適化問題自体がInfeasible、つまり **一つも解を持ち得ないことが保証できた** という意味であることに注意してください。ヒューリスティックソルバーが一つも実行可能解を見つけられなかった場合にこれを使ってはいけません。
```

```{code-cell} ipython3
from ommx.adapter import InfeasibleDetected, UnboundedDetected

def decode_to_state(model: pyscipopt.Model, instance: Instance) -> State:
    """最適化済みのPySCIPOpt Modelからommx.Stateを作成する"""
    if model.getStatus() == "unknown":
        raise OMMXPySCIPOptAdapterError(
            "The model may not be optimized. [status: unknown]"
        )

    if model.getStatus() == "infeasible":
        raise InfeasibleDetected("Model was infeasible")

    if model.getStatus() == "unbounded":
        raise UnboundedDetected("Model was unbounded")

    try:
        # 最適解を取得
        sol = model.getBestSol()
        # 変数名と変数のマッピングを作成
        varname_map = {var.name: var for var in model.getVars()}
        # 変数IDと値のマッピングを持つStateを作成
        return State(
            entries={
                var.id: sol[varname_map[str(var.id)]]
                for var in instance.used_decision_variables
            }
        )
    except Exception:
        raise OMMXPySCIPOptAdapterError(
            f"There is no feasible solution. [status: {model.getStatus()}]"
        )
```

### `ommx.adapter.SolverAdapter` を継承した class を作る

最後に、Adapter毎のAPIを揃えるために `ommx.adapter.SolverAdapter` を継承したクラスを作成します。これは `@abstractmethod` を含む次のような抽象基底クラスです：

```python
from typing import ClassVar

class SolverAdapter(ABC):
    # Adapter applicability を完全に定義する OMMX の条件
    INPUT_CLASS: ClassVar[InstanceClass]

    @classmethod
    def recommended_preparation_policy(cls) -> PreparationPolicy:
        return PreparationPolicy()

    @classmethod
    @abstractmethod
    def solve_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        pass

    @property
    @abstractmethod
    def solver_input(self) -> SolverInput:
        pass

    @abstractmethod
    def decode(self, data: SolverOutput) -> Solution:
        pass
```

この抽象基底クラスは以下の3通りのユースケースを想定しています:

- 通常の1 call workflowでは、継承した`solve`クラスメソッドを使う。入力をcopyし、
  `INPUT_CLASS`向けの`recommended_preparation_policy()`を適用してから、一時的な
  Prepare済みcopyに対して`solve_without_preparation`を呼び出します。
- Application側でexactなAdapter inputを用意する場合は、Preparationを行わない
  `solve_without_preparation`を使う。
- バックエンドソルバーのパラメータなどを調整する場合は、 `solver_input` を使ってバックエンドソルバーの入力用のデータ構造（今回は `pyscipopt.Model`）を取得し、調整した後にバックエンドソルバーへ入力し、最後にバックエンドソルバーの出力を `decode` で変換する。

追加optionがないAdapterは、継承した`solve`をそのまま使えます。Adapter固有optionは、それを公開する
各APIで明示的に型付けします。Preparationの前後で意味が変わらないoptionはeasy methodから
preparation-free methodへ明示的に転送できます。exactなprepared inputに意味が依存し、Preparation過程をまたぐ
transportを定義しない場合、具体的なAdapterはそのoptionを`solve_without_preparation`だけに公開できます。
未知のoptionをtype checkerが拒否できなくなるため、
包括的な`**kwargs`は使いません。予約済みの`diagnostics` keywordは
`Run.log_solve`が管理します。`Run.log_solve(..., store_diagnostics=True)`を使う場合、adapterは
そのsinkにadapter定義のdiagnostic reportを記録できます。`None`の場合、diagnosticsは無効です。

#### 入力 class と推奨 Preparation

Adapter は、受け取れる具体的な `Instance` 値の集合 `INPUT_CLASS` だけで applicability を定義します。`check_applicability()` は呼び出し元の instance を変更せずに membership を report し、`require_applicable()` は membership が満たされない場合だけ同じ構造化 report で例外を送出します。

Applicability は、その後の全ての変換や backend operation の成功を保証するものではありません。`as_linear()` のような helper が扱う、より狭い表現を converter が検証したり、solver input の構築中に backend が数値や実装上の上限を拒否したりすることがあります。これらは converter または backend の error として扱い、Adapter applicability の別の source of truth にしないでください。

通常の`solve` APIがPreparationを所有します。呼び出し元のInstanceをcopyし、
`recommended_preparation_policy()`が返すfreshなPolicyをそのcopyへ適用してから、
`solve_without_preparation`を呼び出します。Preparationやbackendが失敗した場合も含め、呼び出し元の
Instanceは変更されません。推奨Policy自体はinstanceを参照・変更せず、Preparationを
実行せず、Adapter applicabilityも保証しません。

`solve_without_preparation`はpreparation-freeなAPIです。`INPUT_CLASS`のexactなmemberを要求し、
満たさなければ`AdapterNotApplicableError`を送出します。customなPreparation Policyが
必要なapplicationは編集済みPolicyを{meth}`~ommx.Instance.prepare`でInstanceへ適用し、
そのInstanceを`solve_without_preparation`へ渡します。

推奨 Policy から特殊制約 lowering を有効にする場合は、以下の family selector を使います：

- `SpecialConstraintKind.Indicator`: インジケーター制約 (`binvar = 1 → f(x) <= 0`)
- `SpecialConstraintKind.OneHot`: バイナリ変数集合のうち丁度1つが1
- `SpecialConstraintKind.Sos1`: 変数集合のうち高々1つが非ゼロ

`Instance` が現在保持する family は {attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>` で確認できます。選択された Preparation phase は {meth}`Instance.lower_special_constraints <ommx.Instance.lower_special_constraints>` に委譲し、active な family を通常制約へ変換します（indicator/SOS1 は Big-M、one-hot は線形等式）。Validation と数学的な意味は引き続き owner operation が定義します。

```{important}
`INPUT_CLASS`は`solve_without_preparation`が受け取る時点の入力値そのものを記述し、そのmembershipが
applicabilityの完全な条件です。`solve`はprivateなworking copyだけをPrepareします。
Preparationの成功はmembershipを保証します。その後もsolver inputの構築時にconverter
固有またはbackend固有のvalidationが失敗することはありますが、その失敗によって入力が
「not applicable」になるわけではありません。
```

ここまでで用意した関数を使って次のように実装することができます：

```{code-cell} ipython3
from ommx.adapter import DiagnosticsSink, SolverAdapter
from ommx import (
    Equality,
    InstanceClass,
    InstanceClassClause,
    Kind,
    PolynomialRequirement,
    PreparationPolicy,
    Sense,
    SpecialConstraintKind,
    SpecialConstraintPreparation,
)

class OMMXPySCIPOptAdapter(SolverAdapter):
    INPUT_CLASS = InstanceClass(
        [
            InstanceClassClause(
                label="tutorial-quadratic-mip",
                allowed_variable_kinds={Kind.Binary, Kind.Integer, Kind.Continuous},
                objective_polynomial_requirement=PolynomialRequirement.at_most(2),
                regular_constraint_polynomial_requirements={
                    Equality.EqualToZero: PolynomialRequirement.at_most(2),
                    Equality.LessThanOrEqualToZero: PolynomialRequirement.at_most(2),
                },
                indicator_body_polynomial_requirements={
                    Equality.EqualToZero: PolynomialRequirement.at_most(1),
                    Equality.LessThanOrEqualToZero: PolynomialRequirement.at_most(1),
                },
                allows_sos1=True,
                allowed_senses={Sense.Minimize, Sense.Maximize},
            )
        ]
    )

    @classmethod
    def recommended_preparation_policy(cls) -> PreparationPolicy:
        return PreparationPolicy(
            special_constraints=SpecialConstraintPreparation.lower_special_constraints(
                kinds={SpecialConstraintKind.OneHot}
            )
        )

    def __init__(
        self,
        ommx_instance: Instance,
    ):
        self.require_applicable(ommx_instance)
        self.instance = ommx_instance
        self.model = pyscipopt.Model()
        self.model.hideOutput()

        # 関数を使用してモデルを構築
        self.varname_map = set_decision_variables(self.model, self.instance)
        set_objective(self.model, self.instance, self.varname_map)
        set_constraints(self.model, self.instance, self.varname_map)

    @classmethod
    def solve_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """
        PySCIPoptを使ってommx.Instanceを解き、ommx.Solutionを返す
        """
        _ = diagnostics
        adapter = cls(ommx_instance)
        model = adapter.solver_input
        model.optimize()
        return adapter.decode(model)

    @property
    def solver_input(self) -> pyscipopt.Model:
        """生成されたPySCIPOptモデルを返す"""
        return self.model

    def decode(self, data: pyscipopt.Model) -> Solution:
        """
        最適化後のpyscipopt.ModelとOMMX Instanceからommx.Solutionを生成する
        """
        # 解の状態をチェック
        if data.getStatus() == "infeasible":
            raise InfeasibleDetected("Model was infeasible")

        if data.getStatus() == "unbounded":
            raise UnboundedDetected("Model was unbounded")

        # 解を状態に変換
        state = decode_to_state(data, self.instance)
        # インスタンスを使用して解を評価
        solution = self.instance.evaluate(state)

        # backend statusをoutput-objective contractに従って写す
        if data.getStatus() == "optimal":
            solution.optimality = self.instance.map_active_optimality(
                Solution.OPTIMAL
            )

        return solution
```

`map_active_optimality()`は、active formulationの最適性が返却する`Solution`の
objectiveに対する最適性も証明するときにだけ、そのstatusを保持します。

通常の呼び出しでは、InstanceのcopyとPreparationが自動的に行われます：

```python
solution = OMMXPySCIPOptAdapter.solve(instance)
```

通常の呼び出しでは、呼び出し元の`instance`は変更されません。Preparationをcustomizeする
場合は、Instanceをin-placeでPrepareしてpreparation-free APIを呼び出します：

```python
input_class = OMMXPySCIPOptAdapter.INPUT_CLASS
policy = OMMXPySCIPOptAdapter.recommended_preparation_policy()
# Application に異なる選択が必要なら、ここで public field を編集します。
instance.prepare(input_class, policy)
solution = OMMXPySCIPOptAdapter.solve_without_preparation(instance)
```

`solve_without_preparation()`は`INPUT_CLASS` membershipを検査し、その後のPySCIPOpt modelの
構築・求解時にはconverterまたはbackendのerrorを返すことがあります。

これでSolver Adapter完成です 🎉

````{note}
`timeout`のようにPreparationをまたいでも意味が変わらないoptionは、両方のAPIで提供できます。
その場合は両方で明示的に型付けし、easy APIは独自のcopyをPrepareしてから、そのoptionを
preparation-free APIへ転送します。

```python
import copy

class MyAdapter(SolverAdapter):
    INPUT_CLASS = input_class

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        timeout: int | None = None,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        prepared = copy.copy(ommx_instance)
        prepared.prepare(cls.INPUT_CLASS, cls.recommended_preparation_policy())
        return cls.solve_without_preparation(
            prepared,
            timeout=timeout,
            diagnostics=diagnostics,
        )

    @classmethod
    def solve_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        timeout: int | None = None,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        cls.require_applicable(ommx_instance)
        ...
```
````

### Solver Adapterを使ってナップザック問題を解く

動作確認のため、これを使ってナップザック問題を解いてみましょう

```{code-cell} ipython3
v = [10, 13, 18, 31, 7, 15]
w = [11, 25, 20, 35, 10, 33]
W = 47
N = len(v)

x = [
    DecisionVariable.binary(
        id=i,
        name="x",
        subscripts=[i],
    )
    for i in range(N)
]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(v[i] * x[i] for i in range(N)),
    constraints={0: sum(w[i] * x[i] for i in range(N)) - W <= 0},
    sense=Instance.MAXIMIZE,
)

solution = OMMXPySCIPOptAdapter.solve(instance)
```

## Sampler Adapterを実装する

次にOpenJijを使ったSampler Adapterを作ってみましょう。OpenJijには Simulated Annealing (SA) による [`openjij.SASampler`](https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SASampler)と Simulated Quantum Annealing (SQA) による [`openjij.SQASampler`](https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SQASampler) が含まれています。このチュートリアルでは、 `SASampler` を例に説明します。

このチュートリアルでは簡単のためにOpenJijに渡すパラメータは省略しています。詳しくは [`ommx-openjij-adapter`](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-openjij-adapter) の実装を参照してください。また OpenJij Adapterの使い方については [OMMX AdapterでQUBOからサンプリングする](../tutorial/tsp_sampling_with_openjij_adapter) を参照してください。

### `openjij.Response` から `ommx.Samples` への変換

OpenJijは決定変数をOMMXと同様に連番とは限らないIDで管理しているので、PySCIPOptの時のようにIDの対応表を作る必要はありません。

OpenJijのサンプル結果は `openjij.Response` として得られるので、これを `ommx.Samples` に変換する関数を実装します。OpenJijは同じサンプルが得られた時、それが発生した回数を `num_occurrence` として返します。一方 `ommx.Samples` は個々のサンプルが固有のサンプルIDをもち、同じ値を持つサンプルは `SamplesEntry` として圧縮されます。この差異を埋めるための変換が必要なことに注意します。

```{code-cell} ipython3
import openjij as oj
from ommx import Instance, SampleSet, Solution, Samples, State

def decode_to_samples(response: oj.Response) -> Samples:
    samples = Samples({})  # Create empty samples
    sample_id = 0

    num_reads = len(response.record.num_occurrences)
    for i in range(num_reads):
        sample = response.record.sample[i]
        state = State(entries=zip(response.variables, sample))
        # OpenJijでは `num_occurrences` で同じ状態のサンプルが複数出たことを表すが、OMMXではIDに変換する
        ids = []
        for _ in range(response.record.num_occurrences[i]):
            ids.append(sample_id)
            sample_id += 1
        samples.append(ids, state)

    return samples
```

IDの対応を考えなくて良いため、この段階では `ommx.Instance` やその情報を抽出した対応表などが必要ないことに注意してください。

### `ommx.adapter.SamplerAdapter` を継承したクラスの実装

PySCIPOptの時は `SolverAdapter` を継承しましたが、今回は `SamplerAdapter` を継承します。これは次のように3つの `@abstractmethod` を持っています。

```python
class SamplerAdapter(SolverAdapter):
    @classmethod
    @abstractmethod
    def sample_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> SampleSet:
        pass

    @property
    @abstractmethod
    def sampler_input(self) -> SamplerInput:
        pass

    @abstractmethod
    def decode_to_sampleset(self, data: SamplerOutput) -> SampleSet:
        pass
```

`SamplerAdapter`にも同じ二段階のAPIがあります。継承した`sample`は呼び出し元の
InstanceをcopyしてPrepareし、`sample_without_preparation`はexactなAdapter inputを受け取ります。
`SamplerAdapter.solve_without_preparation`は`sample_without_preparation`の`best_feasible`を選び、`solver_input`は
`sampler_input`へ委譲し、`decode`は`decode_to_sampleset(...).best_feasible`を返します。
追加optionがない場合、Sampler実装が定義するのはsampler側の`sample_without_preparation`、
`sampler_input`、`decode_to_sampleset`だけです。Adapter固有optionは公開する各APIで明示的に
型付けします。Preparationをまたいでも意味が変わらないoptionはeasy APIから明示的に転送できます。
exactなprepared inputに意味が依存し、Preparation過程をまたぐtransportを定義しない場合、具体的なSampler Adapterは
そのoptionを`sample_without_preparation`だけに公開できます。Solver APIでoptionを公開する場合も同様です。

`solve` と同様に、予約済みの `diagnostics` keyword は `Run.log_sample` が管理します。sink が `None` でない場合、sampler は adapter 固有の report を記録できます。

```{code-cell} ipython3
from ommx.adapter import DiagnosticsSink, SamplerAdapter

class OMMXOpenJijSAAdapter(SamplerAdapter):
    """
    Sampling QUBO with Simulated Annealing (SA) by `openjij.SASampler`
    """

    INPUT_CLASS = InstanceClass(
        [
            InstanceClassClause(
                label="tutorial-binary-qubo",
                allowed_variable_kinds={Kind.Binary},
                objective_polynomial_requirement=PolynomialRequirement.at_most(2),
                allowed_senses={Sense.Minimize},
            )
        ]
    )

    # SampleSetに変換する必要があるので、Instanceを保持
    ommx_instance: Instance
    
    def __init__(self, ommx_instance: Instance):
        self.require_applicable(ommx_instance)
        self.ommx_instance = ommx_instance

    # サンプリングを行う
    def _sample(self) -> oj.Response:
        sampler = oj.SASampler()
        # QUBOの辞書形式に変換
        # Applicability の成立後でも、QUBO 変換はここで失敗し得る。
        # これは converter error であり、applicability result ではない。
        qubo, _offset = self.ommx_instance.as_qubo_format()
        return sampler.sample_qubo(qubo)

    # サンプリングを行う共通のメソッド
    @classmethod
    def sample_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> SampleSet:
        _ = diagnostics
        adapter = cls(ommx_instance)
        response = adapter._sample()
        return adapter.decode_to_sampleset(response)
    
    # このAdapterでは `SamplerInput` は QUBO形式の辞書を使うことにする
    @property
    def sampler_input(self) -> dict[tuple[int, int], float]:
        qubo, _offset = self.ommx_instance.as_qubo_format()
        return qubo
   
    # OpenJijのResponseをSampleSetに変換
    def decode_to_sampleset(self, data: oj.Response) -> SampleSet:
        samples = decode_to_samples(data)
        # ここで `ommx.Instance` が保持している情報が必要になる
        return self.ommx_instance.evaluate_samples(samples)

```

### Sampler Adapterを使って簡単なサンプリングを行う

動作確認のため、これを使って次のQUBOからサンプリングを行ってみましょう

$$
\begin{aligned}
\min & \quad -x_0 - x_1 + 2 x_0 x_1 \\
& \quad x_0, x_1 \in \{0, 1\}
\end{aligned}
$$

```{code-cell} ipython3
x = [DecisionVariable.binary(id, name="x", subscripts=[id]) for id in range(2)]
instance = Instance.from_components(
    decision_variables=x,
    objective=-x[0] - x[1] + 2 * x[0] * x[1],
    constraints={},
    sense=Instance.MINIMIZE,
)

sample_set = OMMXOpenJijSAAdapter.sample(instance)
sample_set.summary
```

## まとめ

このチュートリアルでは、PySCIPOptと接続するSolver Adapterの実装とOpenJijと接続するSampler Adapterの実装を通して、OMMX Adapterの実装方法について学びました。以下がOMMX Adapterを実装する際の重要なポイントです：

1. OMMX Adapterは `SolverAdapter` または `SamplerAdapter` の抽象基底クラスを継承することで実装します
2. `INPUT_CLASS`でapplicabilityを定義し、preparation-freeな`solve_without_preparation()`または
   `sample_without_preparation()`を実装します。通常の`solve()`と`sample()`は呼び出し元のInstanceを
   copyし、`recommended_preparation_policy()`のfreshなPolicyを自動的に適用します。
   PreparationをcustomizeするapplicationはInstanceをPrepareしてpreparation-free APIを
   呼び出します
3. 実装の主なステップは以下の通りです：
   - `ommx.Instance` をバックエンドソルバーが理解できる形式に変換する
   - バックエンドソルバーを実行して解を取得する
   - バックエンドソルバーの出力を `ommx.Solution` や `ommx.SampleSet` に変換する
4. Converter 固有または backend 固有の validation は solver input の構築経路に置き、その失敗を applicability failure ではなく conversion または backend error として扱います
5. IDの管理や変数の対応付けなど、バックエンドソルバーとOMMXの橋渡しに注意を払う必要があります

独自のバックエンドソルバーをOMMXと接続したい場合は、このチュートリアルを参考に実装すると良いでしょう。このチュートリアルに従ってOMMX Adapterを実装することで、様々なバックエンドソルバーでの最適化を共通化されたAPIで利用できるようになります。

より詳しい実装例については、[ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter)や[ommx-openjij-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-openjij-adapter)などのリポジトリを参照してください。
