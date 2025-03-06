# OMMX Adapterを実装する

[複数のAdapterで最適化問題を解いて結果を比較する](../tutorial/switching_adapters)で触れた通り、OMMX Python SDKにはAdapterを実装するための抽象基底クラスが用意されており、これを継承する事で共通の仕様に沿ったAdapterを実装する事ができます。OMMXはAdapterの性質に応じて二つの抽象基底クラスを用意しています。

- [`ommx.adapter.SolverAdapter`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/adapter/index.html#ommx.adapter.SolverAdapter): 一つの解を返す最適化ソルバーのための抽象基底クラス
- [`ommx.adapter.SamplerAdapter`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/adapter/index.html#ommx.adapter.SamplerAdapter): サンプリングベースの最適化ソルバーのための抽象基底クラス

複数の解が得られるソルバーは、特に複数得られたサンプルのうち最善のものを選択することによって、自動的に単一の解を返すソルバーと見なす事ができるため、`SamplerAdapter` は `SolverAdapter` を継承しています。Adapterを作るときにどちらを実装するか悩んだら、出力される解の数を見て、一つの解を返すなら `SolverAdapter`、複数の解を返すなら `SamplerAdapter` を継承すると良いでしょう。たとえば [PySCIPOpt](https://github.com/scipopt/PySCIPOpt) などの数理最適化ソルバーは一つの解を返すため `SolverAdapter` を使い、[OpenJij](https://github.com/OpenJij/OpenJij) などのサンプラーは複数の解を返すため、`SamplerAdapter` を使います。

OMMXでは `ommx.adapter.SolverAdapter` を継承したクラスを **Solver Adapter**、`ommx.adapter.SamplerAdapter` を継承したクラスを **Sampler Adapter** と呼びます。
またここでの説明のため、PySCIPOptやOpenJijのようにAdapterがラップしようとしているソフトウェアのことをバックエンドソルバーと呼びます。

## Adapterの処理の流れ

Adapterの処理は大雑把にいうと次の3ステップからなります：

1. `ommx.v1.Instance` をバックエンドソルバーが読める形式に変換する
2. バックエンドソルバーを実行して解を取得する
3. バックエンドソルバーの解を `ommx.v1.Solution` や `ommx.v1.SampleSet` に変換して返す

2.はバックエンドソルバーの使い方そのものなので、これは既知としてこのチュートリアルでは扱いません。ここでは 1. と 3. をどのように実装するかを説明します。

多くのバックエンドソルバーが数学的な数理最適化問題を表すための必要な最小限の情報だけを、アルゴリズムに応じた形で受け取るように作られているのに比べて、`ommx.v1.Instance` はデータ分析の一部として数理最適化を行うことを想定しているためより多くの情報を持っています。なのでステップ 1. では多くの情報を削ぎ落とすことになります。またOMMXでは決定変数や制約条件は連番とは限らないIDで管理されていますが、バックエンドソルバーによっては名前で管理されいたり、連番で管理されていることがあります。この対応関係は 3. の処理で必要になるのでAdapterが管理しておく必要があります。

逆にステップ 3. では `ommx.v1.Solution` や `ommx.v1.SampleSet` はバックエンドソルバーの出力だけからは構築できないので、まずバックエンドソルバーの返した解と 1. の時の情報から `ommx.v1.State` あるいは `ommx.v1.Samples` を構築し、それを `ommx.v1.Instance` を使って `ommx.v1.Solution` や `ommx.v1.SampleSet` に変換します。

## Solver Adapterを実装する

ここでは PySCIPOpt を例としてSolver Adapterを実装してみましょう。なお完全な例は [ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter) を確認してください。

まず、必要なモジュールをインポートします。

```python
from __future__ import annotations
from typing import Literal

import pyscipopt
import math

from ommx.adapter import SolverAdapter, InfeasibleDetected, UnboundedDetected
from ommx.v1 import Instance, Solution, DecisionVariable, Constraint
from ommx.v1.function_pb2 import Function
from ommx.v1.solution_pb2 import State, Optimality

# カスタム例外クラス
class OMMXPySCIPOptAdapterError(Exception):
    pass
```

次に、PySCIPOptのモデルを構築するための関数を順番に実装していきます。後で`OMMXPySCIPOptAdapter`クラスでこれらの関数を使うことになります。

### 決定変数を設定する関数

```python
def set_decision_variables(model: pyscipopt.Model, instance: Instance):
    """
    モデルに決定変数を追加し、変数名のマッピングを作成して返す
    """
    varname_map = {}
    
    # 通常の決定変数を追加
    for var in instance.raw.decision_variables:
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
            raise OMMXPySCIPOptAdapterError(
                f"Unsupported decision variable kind: "
                f"id: {var.id}, kind: {var.kind}"
            )

    # 目的関数が2次の場合、線形化のために補助変数を追加
    if instance.raw.objective.HasField("quadratic"):
        model.addVar(
            name="auxiliary_for_linearized_objective", vtype="C", lb=None, ub=None
        )

    # モデルに追加された変数へアクセスするための辞書を作成
    varname_map = {var.name: var for var in model.getVars()}
    return varname_map
```

### 式を生成するヘルパー関数

```python
def make_linear_expr(function: Function, varname_map: dict) -> pyscipopt.Expr:
    """線形式を生成するヘルパー関数"""
    linear = function.linear
    return (
        pyscipopt.quicksum(
            term.coefficient * varname_map[str(term.id)]
            for term in linear.terms
        )
        + linear.constant
    )

def make_quadratic_expr(function: Function, varname_map: dict) -> pyscipopt.Expr:
    """2次式を生成するヘルパー関数"""
    quad = function.quadratic
    quad_terms = pyscipopt.quicksum(
        varname_map[str(row)] * varname_map[str(column)] * value
        for row, column, value in zip(quad.rows, quad.columns, quad.values)
    )

    linear_terms = pyscipopt.quicksum(
        term.coefficient * varname_map[str(term.id)]
        for term in quad.linear.terms
    )

    constant = quad.linear.constant

    return quad_terms + linear_terms + constant
```

### 目的関数と制約条件を設定する関数

```python
def set_objective(model: pyscipopt.Model, instance: Instance, varname_map: dict):
    """モデルに目的関数を設定"""
    objective = instance.raw.objective

    if instance.sense == Instance.MAXIMIZE:
        sense = "maximize"
    elif instance.sense == Instance.MINIMIZE:
        sense = "minimize"
    else:
        raise OMMXPySCIPOptAdapterError(
            f"Sense not supported: {instance.sense}"
        )

    if objective.HasField("constant"):
        model.setObjective(objective.constant, sense=sense)
    elif objective.HasField("linear"):
        expr = make_linear_expr(objective, varname_map)
        model.setObjective(expr, sense=sense)
    elif objective.HasField("quadratic"):
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
    # 通常の制約条件を処理
    for constraint in instance.raw.constraints:
        # 制約関数の種類に基づいて式を生成
        if constraint.function.HasField("linear"):
            expr = make_linear_expr(constraint.function, varname_map)
        elif constraint.function.HasField("quadratic"):
            expr = make_quadratic_expr(constraint.function, varname_map)
        elif constraint.function.HasField("constant"):
            # 定数制約の場合、実行可能かどうかをチェック
            if constraint.equality == Constraint.EQUAL_TO_ZERO and math.isclose(
                constraint.function.constant, 0, abs_tol=1e-6
            ):
                continue
            elif (
                constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
                and constraint.function.constant <= 1e-6
            ):
                continue
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Infeasible constant constraint was found: id {constraint.id}"
                )
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Constraints must be either `constant`, `linear` or `quadratic`."
                f"id: {constraint.id}, "
                f"type: {constraint.function.WhichOneof('function')}"
            )

        # 制約種別（等式/不等式）に基づいて制約を追加
        if constraint.equality == Constraint.EQUAL_TO_ZERO:
            constr_expr = expr == 0
        elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
            constr_expr = expr <= 0
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Not supported constraint equality: "
                f"id: {constraint.id}, equality: {constraint.equality}"
            )

        # 制約をモデルに追加
        model.addCons(constr_expr, name=str(constraint.id))
```

### 解の変換関数

```python
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
                for var in instance.raw.decision_variables
            }
        )
    except Exception:
        raise OMMXPySCIPOptAdapterError(
            f"There is no feasible solution. [status: {model.getStatus()}]"
        )
```

### `OMMXPySCIPOptAdapter` クラスの実装

最後に、上記の関数を使用するAdapterクラスを実装します：

```python
class OMMXPySCIPOptAdapter(SolverAdapter):
    def __init__(
        self,
        ommx_instance: Instance,
    ):
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
            solution.raw.optimality = Optimality.OPTIMALITY_OPTIMAL

        return solution
```

## Sampler Adapterを実装する

ここでは OpenJij を例としてSampler Adapterを実装してみましょう。SamplerAdapterは複数のサンプルを返すソルバーのためのAdapterです。基本的なアプローチはSolverAdapterと同様ですが、複数のサンプルを扱う方法が異なります。

```python
from __future__ import annotations
import openjij as oj
import numpy as np

from ommx.adapter import SamplerAdapter
from ommx.v1 import Instance, SampleSet, Solution
from ommx.v1.solution_pb2 import Samples, Sample, State, Optimality

class OMMXOpenJijAdapter(SamplerAdapter):
    def __init__(self, ommx_instance: Instance):
        self.instance = ommx_instance
        # IDと変数インデックスのマッピングを作成
        self.var_indices = {var.id: i for i, var in enumerate(self.instance.raw.decision_variables)}
        # モデル変換処理を実行
        self.model = self._convert_to_model()
        
    def _convert_to_model(self):
        """
        OMMX InstanceからOpenJijのモデルに変換
        """
        # OpenJijは主にQuadratic Unconstrained Binary Optimization (QUBO)またはIsingモデルで定義
        # ここでは例としてQUBOフォーマットへの変換を示す
        objective = self.instance.raw.objective
        num_vars = len(self.var_indices)
        Q = np.zeros((num_vars, num_vars))
        
        if objective.HasField("quadratic"):
            quad = objective.quadratic
            for row, col, val in zip(quad.rows, quad.columns, quad.values):
                row_idx = self.var_indices[row]
                col_idx = self.var_indices[col]
                Q[row_idx, col_idx] += val
                
            # 線形項も含める
            for term in quad.linear.terms:
                var_idx = self.var_indices[term.id]
                Q[var_idx, var_idx] += term.coefficient
        
        elif objective.HasField("linear"):
            # 線形項はQUBOの対角要素
            for term in objective.linear.terms:
                var_idx = self.var_indices[term.id]
                Q[var_idx, var_idx] += term.coefficient
                
        # 制約条件はペナルティ項として目的関数に追加する必要があります
        # （この実装は簡略化されています）
        
        return Q
    
    @classmethod
    def sample(cls, ommx_instance: Instance, num_reads: int = 100) -> SampleSet:
        """
        OpenJijを使用してサンプリングを実行し、SampleSetを返す
        """
        adapter = cls(ommx_instance)
        sampler = oj.SQASampler()
        response = sampler.sample_qubo(adapter.model, num_reads=num_reads)
        return adapter.decode(response)
    
    def decode(self, data) -> SampleSet:
        """
        OpenJijのサンプリング結果をOMMX SampleSetに変換
        """
        samples = []
        # OpenJijのレスポンスから各サンプルを取得
        for i, sample_array in enumerate(data.record):
            # サンプル値をStateフォーマットに変換
            state_entries = {}
            for var_id, idx in self.var_indices.items():
                state_entries[var_id] = float(sample_array[0][idx])
            
            state = State(entries=state_entries)
            # エネルギー値をOpenJijから取得
            energy = data.energies[i]
            # サンプル数を取得
            num_occurrences = data.num_occurrences[i]
            
            # Sampleオブジェクトを作成
            sample = Sample(
                state=state,
                energy=energy,
                num_occurrences=num_occurrences
            )
            samples.append(sample)
        
        # SampleSetを作成
        sample_set = SampleSet(samples=Samples(samples=samples))
        
        # インスタンスを使って各サンプルを評価
        for sample in sample_set.raw.samples.samples:
            solution = self.instance.evaluate(sample.state)
            sample.evaluated = solution.raw
        
        return sample_set
    
    @property
    def solver_input(self):
        """モデルを返す"""
        return self.model
        
    def decode_to_solution(self, data) -> Solution:
        """
        SamplerAdapterの実装として、最良のサンプルからSolutionを生成
        """
        sample_set = self.decode(data)
        # 最良のサンプルを選択（エネルギー最小化）
        best_sample = min(sample_set.raw.samples.samples, key=lambda s: s.energy)
        # インスタンスを使って解を評価
        solution = self.instance.evaluate(best_sample.state)
        solution.raw.optimality = Optimality.OPTIMALITY_UNKNOWN
        return solution
```

## まとめ

このチュートリアルでは、OMMXのAdapterを実装する方法を学びました。

1. `SolverAdapter` または `SamplerAdapter` を継承したクラスを作成
2. バックエンドソルバーの要件に合わせて `ommx.v1.Instance` からモデルを構築
3. 変数のIDとソルバー内の表現とのマッピングを保持
4. ソルバーを実行して結果を取得
5. ソルバーの結果を `ommx.v1.State` または `ommx.v1.Samples` に変換
6. 最終的に `ommx.v1.Solution` または `ommx.v1.SampleSet` を返す

これらの手順に従って、任意のバックエンドソルバーに対応するOMMX Adapterを実装することができます。Adapterを実装することで、様々な最適化ソルバー間で最適化問題の定式化と解の評価を統一的に扱うことができます。
