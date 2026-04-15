"""
Test metadata modification functionality for Constraint
"""

from ommx.v1 import DecisionVariable, Constraint


def test_constraint_add_name():
    """Test add_name method uses efficient Rust implementation"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Initially no name
    assert constraint.name is None

    # Add name - returns a Constraint object for chaining
    result = constraint.add_name("test_constraint")

    # Name should be set on the returned object
    assert result.name == "test_constraint"

    # Can update name via chaining
    result = result.add_name("updated_name")
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
    constraint = (x == 1).add_name("chained").add_subscripts([1, 2])

    assert constraint.name == "chained"
    assert constraint.subscripts == [1, 2]


def test_constraint_method_efficiency():
    """Test that method chaining works correctly"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Chain multiple modifications
    result = constraint.add_name("test").add_subscripts([1, 2])

    # Values should be set on the chained result
    assert result.name == "test"
    assert result.subscripts == [1, 2]


def test_constraint_description():
    """Test description functionality"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Initially no description
    assert constraint.description is None

    # Add description - returns a Constraint object for chaining
    result = constraint.add_description("This is a test constraint")

    # Description should be set on the returned object
    assert result.description == "This is a test constraint"

    # Can update description via chaining
    result = result.add_description("Updated description")
    assert result.description == "Updated description"


def test_constraint_parameters():
    """Test parameters functionality"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Initially no parameters
    assert constraint.parameters == {}

    # Add parameters - returns a Constraint object for chaining
    result = constraint.add_parameters({"solver": "highs", "timeout": "60"})

    # Parameters should be set on the returned object
    assert result.parameters == {"solver": "highs", "timeout": "60"}

    # Note: add_parameters replaces all parameters (via set_parameters alias)
    result = result.add_parameters({"precision": "1e-6"})
    assert result.parameters == {"precision": "1e-6"}


def test_constraint_set_id():
    """Test set_id functionality"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Get initial ID
    initial_id = constraint.id

    # Set new ID - returns a Constraint object for chaining
    result = constraint.set_id(999)

    # ID should be updated on the returned object
    assert result.id == 999
    assert result.id != initial_id


def test_constraint_complete_metadata():
    """Test all metadata methods together"""
    x = DecisionVariable.binary(0)
    constraint = (
        (x == 1)
        .add_name("comprehensive_test")
        .add_description("A comprehensive test constraint")
        .add_subscripts([10, 20, 30])
        .add_parameters({"method": "branch_and_bound", "threads": "4"})
        .set_id(42)
    )

    # Verify all metadata is set correctly
    assert constraint.id == 42
    assert constraint.name == "comprehensive_test"
    assert constraint.description == "A comprehensive test constraint"
    assert constraint.subscripts == [10, 20, 30]
    assert constraint.parameters == {"method": "branch_and_bound", "threads": "4"}


def test_constraint_metadata_efficiency():
    """Test that all metadata methods can be chained together"""
    x = DecisionVariable.binary(0)
    constraint = x == 1

    # Chain all metadata modifications
    result = (
        constraint.set_id(123)
        .add_name("efficient")
        .add_description("Efficient test")
        .add_subscripts([1])
        .add_parameters({"key": "value"})
    )

    # All values should be set on the chained result
    assert result.id == 123
    assert result.name == "efficient"
    assert result.description == "Efficient test"
    assert result.subscripts == [1]
    assert result.parameters == {"key": "value"}


def test_constraint_constructor_with_metadata():
    """Test Constraint constructor properly handles description and parameters"""

    x = DecisionVariable.binary(0)
    function = x + 1

    # Create constraint with all metadata in constructor
    constraint = Constraint(
        function=function,
        equality=Constraint.EQUAL_TO_ZERO,
        id=100,
        name="constructor_test",
        description="Created via constructor",
        subscripts=[5, 10, 15],
        parameters={"method": "constructor", "priority": "high"},
    )

    # Verify all metadata is set correctly
    assert constraint.id == 100
    assert constraint.name == "constructor_test"
    assert constraint.description == "Created via constructor"
    assert constraint.subscripts == [5, 10, 15]
    assert constraint.parameters == {"method": "constructor", "priority": "high"}


def test_constraint_constructor_partial_metadata():
    """Test Constraint constructor with partial metadata"""
    x = DecisionVariable.binary(0)
    function = x - 2

    # Create constraint with only some metadata
    constraint = Constraint(
        function=function,
        equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
        description="Only description set",
    )

    # Verify metadata
    assert constraint.description == "Only description set"
    assert constraint.parameters == {}  # Should be empty
    assert constraint.name is None  # Should be None
    assert constraint.subscripts == []  # Should be empty
