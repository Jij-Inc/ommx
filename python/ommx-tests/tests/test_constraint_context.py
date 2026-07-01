"""
Test context-field modification functionality for Constraint
"""

from ommx import DecisionVariable, Constraint


def test_constraint_set_name():
    """Test set_name method uses efficient Rust implementation"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Initially no name
    assert constraint.name is None

    # Set name - returns a Constraint object for chaining
    result = constraint.set_name("test_constraint")

    # Name should be set on the returned object
    assert result.name == "test_constraint"

    # Can update name via chaining
    result = result.set_name("updated_name")
    assert result.name == "updated_name"


def test_constraint_add_subscripts():
    """Test add_subscripts method uses efficient Rust implementation"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Initially no subscripts
    assert constraint.subscripts == []

    # Add subscripts - returns a Constraint object for chaining
    result = constraint.add_subscripts([1, 2, 3])

    # Subscripts should be set on the returned object
    assert result.subscripts == [1, 2, 3]

    # Add more subscripts via chaining
    result = result.add_subscripts([4, 5])
    assert result.subscripts == [1, 2, 3, 4, 5]


def test_constraint_chaining():
    """Test that methods can be chained together"""
    x = DecisionVariable.binary(0)
    constraint = (x == 1).set_name("chained").add_subscripts([1, 2])

    assert constraint.name == "chained"
    assert constraint.subscripts == [1, 2]


def test_constraint_method_efficiency():
    """Test that method chaining works correctly"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Chain multiple modifications
    result = constraint.set_name("test").add_subscripts([1, 2])

    # Values should be set on the chained result
    assert result.name == "test"
    assert result.subscripts == [1, 2]


def test_constraint_description():
    """Test description functionality"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Initially no description
    assert constraint.description is None

    # Set description - returns a Constraint object for chaining
    result = constraint.set_description("This is a test constraint")

    # Description should be set on the returned object
    assert result.description == "This is a test constraint"

    # Can update description via chaining
    result = result.set_description("Updated description")
    assert result.description == "Updated description"


def test_constraint_parameters():
    """Test parameters functionality"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Initially no parameters
    assert constraint.parameters == {}

    # Set parameters - returns a Constraint object for chaining
    result = constraint.set_parameters({"solver": "highs", "timeout": "60"})

    # Parameters should be set on the returned object
    assert result.parameters == {"solver": "highs", "timeout": "60"}

    # set_parameters replaces all parameters.
    result = result.set_parameters({"precision": "1e-6"})
    assert result.parameters == {"precision": "1e-6"}


def test_constraint_add_parameters_merges_existing_parameter_dict():
    """add_parameters merges entries; set_parameters replaces the whole map."""
    x = DecisionVariable.binary(0)
    constraint = (x == 1).set_parameters({"solver": "highs", "timeout": "60"})

    result = constraint.add_parameters({"timeout": "120", "precision": "1e-6"})

    assert result.parameters == {
        "solver": "highs",
        "timeout": "120",
        "precision": "1e-6",
    }


def test_constraint_complete_context():
    """Test all context methods together"""
    x = DecisionVariable.binary(0)
    constraint = (
        (x == 1)
        .set_name("comprehensive_test")
        .set_description("A comprehensive test constraint")
        .add_subscripts([10, 20, 30])
        .add_parameters({"method": "branch_and_bound", "threads": "4"})
    )

    # Verify all context is set correctly
    assert constraint.name == "comprehensive_test"
    assert constraint.description == "A comprehensive test constraint"
    assert constraint.subscripts == [10, 20, 30]
    assert constraint.parameters == {"method": "branch_and_bound", "threads": "4"}


def test_constraint_context_efficiency():
    """Test that all context methods can be chained together"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Chain all context-field modifications
    result = (
        constraint.set_name("efficient")
        .set_description("Efficient test")
        .add_subscripts([1])
        .add_parameters({"key": "value"})
    )

    # All values should be set on the chained result
    assert result.name == "efficient"
    assert result.description == "Efficient test"
    assert result.subscripts == [1]
    assert result.parameters == {"key": "value"}


def test_replacing_metadata_does_not_have_add_aliases():
    """Only append/merge metadata operations use the add_* prefix."""
    constraint = DecisionVariable.binary(0) == 1

    assert not hasattr(constraint, "add_name")
    assert not hasattr(constraint, "add_description")


def test_constraint_constructor_with_context():
    """Test Constraint constructor properly handles description and parameters"""

    x = DecisionVariable.binary(0)
    function = x + 1

    # Create constraint with all context in constructor
    constraint = Constraint(
        function=function,
        equality=Constraint.EQUAL_TO_ZERO,
        name="constructor_test",
        description="Created via constructor",
        subscripts=[5, 10, 15],
        parameters={"method": "constructor", "priority": "high"},
    )

    # Verify all context is set correctly
    assert constraint.name == "constructor_test"
    assert constraint.description == "Created via constructor"
    assert constraint.subscripts == [5, 10, 15]
    assert constraint.parameters == {"method": "constructor", "priority": "high"}


def test_constraint_constructor_partial_context():
    """Test Constraint constructor with partial context"""
    x = DecisionVariable.binary(0)
    function = x - 2

    # Create constraint with only some context fields
    constraint = Constraint(
        function=function,
        equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
        description="Only description set",
    )

    # Verify context fields
    assert constraint.description == "Only description set"
    assert constraint.parameters == {}  # Should be empty
    assert constraint.name is None  # Should be None
    assert constraint.subscripts == []  # Should be empty
