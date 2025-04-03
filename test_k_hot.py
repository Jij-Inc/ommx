"""
Test script for k-hot constraints functionality.
"""
from ommx.v1 import ConstraintHintsWrapper
from ommx.v1.constraint_hints_pb2 import ConstraintHints, KHotList
from ommx.v1.one_hot_pb2 import OneHot
from ommx.v1.k_hot_pb2 import KHot

def test_one_hot_constraints():
    """Test one_hot_constraints method."""
    hints = ConstraintHints()
    
    one_hot1 = OneHot(constraint_id=1, decision_variables=[10, 11, 12])
    one_hot2 = OneHot(constraint_id=2, decision_variables=[20, 21, 22])
    hints.one_hot_constraints.extend([one_hot1, one_hot2])
    
    k_hot1 = KHot(constraint_id=3, decision_variables=[30, 31, 32], num_hot_vars=1)
    hints.k_hot_constraints[1].constraints.append(k_hot1)
    
    wrapper = ConstraintHintsWrapper(hints)
    
    one_hot_constraints = wrapper.one_hot_constraints()
    assert len(one_hot_constraints) == 3
    constraint_ids = {c.constraint_id for c in one_hot_constraints}
    assert constraint_ids == {1, 2, 3}
    
    print("test_one_hot_constraints: PASSED")

def test_k_hot_constraints():
    """Test k_hot_constraints method."""
    hints = ConstraintHints()
    
    one_hot1 = OneHot(constraint_id=1, decision_variables=[10, 11, 12])
    one_hot2 = OneHot(constraint_id=2, decision_variables=[20, 21, 22])
    hints.one_hot_constraints.extend([one_hot1, one_hot2])
    
    k_hot1 = KHot(constraint_id=3, decision_variables=[30, 31, 32], num_hot_vars=1)
    hints.k_hot_constraints[1].constraints.append(k_hot1)
    
    k_hot2 = KHot(constraint_id=4, decision_variables=[40, 41, 42, 43], num_hot_vars=2)
    hints.k_hot_constraints[2].constraints.append(k_hot2)
    
    wrapper = ConstraintHintsWrapper(hints)
    
    k_hot_constraints = wrapper.k_hot_constraints()
    assert len(k_hot_constraints) == 2  # k=1 and k=2
    assert 1 in k_hot_constraints
    assert 2 in k_hot_constraints
    assert len(k_hot_constraints[1]) == 3  # 2 from one_hot_constraints + 1 from k_hot_constraints[1]
    assert len(k_hot_constraints[2]) == 1
    
    k1_constraint_ids = {c.constraint_id for c in k_hot_constraints[1]}
    assert k1_constraint_ids == {1, 2, 3}
    
    k2_constraint_ids = {c.constraint_id for c in k_hot_constraints[2]}
    assert k2_constraint_ids == {4}
    
    print("test_k_hot_constraints: PASSED")

if __name__ == "__main__":
    test_one_hot_constraints()
    test_k_hot_constraints()
    print("All tests passed!")
