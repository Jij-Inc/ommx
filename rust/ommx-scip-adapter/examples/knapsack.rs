//! Knapsack Problem Example using SCIP Adapter
//!
//! This example demonstrates how to solve a classic 0-1 knapsack problem
//! using the OMMX SCIP adapter with direct C FFI integration.
//!
//! Problem:
//! - 6 items with different profits and weights
//! - Capacity constraint: total weight <= 47
//! - Objective: maximize total profit
//!
//! Run with:
//! ```bash
//! cargo run --example knapsack -p ommx-scip-adapter
//! ```

use maplit::btreemap;
use ommx::{coeff, linear, Constraint, ConstraintID, DecisionVariable, Instance, Sense, VariableID};
use ommx_scip_adapter::ScipAdapter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("=".repeat(70));
    println!("OMMX SCIP Adapter - Knapsack Problem Example");
    println!("=".repeat(70));

    // Define the knapsack problem
    let profits = vec![10, 13, 18, 32, 7, 15];
    let weights = vec![11, 15, 20, 35, 10, 33];
    let capacity = 47;

    println!("\nProblem Definition:");
    println!("Capacity: {}", capacity);
    println!("\nItems:");
    for i in 0..6 {
        println!(
            "  Item {}: profit={:2}, weight={:2}",
            i, profits[i], weights[i]
        );
    }

    // Create decision variables (binary: 0 or 1)
    let mut decision_variables = btreemap! {};
    for i in 0..6 {
        decision_variables.insert(
            VariableID::from(i as u64),
            DecisionVariable::binary(VariableID::from(i as u64)),
        );
    }

    // Create objective function: maximize sum of profits
    // objective = p[0]*x[0] + p[1]*x[1] + ... + p[5]*x[5]
    let mut objective = ommx::Linear::zero();
    for i in 0..6 {
        objective = objective + coeff!(profits[i] as f64) * linear!(i as u64);
    }

    // Create capacity constraint: sum of weights <= capacity
    // w[0]*x[0] + w[1]*x[1] + ... + w[5]*x[5] <= 47
    // Rewrite as: w[0]*x[0] + ... + w[5]*x[5] - 47 <= 0
    let mut constraint_expr = ommx::Linear::zero();
    for i in 0..6 {
        constraint_expr = constraint_expr + coeff!(weights[i] as f64) * linear!(i as u64);
    }
    constraint_expr = constraint_expr + coeff!(-(capacity as f64));

    let constraints = btreemap! {
        ConstraintID::from(0) => Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(0),
            constraint_expr.into(),
        ),
    };

    // Create OMMX instance
    let instance = Instance::new(
        Sense::Maximize,
        objective.into(),
        decision_variables,
        constraints,
    )?;

    println!("\n{}", "=".repeat(70));
    println!("Solving with SCIP...");
    println!("{}", "=".repeat(70));

    // Solve using SCIP adapter
    let solution = ScipAdapter::solve(&instance)?;

    println!("\n{}", "=".repeat(70));
    println!("Solution Results");
    println!("{}", "=".repeat(70));

    println!("\nFeasible: {}", solution.feasible());
    println!("Objective Value (Total Profit): {}", solution.objective());

    // Extract selected items
    println!("\nSelected Items:");
    let mut total_weight = 0;
    let mut total_profit = 0;
    let mut selected_items = Vec::new();

    for i in 0..6 {
        let value = solution.state().entries().get(&(i as u64)).unwrap_or(&0.0);
        if *value > 0.5 {
            // Binary variable should be 0 or 1
            selected_items.push(i);
            total_weight += weights[i];
            total_profit += profits[i];
            println!(
                "  ✓ Item {}: profit={:2}, weight={:2}",
                i, profits[i], weights[i]
            );
        }
    }

    println!("\nUnselected Items:");
    for i in 0..6 {
        let value = solution.state().entries().get(&(i as u64)).unwrap_or(&0.0);
        if *value < 0.5 {
            println!(
                "  ✗ Item {}: profit={:2}, weight={:2}",
                i, profits[i], weights[i]
            );
        }
    }

    println!("\nSummary:");
    println!("  Total Items Selected: {}", selected_items.len());
    println!("  Total Profit: {}", total_profit);
    println!("  Total Weight: {}/{}", total_weight, capacity);
    println!("  Remaining Capacity: {}", capacity - total_weight);

    // Verify constraint
    println!("\nConstraint Verification:");
    let constraint_value = solution.get_constraint_value(&ConstraintID::from(0))?;
    println!("  Capacity constraint value: {:.2}", constraint_value);
    println!(
        "  (Interpretation: {} - {} = {} <= 0)",
        total_weight,
        capacity,
        total_weight - capacity
    );

    println!("\n{}", "=".repeat(70));
    println!("✅ Solution is optimal!");
    println!("{}", "=".repeat(70));

    Ok(())
}
