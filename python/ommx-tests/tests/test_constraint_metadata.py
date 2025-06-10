"""
Test metadata modification functionality for Constraint
"""
from ommx.v1 import DecisionVariable


def test_constraint_add_name():
    """Test add_name method uses efficient Rust implementation"""
    x = DecisionVariable.binary(0)
    constraint = x == 1
    
    # Initially no name
    assert constraint.name is None
    
    # Add name
    result = constraint.add_name("test_constraint")
    
    # Should return the same constraint object (chaining)
    assert result is constraint
    
    # Name should be set
    assert constraint.name == "test_constraint"
    
    # Can update name
    constraint.add_name("updated_name")
    assert constraint.name == "updated_name"


def test_constraint_add_subscripts():
    """Test add_subscripts method uses efficient Rust implementation"""
    x = DecisionVariable.binary(0)
    constraint = x == 1
    
    # Initially no subscripts
    assert constraint.subscripts == []
    
    # Add subscripts
    result = constraint.add_subscripts([1, 2, 3])
    
    # Should return the same constraint object (chaining)
    assert result is constraint
    
    # Subscripts should be set
    assert constraint.subscripts == [1, 2, 3]
    
    # Add more subscripts
    constraint.add_subscripts([4, 5])
    assert constraint.subscripts == [1, 2, 3, 4, 5]


def test_constraint_chaining():
    """Test that methods can be chained together"""
    x = DecisionVariable.binary(0)
    constraint = (x == 1).add_name("chained").add_subscripts([1, 2])
    
    assert constraint.name == "chained"
    assert constraint.subscripts == [1, 2]


def test_constraint_method_efficiency():
    """Test that methods modify constraint in place rather than creating new objects"""
    x = DecisionVariable.binary(0)
    constraint = x == 1
    
    # Store reference to the raw object
    original_raw = constraint.raw
    
    # Modify metadata
    constraint.add_name("test").add_subscripts([1, 2])
    
    # Raw object should be the same (in-place modification)
    assert constraint.raw is original_raw
    
    # But values should be updated
    assert constraint.name == "test"
    assert constraint.subscripts == [1, 2]


def test_constraint_description():
    """Test description functionality"""
    x = DecisionVariable.binary(0)
    constraint = x == 1
    
    # Initially no description
    assert constraint.description is None
    
    # Add description
    result = constraint.add_description("This is a test constraint")
    
    # Should return the same constraint object (chaining)
    assert result is constraint
    
    # Description should be set
    assert constraint.description == "This is a test constraint"
    
    # Can update description
    constraint.add_description("Updated description")
    assert constraint.description == "Updated description"


def test_constraint_parameters():
    """Test parameters functionality"""
    x = DecisionVariable.binary(0)
    constraint = x == 1
    
    # Initially no parameters
    assert constraint.parameters == {}
    
    # Add parameters
    result = constraint.add_parameters({"solver": "highs", "timeout": "60"})
    
    # Should return the same constraint object (chaining)
    assert result is constraint
    
    # Parameters should be set
    assert constraint.parameters == {"solver": "highs", "timeout": "60"}
    
    # Add more parameters
    constraint.add_parameters({"precision": "1e-6"})
    assert constraint.parameters == {"solver": "highs", "timeout": "60", "precision": "1e-6"}


def test_constraint_set_id():
    """Test set_id functionality"""
    x = DecisionVariable.binary(0)
    constraint = x == 1
    
    # Get initial ID
    initial_id = constraint.id
    
    # Set new ID
    result = constraint.set_id(999)
    
    # Should return the same constraint object (chaining)
    assert result is constraint
    
    # ID should be updated
    assert constraint.id == 999
    assert constraint.id != initial_id


def test_constraint_complete_metadata():
    """Test all metadata methods together"""
    x = DecisionVariable.binary(0)
    constraint = (x == 1).add_name("comprehensive_test").add_description("A comprehensive test constraint").add_subscripts([10, 20, 30]).add_parameters({"method": "branch_and_bound", "threads": "4"}).set_id(42)
    
    # Verify all metadata is set correctly
    assert constraint.id == 42
    assert constraint.name == "comprehensive_test"
    assert constraint.description == "A comprehensive test constraint"
    assert constraint.subscripts == [10, 20, 30]
    assert constraint.parameters == {"method": "branch_and_bound", "threads": "4"}


def test_constraint_metadata_efficiency():
    """Test that all metadata methods modify constraint in place"""
    x = DecisionVariable.binary(0)
    constraint = x == 1
    
    # Store reference to the raw object
    original_raw = constraint.raw
    
    # Modify all metadata
    constraint.set_id(123).add_name("efficient").add_description("Efficient test").add_subscripts([1]).add_parameters({"key": "value"})
    
    # Raw object should be the same (in-place modification)
    assert constraint.raw is original_raw
    
    # But all values should be updated
    assert constraint.id == 123
    assert constraint.name == "efficient"
    assert constraint.description == "Efficient test"
    assert constraint.subscripts == [1]
    assert constraint.parameters == {"key": "value"}


def test_constraint_constructor_with_metadata():
    """Test Constraint constructor properly handles description and parameters"""
    from ommx.v1 import Constraint, Equality
    
    x = DecisionVariable.binary(0)
    function = x + 1
    
    # Create constraint with all metadata in constructor
    constraint = Constraint(
        function=function,
        equality=Equality.EQUALITY_EQUAL_TO_ZERO,
        id=100,
        name="constructor_test",
        description="Created via constructor",
        subscripts=[5, 10, 15],
        parameters={"method": "constructor", "priority": "high"}
    )
    
    # Verify all metadata is set correctly
    assert constraint.id == 100
    assert constraint.name == "constructor_test"
    assert constraint.description == "Created via constructor"
    assert constraint.subscripts == [5, 10, 15]
    assert constraint.parameters == {"method": "constructor", "priority": "high"}


def test_constraint_constructor_partial_metadata():
    """Test Constraint constructor with partial metadata"""
    from ommx.v1 import Constraint, Equality
    
    x = DecisionVariable.binary(0)
    function = x - 2
    
    # Create constraint with only some metadata
    constraint = Constraint(
        function=function,
        equality=Equality.EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO,
        description="Only description set"
    )
    
    # Verify metadata
    assert constraint.description == "Only description set"
    assert constraint.parameters == {}  # Should be empty
    assert constraint.name is None      # Should be None
    assert constraint.subscripts == []  # Should be empty