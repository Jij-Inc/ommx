use anyhow::Result;
use ommx::{coeff, quadratic, Constraint, DecisionVariable, Function, Instance, Sense, VariableID};
use std::collections::BTreeMap;

fn main() -> Result<()> {
    // All decision variables included in the instance
    let mut decision_variables = BTreeMap::new();

    // Binary variable x
    // Note: It is not necessarily required that "x has ID 0, y has ID 1"
    // The type of decision variable is "binary variable"
    // The lower bound of the binary variable is 0, the upper bound is 1
    decision_variables.insert(
        VariableID::from(0),
        DecisionVariable::binary(VariableID::from(0)),
    );

    // Binary variable y
    // Note: It is not necessarily required that "x has ID 0, y has ID 1"
    // The type of decision variable is "binary variable"
    // The lower bound of the binary variable is 0, the upper bound is 1
    decision_variables.insert(
        VariableID::from(1),
        DecisionVariable::binary(VariableID::from(1)),
    );

    // Objective function: 2.0 * x * y
    // QUBO model (Quadratic) so it's a quadratic polynomial
    let objective = Function::Quadratic(coeff!(2.0) * quadratic!(0, 1));

    // QUBO model (Unconstrained) so there are no constraints
    let constraints: BTreeMap<_, Constraint> = BTreeMap::new();

    // Minimize the objective function
    // For simplicity, there are no other fields to specify
    let instance = Instance::new(Sense::Minimize, objective, decision_variables, constraints)?;

    // Display instance information
    println!("Sense: {:?}", instance.sense());
    println!(
        "Decision variables: {}",
        instance.decision_variables().len()
    );
    println!("Constraints: {}", instance.constraints().len());
    println!("Objective: {:?}", instance.objective());

    Ok(())
}
