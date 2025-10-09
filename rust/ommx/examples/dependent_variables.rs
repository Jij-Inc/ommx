//! Example: Using dependent variables in optimization instances
//!
//! This example demonstrates how to add dependent variables to an Instance using
//! the substitute_one and substitute_acyclic methods.
//!
//! Dependent variables are variables that are defined as functions of other variables.
//! They are stored in the `decision_variable_dependency` field and are automatically
//! excluded from appearing in the objective function and constraints after substitution.
//!
//! Key concepts:
//! - `substitute_one`: Add a single dependent variable by substituting it in the instance
//! - `substitute_acyclic`: Efficiently add multiple dependent variables at once
//! - Dependent variables maintain the invariant: they never appear in objective/constraints

use anyhow::Result;
use ommx::{
    assign, coeff, linear, Constraint, ConstraintID, DecisionVariable, Function, Instance, Sense,
    Substitute, VariableID,
};
use std::collections::BTreeMap;

fn main() -> Result<()> {
    println!("=== Example: Dependent Variables ===\n");

    // Create initial instance: minimize x1 + x2 subject to x1 + x2 <= 10
    let mut decision_variables = BTreeMap::new();
    decision_variables.insert(
        VariableID::from(1),
        DecisionVariable::continuous(VariableID::from(1)),
    );
    decision_variables.insert(
        VariableID::from(2),
        DecisionVariable::continuous(VariableID::from(2)),
    );
    decision_variables.insert(
        VariableID::from(3),
        DecisionVariable::continuous(VariableID::from(3)),
    );
    decision_variables.insert(
        VariableID::from(4),
        DecisionVariable::continuous(VariableID::from(4)),
    );

    let objective = Function::from(linear!(1) + linear!(2));
    let constraint = Constraint::less_than_or_equal_to_zero(
        ConstraintID::from(1),
        (linear!(1) + linear!(2) + coeff!(-10.0)).into(),
    );

    let mut constraints = BTreeMap::new();
    constraints.insert(ConstraintID::from(1), constraint);

    let instance = Instance::new(Sense::Minimize, objective, decision_variables, constraints)?;

    println!("Initial instance:");
    println!("  Objective: x1 + x2");
    println!("  Constraint: x1 + x2 <= 10");
    println!(
        "  Decision variables: {}",
        instance.decision_variables().len()
    );
    println!(
        "  Dependent variables: {}\n",
        instance.decision_variable_dependency().len()
    );

    // Example 1: Using substitute_one to add a single dependent variable
    println!("=== Example 1: substitute_one ===");
    println!("Substituting x1 with x3 + x4 (makes x1 dependent on x3 and x4)");

    let substitution = Function::from(linear!(3) + linear!(4));
    let instance = instance.substitute_one(VariableID::from(1), &substitution)?;

    println!("\nAfter substitution:");
    println!("  Objective becomes: (x3 + x4) + x2 = x2 + x3 + x4");
    println!("  Constraint becomes: (x3 + x4) + x2 <= 10");
    println!(
        "  Dependent variables: {}",
        instance.decision_variable_dependency().len()
    );
    println!("  Dependent variable mapping:");
    for (id, func) in instance.decision_variable_dependency().iter() {
        println!("    x{} <- {:?}", id.into_inner(), func);
    }

    // Example 2: Using substitute_acyclic to add multiple dependent variables efficiently
    println!("\n=== Example 2: substitute_acyclic ===");
    println!("Substituting multiple variables at once:");
    println!("  x2 <- x3 + 1.0");
    println!("  x4 <- 2.0 * x3");

    let substitutions = assign! {
        2 <- linear!(3) + coeff!(1.0),
        4 <- coeff!(2.0) * linear!(3)
    };

    let instance = instance.substitute_acyclic(&substitutions)?;

    println!("\nAfter multiple substitutions:");
    println!("  Objective becomes: (x3 + 1.0) + x3 + (2.0 * x3) = 4x3 + 1.0");
    println!("  Constraint becomes: (x3 + (2.0 * x3)) + (x3 + 1.0) <= 10");
    println!(
        "  Dependent variables: {}",
        instance.decision_variable_dependency().len()
    );
    println!("  Dependent variable mappings:");
    for (id, func) in instance.decision_variable_dependency().iter() {
        println!("    x{} <- {:?}", id.into_inner(), func);
    }

    println!("\n=== Key Points ===");
    println!("1. Dependent variables are stored in decision_variable_dependency");
    println!("2. They are automatically removed from objective and constraints");
    println!("3. substitute_acyclic is more efficient for multiple variables");
    println!(
        "4. The invariant is maintained: dependent variables never appear in objective/constraints"
    );

    Ok(())
}
