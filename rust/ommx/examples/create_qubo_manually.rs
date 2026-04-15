//! Example: Creating a QUBO instance manually
//!
//! This example demonstrates how to manually construct a QUBO (Quadratic Unconstrained Binary Optimization)
//! instance using the OMMX Rust SDK v2 API.
//!
//! Note: OMMX also provides functionality to convert general instances to QUBO format.
//! This example shows the manual construction approach.

use anyhow::Result;
use ommx::{coeff, quadratic, Constraint, DecisionVariable, Function, Instance, Sense, VariableID};
use std::collections::BTreeMap;

fn main() -> Result<()> {
    let mut decision_variables = BTreeMap::new();

    // Binary variable x_{0, 0}
    {
        let mut var = DecisionVariable::binary(VariableID::from(0));
        var.metadata.name = Some("x".to_string());
        var.metadata.subscripts = vec![0, 0];
        decision_variables.insert(VariableID::from(0), var);
    }

    // Binary variable x_{1, 0}
    {
        let mut var = DecisionVariable::binary(VariableID::from(1));
        var.metadata.name = Some("x".to_string());
        var.metadata.subscripts = vec![1, 0];
        decision_variables.insert(VariableID::from(1), var);
    }

    // Objective function: 2.0 * x_{0, 0} * x_{1, 0} - x_{0, 0} - x_{1, 0} + 3.0
    let objective = Function::Quadratic(
        // Quadratic term: 2.0 * x_{0,0} * x_{1, 0}
        coeff!(2.0) * quadratic!(0, 1)
            // Linear term: - x_{0, 0}
            + coeff!(-1.0) * quadratic!(0)
            // Linear term: - x_{1, 0}
            + coeff!(-1.0) * quadratic!(1)
            // Constant term: 3.0
            + coeff!(3.0),
    );

    // No constraints (unconstrained QUBO)
    let constraints: BTreeMap<_, Constraint> = BTreeMap::new();

    // Minimize the objective function
    let instance = Instance::new(Sense::Minimize, objective, decision_variables, constraints)?;

    // Display instance information
    println!("Sense: {:?}", instance.sense());
    println!(
        "Decision variables: {}",
        instance.decision_variables().len()
    );
    println!("Constraints: {}", instance.constraints().len());
    println!("Objective: {:?}", instance.objective());

    // Display decision variable metadata
    println!("\nDecision variables:");
    for (id, var) in instance.decision_variables() {
        println!(
            "  Variable {}: name={:?}, subscripts={:?}",
            id, var.metadata.name, var.metadata.subscripts
        );
    }

    Ok(())
}
