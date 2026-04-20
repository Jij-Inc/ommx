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

1. `ommx.v1.Instance` をバックエンドソルバーが読める形式に変換する
2. バックエンドソルバーを実行して解を取得する
3. バックエンドソルバーの解を `ommx.v1.Solution` や `ommx.v1.SampleSet` に変換して返す

2.はバックエンドソルバーの使い方そのものなので、これは既知としてこのチュートリアルでは扱いません。ここでは 1. と 3. をどのように実装するかを説明します。

多くのバックエンドソルバーが最適化問題を表すための必要な最小限の情報だけを、アルゴリズムに応じた形で受け取るように作られているのに比べて、`ommx.v1.Instance` はデータ分析の一部として最適化を行うことを想定しているためより多くの情報を持っています。なのでステップ 1. では多くの情報を削ぎ落とすことになります。またOMMXでは決定変数や制約条件は連番とは限らないIDで管理されていますが、バックエンドソルバーによっては名前で管理されいたり、連番で管理されていることがあります。この対応関係は 3. の処理で必要になるのでAdapterが管理しておく必要があります。

逆にステップ 3. では `ommx.v1.Solution` や `ommx.v1.SampleSet` はバックエンドソルバーの出力だけからは構築できないので、まずバックエンドソルバーの返した解と 1. の時の情報から `ommx.v1.State` あるいは `ommx.v1.Samples` を構築し、それを `ommx.v1.Instance` を使って `ommx.v1.Solution` や `ommx.v1.SampleSet` に変換します。

## Solver Adapterを実装する

ここでは PySCIPOpt を例としてSolver Adapterを実装してみましょう。なお完全な例は [ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter) を確認してください。

ここではチュートリアルということで、順番に実行しやすいように以下のように作業します。

- `ommx.v1.Instance` から PySCIPOpt のモデルを構築するための関数を順番に実装していきます。
- 最後にこれらの関数を `OMMXPySCIPOptAdapter` クラスとしてまとめます

### カスタム例外

まずカスタム例外を定義しておくといいでしょう。これによりユーザーは例外が発生したときに、どの部分が問題を引き起こしているのかを理解しやすくなります。

```{code-cell} ipython3
class OMMXPySCIPOptAdapterError(Exception):
    pass
```

OMMXは広いクラスの最適化問題を保存できるようになっているので、バックエンドソルバーが対応していない問題が入力されるケースがあります。その場合はエラーを投げるようにしてください。

### 決定変数を設定する

PySCIPOptは決定変数を名前で管理するので、OMMXの決定変数のIDを文字列にして名前として登録します。これにより後述する `decode_to_state` においてPySCIPOptの決定変数から `ommx.v1.State` を復元することができます。これはバックエンドソルバーの実装に応じて適切な方法が変わることに注意してください。重要なのは解を得た後に `ommx.v1.State` に変換するための情報を保持することです。

```{code-cell} ipython3
import pyscipopt
from ommx.v1 import Instance, Solution, DecisionVariable, Constraint, State, Function

def set_decision_variables(
    model: pyscipopt.Model,  # チュートリアルのために状態を引数で受け取っているがclassで管理するのが一般的
    instance: Instance
) -> dict[str, pyscipopt.Variable]:
    """
    モデルに決定変数を追加し、変数名のマッピングを作成して返す
    """
    # OMMXの決定変数の情報からPySCIPOptの変数を作成
    for var in instance.decision_variables:
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

### `ommx.v1.Function` を `pyscipopt.Expr` に変換する

`ommx.v1.Function` を `pyscipopt.Expr` に変換するための関数を実装します。`ommx.v1.Function` はOMMXの決定変数のIDしか持っていないので、IDからPySCIPOpt側の変数を取得する必要があり、そのために `set_decision_variables` で作成した変数名と変数のマッピングを使います。

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

以上で `ommx.v1.Instance` から `pycscipopt.Model` が構築できるようになりました。

### 得られた解を `ommx.v1.State` に変換する

次に、PySCIPOptのモデルを解いて得られた解を `ommx.v1.State` に変換する関数を実装します。まず解けているかを確認します。SCIPには最適性を保証する機能や解が非有界であることを検知する機能があるので、それらを検知していたら対応した例外を投げます。これもバックエンドソルバーに依存します。

```{warning}
特に `ommx.adapter.InfeasibleDetected` は解がInfeasibleではなくて最適化問題自体がInfeasible、つまり **一つも解を持ち得ないことが保証できた** という意味であることに注意してください。ヒューリスティックソルバーが一つも実行可能解を見つけられなかった場合にこれを使ってはいけません。
```

```{code-cell} ipython3
from ommx.adapter import InfeasibleDetected, UnboundedDetected

def decode_to_state(model: pyscipopt.Model, instance: Instance) -> State:
    """最適化済みのPySCIPOpt Modelからommx.v1.Stateを作成する"""
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
                for var in instance.decision_variables
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
class SolverAdapter(ABC):
    ADDITIONAL_CAPABILITIES: set[AdditionalCapability] = set()

    def __init__(self, ommx_instance: Instance):
        """制約の互換性をチェックする。サブクラスは super().__init__() を呼ぶ必要がある。"""
        ommx_instance.check_capabilities(self.ADDITIONAL_CAPABILITIES)

    @classmethod
    @abstractmethod
    def solve(cls, ommx_instance: Instance) -> Solution:
        pass

    @property
    @abstractmethod
    def solver_input(self) -> SolverInput:
        pass

    @abstractmethod
    def decode(self, data: SolverOutput) -> Solution:
        pass
```

この抽象基底クラスは以下の2通りのユースケースを想定しています: 

- バックエンドソルバーのパラメータなどを調整しない場合は、  `solve` クラスメソッドを使う。
- バックエンドソルバーのパラメータなどを調整する場合は、 `solver_input` を使ってバックエンドソルバーの入力用のデータ構造（今回は `pyscipopt.Model`）を取得し、調整した後にバックエンドソルバーへ入力し、最後にバックエンドソルバーの出力を `decode` で変換する。

#### 制約タイプの Capability 宣言

各アダプターは `ADDITIONAL_CAPABILITIES` クラス属性で、サポートする制約タイプを宣言する必要があります。基底クラスは `super().__init__()` の呼び出し時に、与えられた `Instance` がサポートされている制約タイプのみを使用していることを自動的にチェックします。利用可能な capability は以下の通りです：

- `AdditionalCapability.Indicator`: インジケーター制約 (`binvar = 1 → f(x) <= 0`)

`ADDITIONAL_CAPABILITIES` をオーバーライドしない場合、デフォルトでは通常の制約のみがサポートされます。`Instance` がサポートされていない制約タイプを含む場合、自動的にエラーが発生します。

```{important}
サブクラスは `__init__` メソッドで **必ず** `super().__init__(ommx_instance)` を呼び出してください。これにより、制約 capability の自動チェックが有効になります。
```

ここまでで用意した関数を使って次のように実装することができます：

```{code-cell} ipython3
from ommx.adapter import SolverAdapter
from ommx.v1 import AdditionalCapability

class OMMXPySCIPOptAdapter(SolverAdapter):
    # PySCIPOptは通常の制約とインジケーター制約の両方をサポート
    ADDITIONAL_CAPABILITIES = {AdditionalCapability.Indicator}

    def __init__(
        self,
        ommx_instance: Instance,
    ):
        super().__init__(ommx_instance)  # 制約 capability のチェック
        self.instance = ommx_instance
        self.model = pyscipopt.Model()
        self.model.hideOutput()

        # 関数を使用してモデルを構築
        self.varname_map = set_decision_variables(self.model, self.instance)
        set_objective(self.model, self.instance, self.varname_map)
        set_constraints(self.model, self.instance, self.varname_map)

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
    ) -> Solution:
        """
        PySCIPoptを使ってommx.v1.Instanceを解き、ommx.v1.Solutionを返す
        """
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
        最適化後のpyscipopt.ModelとOMMX Instanceからommx.v1.Solutionを生成する
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

        # 最適性ステータスを設定
        if data.getStatus() == "optimal":
            solution.optimality = Solution.OPTIMAL

        return solution
```

これでSolver Adapter完成です 🎉

```{note}
Pythonは継承したクラスでパラメータ引数を追加してもいいので、次のように追加のパラメータを定義することもできます。ただし、これによってバックエンドソルバーの様々な機能が使えるようになる一方、他のAdapterとの互換性が損なわれるので、Adapterを作る際には慎重に検討してください。

```python
    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        timeout: Optional[int] = None,
    ) -> Solution:
```

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

### `openjij.Response` から `ommx.v1.Samples` への変換

OpenJijは決定変数をOMMXと同様に連番とは限らないIDで管理しているので、PySCIPOptの時のようにIDの対応表を作る必要はありません。

OpenJijのサンプル結果は `openjij.Response` として得られるので、これを `ommx.v1.Samples` に変換する関数を実装します。OpenJijは同じサンプルが得られた時、それが発生した回数を `num_occurrence` として返します。一方 `ommx.v1.Samples` は個々のサンプルが固有のサンプルIDをもち、同じ値を持つサンプルは `SamplesEntry` として圧縮されます。この差異を埋めるための変換が必要なことに注意します。

```{code-cell} ipython3
import openjij as oj
from ommx.v1 import Instance, SampleSet, Solution, Samples, State

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

IDの対応を考えなくて良いため、この段階では `ommx.v1.Instance` やその情報を抽出した対応表などが必要ないことに注意してください。

### `ommx.adapter.SamplerAdapter` を継承したクラスの実装

PySCIPOptの時は `SolverAdapter` を継承しましたが、今回は `SamplerAdapter` を継承します。これは次のように3つの `@abstractmethod` を持っています。

```python
class SamplerAdapter(SolverAdapter):
    @classmethod
    @abstractmethod
    def sample(cls, ommx_instance: Instance) -> SampleSet:
        pass

    @property
    @abstractmethod
    def sampler_input(self) -> SamplerInput:
        pass

    @abstractmethod
    def decode_to_sampleset(self, data: SamplerOutput) -> SampleSet:
        pass
```

`SamplerAdapter` は `SolverAdapter` を継承しているので `solve` などの `@abstractmethod` も実装する必要と思うかもしれません。しかし、これらについては `sample` を使って最善のサンプルを返すという機能が `SamplerAdapter` に実装されているため、`sample` だけを実装すれば十分です。自分でより効率の良い実装を行いたい場合は `solve` をオーバーライドしてください。

```{code-cell} ipython3
from ommx.adapter import SamplerAdapter

class OMMXOpenJijSAAdapter(SamplerAdapter):
    """
    Sampling QUBO with Simulated Annealing (SA) by `openjij.SASampler`
    """

    # SampleSetに変換する必要があるので、Instanceを保持
    ommx_instance: Instance
    
    def __init__(self, ommx_instance: Instance):
        super().__init__(ommx_instance)  # 制約 capability のチェック
        self.ommx_instance = ommx_instance

    # サンプリングを行う
    def _sample(self) -> oj.Response:
        sampler = oj.SASampler()
        # QUBOの辞書形式に変換
        # InstanceがQUBO形式でなければここでエラーになる
        qubo, _offset = self.ommx_instance.to_qubo()
        return sampler.sample_qubo(qubo)

    # サンプリングを行う共通のメソッド
    @classmethod
    def sample(cls, ommx_instance: Instance) -> SampleSet:
        adapter = cls(ommx_instance)
        response = adapter._sample()
        return adapter.decode_to_sampleset(response)
    
    # このAdapterでは `SamplerInput` は QUBO形式の辞書を使うことにする
    @property
    def sampler_input(self) -> dict[tuple[int, int], float]:
        qubo, _offset = self.ommx_instance.to_qubo()
        return qubo
   
    # OpenJijのResponseをSampleSetに変換
    def decode_to_sampleset(self, data: oj.Response) -> SampleSet:
        samples = decode_to_samples(data)
        # ここで `ommx.v1.Instance` が保持している情報が必要になる
        return self.ommx_instance.evaluate_samples(samples)

    # SamplerAdapterはSolverAdapterとしても使えるようにするため、必要なAPIを実装します
    @property
    def solver_input(self) -> dict[tuple[int, ...], float]:
        return self.sampler_input

    # ここではサンプル結果のうち最良の実行可能な解を返すようにする
    def decode(self, data: oj.Response) -> Solution:
        sample_set = self.decode_to_sampleset(data)
        return sample_set.best_feasible

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
    ) -> Solution:
        sample_set = cls.sample( ommx_instance,)
        return sample_set.best_feasible
```

### Sampler Adapterを使って簡単なサンプリングを行う

動作確認のため、これを使って次の最適化問題からサンプリングを行ってみましょう

$$
\begin{aligned}
\max & \quad x_0 + x_1 \\
\text{s.t.} & \quad x_0 \cdot x_1 = 1 \\
& \quad x_0, x_1 \in \{0, 1\}
\end{aligned}
$$

```{code-cell} ipython3
x = [DecisionVariable.binary(id, name="x", subscripts=[id]) for id in range(2)]
instance = Instance.from_components(
    decision_variables=x,
    objective=x[0] + x[1],
    constraints={0: x[0] * x[1] == 1},
    sense=Instance.MAXIMIZE,
)

sample_set = OMMXOpenJijSAAdapter.sample(instance)
sample_set.summary
```

## まとめ

このチュートリアルでは、PySCIPOptと接続するSolver Adapterの実装とOpenJijと接続するSampler Adapterの実装を通して、OMMX Adapterの実装方法について学びました。以下がOMMX Adapterを実装する際の重要なポイントです：

1. OMMX Adapterは `SolverAdapter` または `SamplerAdapter` の抽象基底クラスを継承することで実装します
2. `ADDITIONAL_CAPABILITIES` でサポートする制約タイプを宣言し、`super().__init__()` を呼び出して自動的な capability チェックを有効にします
3. 実装の主なステップは以下の通りです：
   - `ommx.v1.Instance` をバックエンドソルバーが理解できる形式に変換する
   - バックエンドソルバーを実行して解を取得する
   - バックエンドソルバーの出力を `ommx.v1.Solution` や `ommx.v1.SampleSet` に変換する
4. 各バックエンドソルバーの特性や制限を理解し、適切に処理する必要があります
4. IDの管理や変数の対応付けなど、バックエンドソルバーとOMMXの橋渡しに注意を払う必要があります

独自のバックエンドソルバーをOMMXと接続したい場合は、このチュートリアルを参考に実装すると良いでしょう。このチュートリアルに従ってOMMX Adapterを実装することで、様々なバックエンドソルバーでの最適化を共通化されたAPIで利用できるようになります。

より詳しい実装例については、[ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter)や[ommx-openjij-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-openjij-adapter)などのリポジトリを参照してください。
