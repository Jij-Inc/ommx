# Sampler Adapterを実装する

[OMMX Adapterを実装する](../tutorial/implement_solver_adapter)では、PySCIPOptを使った Solver Adapterを実装する方法について説明しました。このページでは、OpenJijを使った Sampler Adapter を実装する方法について説明します。また OpenJij Adapterの使い方については [OMMX AdapterでQUBOからサンプリングする](../tutorial/tsp_sampling_with_openjij_adapter) を参照してください。

```{note}
OpenJijには Simulated Annealing (SA) による [`openjij.SASampler`](https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SASampler)と Simulated Quantum Annealing (SQA) による [`openjij.SQASampler`](https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SQASampler) が含まれています。このチュートリアルでは、 `SASampler` を使ったサンプリングを例に説明します。
```

このチュートリアルでは簡単のためにOpenJijに渡すパラメータは省略しています。詳しくは [`ommx-openjij-adapter`](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-openjij-adapter) の実装を参照してください。

## `openjij.Response` から `ommx.v1.Samples` への変換

OpenJijは決定変数をOMMXと同様に連番とは限らないIDで管理しているので、PySCIPOptの時のようにIDの対応表を作る必要はありません。

OpenJijのサンプル結果は `openjij.Response` として得られるので、これを `ommx.v1.Samples` に変換する関数を実装します。OpenJijは同じサンプルが得られた時、それが発生した回数を `num_occurrence` として返します。一方 `ommx.v1.Samples` はここのサンプルが固有のサンプルIDをもち、同じ値を持つサンプルは `SamplesEntry` として圧縮されます。この変換を行う必要があります。

```python markdown-code-runner
import openjij as oj
from ommx.v1 import Instance, SampleSet, Solution, Samples, State

def decode_to_samples(response: oj.Response) -> Samples:
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

IDの対応を考えなくていいため、この段階では `ommx.v1.Instance` やその情報を抽出した対応表などが必要ないことに注意してください。

## `ommx.adapter.SamplerAdapter` を継承したクラスの実装

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

加えて `SolverAdapter` を継承しているので `solve` などの `@abstractmethod` も実装する必要と思うかもしれませんが、これらについては `sample` を使って、最善のサンプルを返すというデフォルト実装が提供されているため、`sample` だけを実装すればよいです。あるいは自分でより効率の良い実装を行いたい場合は `solve` をオーバーライドしてください。

```python markdown-code-runner
from ommx.adapter import SamplerAdapter

class OMMXOpenJijSAAdapter(SamplerAdapter):
    """
    Sampling QUBO with Simulated Annealing (SA) by `openjij.SASampler`
    """

    # SampleSetに変換する必要があるので、Instanceを保持
    ommx_instance: Instance
    
    def __init__(self, ommx_instance: Instance):
        self.ommx_instance = ommx_instance

    # サンプリングを行う
    def _sample(self) -> oj.Response:
        sampler = oj.SASampler()
        # QUBOの辞書形式に変換
        # InstanceがQUBO形式でなければここでエラーになる
        qubo, _offset = self.ommx_instance.as_qubo_format()
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
        qubo, _offset = self.ommx_instance.as_qubo_format()
        return qubo
   
    # OpenJijのResponseをSampleSetに変換
    def decode_to_sampleset(self, data: oj.Response) -> SampleSet:
        samples = decode_to_samples(data)
        # ここで `ommx.v1.Instance` が保持している情報が必要になる
        return self.ommx_instance.evaluate_samples(samples)
```
