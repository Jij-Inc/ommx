from ommx.v1 import Instance, Solution, State
import openjij as oj
from dataclasses import dataclass


@dataclass
class SampleSet:
    samples: list[Solution]


def sample_qubo(instance: Instance, *, num_reads: int = 1) -> SampleSet:
    q, c = instance.as_qubo_format()
    sampler = oj.SASampler()
    response = sampler.sample_qubo(q, num_reads=num_reads)  # type: ignore
    states = [State(entries=sample) for sample in response.samples()]
    return SampleSet(samples=[instance.evaluate(state) for state in states])
