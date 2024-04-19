from ommx.v1._test_generator import LPTestDataType, LPTestGenerator
from ommx.v1.instance_pb2 import Instance
from ommx.v1.solution_pb2 import SolutionList


def test_generator():
    generator = LPTestGenerator(3, LPTestDataType.INT)
    ommx_instance_byte = generator.get_instance()
    ommx_solution_byte = generator.get_solution()

    Instance().ParseFromString(ommx_instance_byte)
    SolutionList().ParseFromString(ommx_solution_byte)
