//! Simple Linear Programming Example
//!
//! Minimize: x + 2*y
//! Subject to:
//!   x + y >= 1
//!   x, y >= 0

use maplit::btreemap;
use ommx::{coeff, linear, Constraint, ConstraintID, DecisionVariable, Instance, Sense, VariableID};
use ommx_scip_adapter::ScipAdapter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Simple Linear Programming Example");
    println!("==================================");
    println!("\nProblem:");
    println!("  Minimize: x + 2*y");
    println!("  Subject to: x + y >= 1");
    println!("             x, y >= 0");

    // Create decision variables
    let decision_variables = btreemap! {
        VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1)),
        VariableID::from(2) => DecisionVariable::continuous(VariableID::from(2)),
    };

    // Objective: x + 2*y
    let objective = coeff!(1.0) * linear!(1) + coeff!(2.0) * linear!(2);

    // Constraint: x + y >= 1  =>  -(x + y) + 1 <= 0
    let constraint_expr = coeff!(-1.0) * linear!(1) + coeff!(-1.0) * linear!(2) + coeff!(1.0);

    let constraints = btreemap! {
        ConstraintID::from(1) => Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(1),
            constraint_expr.into(),
        ),
    };

    // Create instance
    let instance = Instance::new(Sense::Minimize, objective.into(), decision_variables, constraints)?;

    println!("\nSolving...");

    // Solve
    let solution = ScipAdapter::solve(&instance)?;

    println!("\nSolution:");
    println!("  x = {}", solution.state().entries().get(&1).unwrap_or(&0.0));
    println!("  y = {}", solution.state().entries().get(&2).unwrap_or(&0.0));
    println!("  Objective value = {}", solution.objective());
    println!("  Feasible = {}", solution.feasible());

    Ok(())
}
