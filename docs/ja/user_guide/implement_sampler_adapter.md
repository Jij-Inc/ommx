# Sampler Adapterを実装する

[複数のAdapterで最適化問題を解いて結果を比較する](../tutorial/switching_adapters)で触れた通り、OMMX Python SDKにはAdapterを実装するための抽象基底クラスが用意されており、これを継承する事で共通の仕様に沿ったAdapterを実装する事ができます。OMMXはAdapterの性質に応じて二つの抽象基底クラスを用意しています。

- [`ommx.adapter.SolverAdapter`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/adapter/index.html#ommx.adapter.SolverAdapter): 一つの解を返す最適化ソルバーのための抽象基底クラス
- [`ommx.adapter.SamplerAdapter`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/adapter/index.html#ommx.adapter.SamplerAdapter): サンプリングベースの最適化ソルバーのための抽象基底クラス

[OMMX Adapterを実装する](../tutorial/implement_solver_adapter)では`SolverAdapter`の実装方法について説明しました。このページでは`SamplerAdapter`の実装方法について、OpenJijのSimulated Annealingを例に説明します。

## Sampler Adapterの処理の流れ

Sampler Adapterの処理は大雑把にいうと次の3ステップからなります：

1. `ommx.v1.Instance` をバックエンドソルバーが読める形式に変換する
2. バックエンドソルバーを実行して複数のサンプルを取得する
3. バックエンドソルバーの出力を `ommx.v1.Samples` および `ommx.v1.SampleSet` に変換して返す

ここでは、OpenJijのシミュレーテッドアニーリング（SA）を使ってサンプル生成を行うAdapterを順番に実装していきます。

## OpenJijからSamplesへの変換

まず必要なモジュールをインポートします：

```python markdown-code-runner
from __future__ import annotations
import openjij as oj
from ommx.v1 import Instance, SampleSet, Solution, Samples, State
from ommx.adapter import SamplerAdapter
```

バックエンドソルバーの出力を`Samples`形式に変換する関数から実装しましょう：

```python markdown-code-runner
def decode_to_samples(response: oj.Response) -> Samples:
    """
    OpenJijのResponseをommx.v1.Samplesに変換する
    """
    # サンプルIDを生成
    sample_id = 0
    entries = []

    num_reads = len(response.record.num_occurrences)
    for i in range(num_reads):
        sample = response.record.sample[i]
        state = State(entries=zip(response.variables, sample))
        # `num_occurrences`をサンプルIDリストにエンコード
        ids = []
        for _ in range(response.record.num_occurrences[i]):
            ids.append(sample_id)
            sample_id += 1
        entries.append(Samples.SamplesEntry(state=state, ids=ids))
    return Samples(entries=entries)
```

## OpenJijでのサンプリング

次に、OpenJijを使ってサンプリングを行う関数を実装します：

```python markdown-code-runner
def sample_with_openjij_sa(
    ommx_instance: Instance,
    beta_min: float | None = None,
    beta_max: float | None = None,
    num_sweeps: int | None = None,
    num_reads: int | None = None,
    schedule: list | None = None,
    initial_state: list | dict | None = None,
    updater: str | None = None,
    sparse: bool | None = None,
    reinitialize_state: bool | None = None,
    seed: int | None = None,
) -> oj.Response:
    """
    OpenJijのSASamplerを使ったサンプリング
    """
    sampler = oj.SASampler()
    qubo, _offset = ommx_instance.as_qubo_format()
    return sampler.sample_qubo(
        qubo,
        beta_min=beta_min,
        beta_max=beta_max,
        num_sweeps=num_sweeps,
        num_reads=num_reads,
        schedule=schedule,
        initial_state=initial_state,
        updater=updater,
        sparse=sparse,
        reinitialize_state=reinitialize_state,
        seed=seed,
    )
```

## SamplesからSampleSetへの変換

サンプル結果を評価してSampleSetに変換する関数も実装します：

```python markdown-code-runner
def evaluate_to_sampleset(
    ommx_instance: Instance, 
    samples: Samples
) -> SampleSet:
    """
    Samplesを評価してSampleSetに変換
    """
    return ommx_instance.evaluate_samples(samples)
```

## OpenJijを使った完全なサンプリングフロー

上の関数を組み合わせて、完全なサンプリングフローを実装します：

```python markdown-code-runner
def sample_qubo_with_openjij_sa(
    ommx_instance: Instance,
    beta_min: float | None = None,
    beta_max: float | None = None,
    num_sweeps: int | None = None,
    num_reads: int | None = None,
    schedule: list | None = None,
    initial_state: list | dict | None = None,
    updater: str | None = None,
    sparse: bool | None = None,
    reinitialize_state: bool | None = None,
    seed: int | None = None,
) -> SampleSet:
    """
    OpenJijのSASamplerを使って問題を解き、SampleSetを返す
    """
    # OpenJijでサンプリング
    response = sample_with_openjij_sa(
        ommx_instance,
        beta_min=beta_min,
        beta_max=beta_max,
        num_sweeps=num_sweeps,
        num_reads=num_reads,
        schedule=schedule,
        initial_state=initial_state,
        updater=updater,
        sparse=sparse,
        reinitialize_state=reinitialize_state,
        seed=seed,
    )
    
    # ResponseをSamplesに変換
    samples = decode_to_samples(response)
    
    # SamplesをSampleSetに変換
    return evaluate_to_sampleset(ommx_instance, samples)
```

## 使用例

実装した関数を使って、シンプルな問題を解いてみましょう：

```python markdown-code-runner
from ommx.v1 import DecisionVariable, Instance

# 問題をQUBOとして定義
n = 5
x = [DecisionVariable.binary(id=i) for i in range(n)]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(x[i] * x[i+1] for i in range(n-1)),
    sense=Instance.MINIMIZE,
)

# サンプリング実行
sample_set = sample_qubo_with_openjij_sa(
    instance,
    num_reads=10,
    num_sweeps=100,
)

# 結果の分析
print(f"Number of samples: {len(sample_set.samples)}")
print(f"Best energy: {sample_set.first.evaluate.objective_value}")
```

## SamplerAdapterクラスとしての実装

最後に、これまで実装した関数をまとめて`SamplerAdapter`を継承したクラスを作成します：

```python markdown-code-runner
class OMMXOpenJijSAAdapter(SamplerAdapter):
    """
    Sampling QUBO with Simulated Annealing (SA) by `openjij.SASampler`
    """
    
    def __init__(
        self,
        ommx_instance: Instance,
        *,
        beta_min: float | None = None,
        beta_max: float | None = None,
        num_sweeps: int | None = None,
        num_reads: int | None = None,
        schedule: list | None = None,
        initial_state: list | dict | None = None,
        updater: str | None = None,
        sparse: bool | None = None,
        reinitialize_state: bool | None = None,
        seed: int | None = None,
    ):
        self.ommx_instance = ommx_instance
        self.beta_min = beta_min
        self.beta_max = beta_max
        self.num_sweeps = num_sweeps
        self.num_reads = num_reads
        self.schedule = schedule
        self.initial_state = initial_state
        self.updater = updater
        self.sparse = sparse
        self.reinitialize_state = reinitialize_state
        self.seed = seed

    @classmethod
    def sample(
        cls,
        ommx_instance: Instance,
        *,
        beta_min: float | None = None,
        beta_max: float | None = None,
        num_sweeps: int | None = None,
        num_reads: int | None = None,
        schedule: list | None = None,
        initial_state: list | dict | None = None,
        updater: str | None = None,
        sparse: bool | None = None,
        reinitialize_state: bool | None = None,
        seed: int | None = None,
    ) -> SampleSet:
        """
        クラスメソッドでサンプリングを実行
        """
        adapter = cls(
            ommx_instance,
            beta_min=beta_min,
            beta_max=beta_max,
            num_sweeps=num_sweeps,
            num_reads=num_reads,
            schedule=schedule,
            initial_state=initial_state,
            updater=updater,
            sparse=sparse,
            reinitialize_state=reinitialize_state,
            seed=seed,
        )
        response = adapter._sample()
        return adapter.decode_to_sampleset(response)
    
    @property
    def sampler_input(self) -> dict[tuple[int, int], float]:
        """バックエンドソルバー入力形式を返す"""
        qubo, _offset = self.ommx_instance.as_qubo_format()
        return qubo
    
    def _sample(self) -> oj.Response:
        """実際のサンプリングを実行"""
        return sample_with_openjij_sa(
            self.ommx_instance,
            beta_min=self.beta_min,
            beta_max=self.beta_max,
            num_sweeps=self.num_sweeps,
            num_reads=self.num_reads,
            schedule=self.schedule,
            initial_state=self.initial_state,
            updater=self.updater,
            sparse=self.sparse,
            reinitialize_state=self.reinitialize_state,
            seed=self.seed,
        )
    
    def decode_to_sampleset(self, data: oj.Response) -> SampleSet:
        """OpenJijのResponseをSampleSetに変換"""
        samples = decode_to_samples(data)
        return evaluate_to_sampleset(self.ommx_instance, samples)
    
    def decode_to_samples(self, data: oj.Response) -> Samples:
        """OpenJijのResponseをSamplesに変換"""
        return decode_to_samples(data)
```

クラスを使った実行例：

```python markdown-code-runner
# クラスを使ったサンプリング実行
sample_set2 = OMMXOpenJijSAAdapter.sample(
    instance,
    num_reads=10,
    num_sweeps=100,
)

print(f"Number of samples using class: {len(sample_set2.samples)}")
print(f"Best energy using class: {sample_set2.first.evaluate.objective_value}")
```

## まとめ

このチュートリアルでは、Sampler Adapterを実装する方法を学びました。主なステップは以下の通りです：

1. バックエンドソルバーの出力を`ommx.v1.Samples`に変換する関数の実装
2. バックエンドソルバーを使ってサンプリングを行う関数の実装
3. サンプルを評価して`ommx.v1.SampleSet`に変換する関数の実装
4. 上記をまとめた完全なサンプリングフローの実装
5. `SamplerAdapter`を継承したクラスの実装

これらのステップに従うことで、任意のサンプリングベースのソルバーに対応するOMMX Adapterを実装できます。Adapterを使うことで、OMMX Python SDKのエコシステムに様々なソルバーを統合することができます。
