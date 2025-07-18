use crate::{
    coeff, linear, mps::*, Bound, Constraint, ConstraintID, DecisionVariable, Function, Instance,
    Sense, VariableID,
};
use maplit::btreemap;

// Test variable filtering and removed constraint handling
#[test]
fn test_unused_variable_filtering() {
    // Create instance with 3 variables but only use 2
    let decision_variables = btreemap! {
        VariableID::from(0) => DecisionVariable::new(
            VariableID::from(0),
            crate::decision_variable::Kind::Continuous,
            Bound::new(0.0, 10.0).unwrap(),
            None,
            crate::ATol::default()
        ).unwrap(),
        VariableID::from(1) => DecisionVariable::new(
            VariableID::from(1),
            crate::decision_variable::Kind::Continuous,
            Bound::new(0.0, 10.0).unwrap(),
            None,
            crate::ATol::default()
        ).unwrap(),
        VariableID::from(2) => DecisionVariable::new(
            VariableID::from(2),
            crate::decision_variable::Kind::Continuous,
            Bound::new(0.0, 10.0).unwrap(),
            None,
            crate::ATol::default()
        ).unwrap(),  // This variable is unused
    };

    // Only use x1 in objective: minimize x1
    let objective = Function::from(linear!(0));

    // Only use x2 in constraint: x2 - 5 <= 0
    let constraints = btreemap! {
        ConstraintID::from(0) => Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(0),
            Function::from(linear!(1) + coeff!(-5.0))
        ),
    };

    let instance =
        Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

    // Write to MPS and read back
    let mut buffer = Vec::new();
    format::format(&instance, &mut buffer).unwrap();
    let loaded_instance = parse(&buffer[..]).unwrap();

    // Should only have 2 variables (x0, x1), not 3
    assert_eq!(loaded_instance.decision_variables().len(), 2);

    // Check that unused variable (id=2) is not present
    let var_ids: Vec<u64> = loaded_instance
        .decision_variables()
        .keys()
        .map(|id| id.into_inner())
        .collect();
    assert!(var_ids.contains(&0)); // x0 should be present (used in objective)
    assert!(var_ids.contains(&1)); // x1 should be present (used in constraint)
    assert!(!var_ids.contains(&2)); // x2 should be filtered out (unused)
}

#[test]
fn test_removed_constraint_variable_preservation() {
    // Create instance with variables and constraints, then relax one
    let decision_variables = btreemap! {
        VariableID::from(0) => DecisionVariable::new(
            VariableID::from(0),
            crate::decision_variable::Kind::Continuous,
            Bound::new(0.0, 10.0).unwrap(),
            None,
            crate::ATol::default()
        ).unwrap(),
        VariableID::from(1) => DecisionVariable::new(
            VariableID::from(1),
            crate::decision_variable::Kind::Continuous,
            Bound::new(0.0, 10.0).unwrap(),
            None,
            crate::ATol::default()
        ).unwrap(),
    };

    // Use only x0 in objective: minimize x0
    let objective = Function::from(linear!(0));

    // Add constraint that uses x1: x1 - 3 <= 0
    let constraints = btreemap! {
        ConstraintID::from(100) => Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(100),
            Function::from(linear!(1) + coeff!(-3.0))
        ),
    };

    let mut instance =
        Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

    // Relax the constraint to create a removed constraint
    instance
        .relax_constraint(ConstraintID::from(100), "test_removal".to_string(), [])
        .unwrap();

    // Write to MPS and read back
    let mut buffer = Vec::new();
    format::format(&instance, &mut buffer).unwrap();
    let loaded_instance = parse(&buffer[..]).unwrap();

    // Check what variables are actually present after roundtrip
    let var_ids: Vec<u64> = loaded_instance
        .decision_variables()
        .keys()
        .map(|id| id.into_inner())
        .collect();

    // Only x0 should be preserved since the constraint using x1 was removed
    assert_eq!(loaded_instance.decision_variables().len(), 1);
    assert_eq!(var_ids, vec![0]); // Only variable from objective is preserved

    // Should have 0 constraints (removed constraint is not exported)
    assert_eq!(loaded_instance.constraints().len(), 0);

    // Should have 0 removed_constraints (not supported in MPS format)
    assert_eq!(loaded_instance.removed_constraints().len(), 0);

    // Original instance should have 1 removed constraint
    assert_eq!(instance.removed_constraints().len(), 1);
}

#[test]
fn test_removed_constraint_information_loss() {
    // Create instance with both active and removed constraints
    let decision_variables = btreemap! {
        VariableID::from(0) => DecisionVariable::new(
            VariableID::from(0),
            crate::decision_variable::Kind::Continuous,
            Bound::new(0.0, 10.0).unwrap(),
            None,
            crate::ATol::default()
        ).unwrap(),
    };

    // Add objective: minimize x0
    let objective = Function::from(linear!(0));

    // Add two constraints
    let constraints = btreemap! {
        ConstraintID::from(0) => Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(0),
            Function::from(linear!(0) + coeff!(-5.0))  // x0 - 5 <= 0
        ),
        ConstraintID::from(1) => Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::from(coeff!(2.0) * linear!(0) + coeff!(-10.0))  // 2*x0 - 10 = 0
        ),
    };

    let mut instance =
        Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

    // Relax one constraint to create a removed constraint
    instance
        .relax_constraint(ConstraintID::from(1), "redundant".to_string(), [])
        .unwrap();

    // Write to MPS and read back
    let mut buffer = Vec::new();
    format::format(&instance, &mut buffer).unwrap();
    let loaded_instance = parse(&buffer[..]).unwrap();

    // Only active constraint should remain
    assert_eq!(loaded_instance.constraints().len(), 1);

    // Verify the active constraint exists and has correct structure
    let first_constraint = loaded_instance.constraints().values().next().unwrap();
    // Check that it's the correct constraint (should be x0 - 5 <= 0)
    assert_eq!(
        first_constraint.equality,
        crate::Equality::LessThanOrEqualToZero
    );

    // No removed constraints in the result (MPS format doesn't support them)
    assert_eq!(loaded_instance.removed_constraints().len(), 0);

    // Original instance should have 1 active + 1 removed = 2 total constraint-like objects
    assert_eq!(instance.constraints().len(), 1); // 1 active constraint
    assert_eq!(instance.removed_constraints().len(), 1); // 1 removed constraint
}
