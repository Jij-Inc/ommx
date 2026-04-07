"""Tests for NamedFunction wrapper and its integration with Instance, Solution, and SampleSet."""

from ommx.v1 import (
    DecisionVariable,
    Function,
    Instance,
    Linear,
    NamedFunction,
    Constraint,
    State,
)
import ommx._ommx_rust as rust


def _make_instance_with_named_functions():
    """Helper: create an instance with decision variables and named functions."""
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
    objective = x[0] + 2 * x[1] + 3 * x[2]

    nf0 = NamedFunction(
        id=0,
        function=x[0] + x[1],
        name="sum_01",
        subscripts=[0],
        description="sum of x0 and x1",
    )
    nf1 = NamedFunction(
        id=1,
        function=x[1] + x[2],
        name="sum_12",
        subscripts=[1],
    )

    instance = Instance.from_components(
        decision_variables=x,
        objective=objective,
        constraints=[Function(sum(x)) <= 2],
        sense=Instance.MAXIMIZE,
        named_functions=[nf0, nf1],
    )
    return instance, x


class TestNamedFunctionConstruction:
    def test_basic_properties(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(
            id=42,
            function=2 * x + 1,
            name="my_func",
            subscripts=[1, 2],
            description="test function",
            parameters={"key": "value"},
        )
        assert nf.id == 42
        assert nf.name == "my_func"
        assert nf.subscripts == [1, 2]
        assert nf.description == "test function"
        assert nf.parameters == {"key": "value"}

    def test_function_property_returns_function(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=3 * x + 5)
        f = nf.function
        assert isinstance(f, Function)
        assert f.evaluate({1: 2.0}) == 11.0

    def test_from_scalar(self):
        nf = NamedFunction(id=0, function=42)
        assert nf.function.evaluate({}) == 42.0

    def test_from_linear(self):
        linear = Linear(terms={1: 3.0}, constant=1.0)
        nf = NamedFunction(id=0, function=linear)
        assert nf.function.evaluate({1: 2.0}) == 7.0

    def test_from_decision_variable(self):
        x = DecisionVariable.integer(5)
        nf = NamedFunction(id=0, function=x)
        assert nf.function.evaluate({5: 3.0}) == 3.0

    def test_optional_fields_default(self):
        nf = NamedFunction(id=0, function=0)
        assert nf.name is None
        assert nf.subscripts == []
        assert nf.description is None
        assert nf.parameters == {}


class TestNamedFunctionSerialization:
    def test_roundtrip(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(
            id=7,
            function=2 * x + 1,
            name="test",
            subscripts=[3],
        )
        data = nf.to_bytes()
        nf2 = NamedFunction.from_bytes(data)
        assert nf2.id == 7
        assert nf2.name == "test"
        assert nf2.subscripts == [3]
        assert nf2.function.evaluate({1: 5.0}) == 11.0


class TestNamedFunctionArithmetic:
    def test_add(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=2 * x)
        result = nf + 3
        assert isinstance(result, Function)
        assert result.evaluate({1: 1.0}) == 5.0

    def test_radd(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=2 * x)
        result = 3 + nf
        assert isinstance(result, Function)
        assert result.evaluate({1: 1.0}) == 5.0

    def test_sub(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=2 * x)
        result = nf - 1
        assert isinstance(result, Function)
        assert result.evaluate({1: 3.0}) == 5.0

    def test_rsub(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=2 * x)
        result = 10 - nf
        assert isinstance(result, Function)
        assert result.evaluate({1: 3.0}) == 4.0

    def test_mul(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=x + 1)
        result = nf * 3
        assert isinstance(result, Function)
        assert result.evaluate({1: 2.0}) == 9.0

    def test_rmul(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=x + 1)
        result = 3 * nf
        assert isinstance(result, Function)
        assert result.evaluate({1: 2.0}) == 9.0

    def test_neg(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=2 * x + 1)
        result = -nf
        assert isinstance(result, Function)
        assert result.evaluate({1: 3.0}) == -7.0


class TestNamedFunctionConstraint:
    def test_eq(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=2 * x)
        c = nf == 4
        assert isinstance(c, Constraint)

    def test_le(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=2 * x)
        c = nf <= 5
        assert isinstance(c, Constraint)

    def test_ge(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=2 * x)
        c = nf >= 0
        assert isinstance(c, Constraint)


class TestNamedFunctionEvaluate:
    def test_evaluate(self):
        x = DecisionVariable.integer(1)
        nf = NamedFunction(id=0, function=2 * x + 3, name="f")
        state = State({1: 5.0})
        result_bytes = nf.evaluate(state.to_bytes())
        result = rust.EvaluatedNamedFunction.from_bytes(result_bytes)
        assert result.evaluated_value == 13.0

    def test_partial_evaluate(self):
        x1 = DecisionVariable.integer(1)
        x2 = DecisionVariable.integer(2)
        nf = NamedFunction(id=0, function=2 * x1 + 3 * x2 + 1)
        state = State({1: 4.0})
        result_bytes = nf.partial_evaluate(state.to_bytes())
        nf2 = NamedFunction.from_bytes(result_bytes)
        assert isinstance(nf2, NamedFunction)
        # After substituting x1=4: 8 + 3*x2 + 1 = 3*x2 + 9
        assert nf2.function.evaluate({2: 2.0}) == 15.0


class TestInstanceNamedFunctions:
    def test_named_functions_property(self):
        instance, _ = _make_instance_with_named_functions()
        nfs = instance.named_functions
        assert len(nfs) == 2
        assert all(isinstance(nf, NamedFunction) for nf in nfs)

    def test_named_function_names(self):
        instance, _ = _make_instance_with_named_functions()
        names = instance.named_function_names
        assert names == {"sum_01", "sum_12"}

    def test_get_named_function_by_id(self):
        instance, _ = _make_instance_with_named_functions()
        nf = instance.get_named_function_by_id(0)
        assert isinstance(nf, NamedFunction)
        assert nf.name == "sum_01"

    def test_from_components_without_named_functions(self):
        """Ensure backward compatibility: named_functions=None works."""
        x = [DecisionVariable.binary(i) for i in range(2)]
        instance = Instance.from_components(
            decision_variables=x,
            objective=x[0] + x[1],
            constraints=[],
            sense=Instance.MINIMIZE,
        )
        assert instance.named_functions == []


class TestSolutionNamedFunctions:
    def test_solution_named_functions(self):
        instance, _ = _make_instance_with_named_functions()
        # x0=1, x1=1, x2=0
        solution = instance.evaluate({0: 1, 1: 1, 2: 0})
        nfs = solution.named_functions
        assert len(nfs) == 2

    def test_solution_named_function_ids(self):
        instance, _ = _make_instance_with_named_functions()
        solution = instance.evaluate({0: 1, 1: 1, 2: 0})
        assert solution.named_function_ids == {0, 1}

    def test_solution_named_function_names(self):
        instance, _ = _make_instance_with_named_functions()
        solution = instance.evaluate({0: 1, 1: 1, 2: 0})
        assert solution.named_function_names == {"sum_01", "sum_12"}

    def test_get_named_function_by_id(self):
        instance, _ = _make_instance_with_named_functions()
        solution = instance.evaluate({0: 1, 1: 1, 2: 0})
        nf = solution.get_named_function_by_id(0)
        # sum_01 = x0 + x1 = 1 + 1 = 2
        assert nf.evaluated_value == 2.0

    def test_extract_named_functions(self):
        instance, _ = _make_instance_with_named_functions()
        solution = instance.evaluate({0: 1, 1: 1, 2: 0})
        # sum_01 has subscripts=[0], so key is (0,)
        values = solution.extract_named_functions("sum_01")
        assert values == {(0,): 2.0}

    def test_extract_all_named_functions(self):
        instance, _ = _make_instance_with_named_functions()
        solution = instance.evaluate({0: 1, 1: 1, 2: 0})
        all_nfs = solution.extract_all_named_functions()
        assert "sum_01" in all_nfs
        assert "sum_12" in all_nfs
        assert all_nfs["sum_01"] == {(0,): 2.0}
        # sum_12 = x1 + x2 = 1 + 0 = 1
        assert all_nfs["sum_12"] == {(1,): 1.0}


class TestSampleSetNamedFunctions:
    def test_sample_set_named_functions(self):
        instance, _ = _make_instance_with_named_functions()
        samples = {
            0: {0: 1, 1: 0, 2: 0},
            1: {0: 1, 1: 1, 2: 0},
        }
        sample_set = instance.evaluate_samples(samples)
        nfs = sample_set.named_functions
        assert len(nfs) == 2

    def test_sample_set_named_function_names(self):
        instance, _ = _make_instance_with_named_functions()
        samples = {0: {0: 1, 1: 0, 2: 0}}
        sample_set = instance.evaluate_samples(samples)
        assert sample_set.named_function_names == {"sum_01", "sum_12"}

    def test_get_named_function_by_id(self):
        instance, _ = _make_instance_with_named_functions()
        samples = {0: {0: 1, 1: 0, 2: 0}}
        sample_set = instance.evaluate_samples(samples)
        nf = sample_set.get_named_function_by_id(0)
        assert nf.name == "sum_01"

    def test_extract_named_functions(self):
        instance, _ = _make_instance_with_named_functions()
        samples = {0: {0: 1, 1: 1, 2: 0}}
        sample_set = instance.evaluate_samples(samples)
        values = sample_set.extract_named_functions("sum_01", 0)
        assert values == {(0,): 2.0}

    def test_extract_all_named_functions(self):
        instance, _ = _make_instance_with_named_functions()
        samples = {0: {0: 1, 1: 1, 2: 0}}
        sample_set = instance.evaluate_samples(samples)
        all_nfs = sample_set.extract_all_named_functions(0)
        assert "sum_01" in all_nfs
        assert "sum_12" in all_nfs
