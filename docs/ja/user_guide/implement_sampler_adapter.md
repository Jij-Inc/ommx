# Sampler Adapterを実装する

ここでは OpenJij を例としてSampler Adapterを実装してみましょう。SamplerAdapterは複数のサンプルを返すソルバーのためのAdapterです。基本的なアプローチはSolverAdapterと同様ですが、複数のサンプルを扱う方法が異なります。

```python markdown-code-runner
from __future__ import annotations
import openjij as oj
import numpy as np

from ommx.adapter import SamplerAdapter
from ommx.v1 import Instance, SampleSet, Solution, Samples
from ommx.v1.solution_pb2 import State, Optimality

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
