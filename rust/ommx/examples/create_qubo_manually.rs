//! Example: Creating a QUBO instance manually
//!
//! This example demonstrates how to manually construct a QUBO (Quadratic Unconstrained Binary Optimization)
//! instance using the OMMX Rust SDK v3 API.
//!
//! Note: OMMX also provides functionality to convert general instances to QUBO format.
//! This example shows the manual construction approach.
//!
//! In v3, per-variable auxiliary metadata (name, subscripts, …) lives in
//! the [`VariableMetadataStore`] sibling field of [`Instance`] rather
//! than on each [`DecisionVariable`]. Set it via
//! [`Instance::variable_metadata_mut`] after construction.

use anyhow::Result;
use ommx::{coeff, quadratic, Constraint, DecisionVariable, Function, Instance, Sense, VariableID};
use std::collections::BTreeMap;

fn main() -> Result<()> {
    let mut decision_variables = BTreeMap::new();

    // Binary variable x_{0, 0}
    decision_variables.insert(
        VariableID::from(0),
        DecisionVariable::binary(VariableID::from(0)),
    );
    // Binary variable x_{1, 0}
    decision_variables.insert(
        VariableID::from(1),
        DecisionVariable::binary(VariableID::from(1)),
    );

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
    let mut instance = Instance::new(Sense::Minimize, objective, decision_variables, constraints)?;

    // Attach per-variable metadata via the SoA store on the instance.
    {
        let meta = instance.variable_metadata_mut();
        meta.set_name(VariableID::from(0), "x");
        meta.set_subscripts(VariableID::from(0), vec![0, 0]);
        meta.set_name(VariableID::from(1), "x");
        meta.set_subscripts(VariableID::from(1), vec![1, 0]);
    }

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
    let meta = instance.variable_metadata();
    for (id, _var) in instance.decision_variables() {
        println!(
            "  Variable {}: name={:?}, subscripts={:?}",
            id,
            meta.name(*id),
            meta.subscripts(*id),
        );
    }

    Ok(())
}
