//! # Optimization Modeling
//!
//! This tutorial demonstrates how to model optimization problems using the OMMX Rust API.
//!
//! ## Creating an Instance
//!
//! An `Instance` represents a complete optimization problem, including decision variables,
//! constraints, and an objective function.
//!
//! ```rust
//! use ommx::v1::{Instance, DecisionVariable, Function, Linear, Constraint, constraint::Sense};
//!
//! // Create a new instance
//! let mut instance = Instance::default();
//!
//! // Add decision variables
//! let mut x1 = DecisionVariable::default();
//! x1.id = 1;
//! x1.name = "x1".to_string();
//! x1.lower_bound = 0.0;
//! x1.upper_bound = 10.0;
//! instance.decision_variables.push(x1);
//!
//! let mut x2 = DecisionVariable::default();
//! x2.id = 2;
//! x2.name = "x2".to_string();
//! x2.lower_bound = 0.0;
//! x2.upper_bound = 10.0;
//! instance.decision_variables.push(x2);
//!
//! // Add constraints
//! let mut constraint = Constraint::default();
//! constraint.id = 1;
//! constraint.name = "constraint1".to_string();
//! constraint.sense = Sense::LessThanOrEqual as i32;
//! constraint.rhs = 15.0;
//! constraint.function = Some(Function {
//!     function: Some(ommx::v1::function::Function::Linear(
//!         Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0)
//!     ))
//! });
//! instance.constraints.push(constraint);
//!
//! // Set objective function
//! instance.objective = Some(Function {
//!     function: Some(ommx::v1::function::Function::Linear(
//!         Linear::single_term(1, 3.0) + Linear::single_term(2, 4.0)
//!     ))
//! });
//! instance.sense = ommx::v1::instance::Sense::Maximize as i32;
//! ```
//!
//! ## Defining Decision Variables
//!
//! Decision variables represent the unknowns in an optimization problem:
//!
//! ```rust
//! use ommx::v1::DecisionVariable;
//!
//! // Create a continuous variable with bounds [0, 10]
//! let mut x1 = DecisionVariable::default();
//! x1.id = 1;
//! x1.name = "x1".to_string();
//! x1.lower_bound = 0.0;
//! x1.upper_bound = 10.0;
//!
//! // Create a binary variable (0 or 1)
//! let mut y1 = DecisionVariable::default();
//! y1.id = 2;
//! y1.name = "y1".to_string();
//! y1.lower_bound = 0.0;
//! y1.upper_bound = 1.0;
//! y1.is_integer = true;
//!
//! // Create an integer variable with bounds [0, 5]
//! let mut z1 = DecisionVariable::default();
//! z1.id = 3;
//! z1.name = "z1".to_string();
//! z1.lower_bound = 0.0;
//! z1.upper_bound = 5.0;
//! z1.is_integer = true;
//! ```
//!
//! ## Adding Constraints
//!
//! Constraints define the feasible region of the optimization problem:
//!
//! ```rust
//! use ommx::v1::{Constraint, constraint::Sense, Function, Linear};
//!
//! // Create a constraint: x1 + 2*x2 <= 15
//! let mut constraint = Constraint::default();
//! constraint.id = 1;
//! constraint.name = "constraint1".to_string();
//! constraint.sense = Sense::LessThanOrEqual as i32;
//! constraint.rhs = 15.0;
//! constraint.function = Some(Function {
//!     function: Some(ommx::v1::function::Function::Linear(
//!         Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0)
//!     ))
//! });
//!
//! // Create an equality constraint: x1 - x2 = 5
//! let mut eq_constraint = Constraint::default();
//! eq_constraint.id = 2;
//! eq_constraint.name = "constraint2".to_string();
//! eq_constraint.sense = Sense::Equal as i32;
//! eq_constraint.rhs = 5.0;
//! eq_constraint.function = Some(Function {
//!     function: Some(ommx::v1::function::Function::Linear(
//!         Linear::single_term(1, 1.0) + Linear::single_term(2, -1.0)
//!     ))
//! });
//! ```
//!
//! ## Setting the Objective Function
//!
//! The objective function defines what we want to optimize:
//!
//! ```rust
//! use ommx::v1::{Instance, Function, Linear};
//!
//! // Create a new instance
//! let mut instance = Instance::default();
//!
//! // Set a linear objective function: maximize 3*x1 + 4*x2
//! instance.objective = Some(Function {
//!     function: Some(ommx::v1::function::Function::Linear(
//!         Linear::single_term(1, 3.0) + Linear::single_term(2, 4.0)
//!     ))
//! });
//! instance.sense = ommx::v1::instance::Sense::Maximize as i32;
//!
//! // For minimization, use:
//! // instance.sense = ommx::v1::instance::Sense::Minimize as i32;
//! ```
//!
//! ## Multi-dimensional Decision Variables
//!
//! You can represent multi-dimensional decision variables using a flattening scheme:
//!
//! ```rust
//! use ommx::v1::DecisionVariable;
//!
//! // Create a 3x3 matrix of decision variables
//! let n = 3;
//! let mut variables = Vec::new();
//!
//! for i in 0..n {
//!     for j in 0..n {
//!         let mut var = DecisionVariable::default();
//!         // Flatten 2D indices to 1D
//!         var.id = (i * n + j + 1) as u64;
//!         var.name = format!("x_{}_{}", i, j);
//!         var.lower_bound = 0.0;
//!         var.upper_bound = 1.0;
//!         variables.push(var);
//!     }
//! }
//! ```
//!
//! ## Practical Example: Linear Programming
//!
//! Here's a complete example of a linear programming problem:
//!
//! ```rust
//! use ommx::v1::{Instance, DecisionVariable, Function, Linear, Constraint, constraint::Sense};
//!
//! // Create a new instance
//! let mut instance = Instance::default();
//!
//! // Add decision variables: x1, x2
//! let mut x1 = DecisionVariable::default();
//! x1.id = 1;
//! x1.name = "x1".to_string();
//! x1.lower_bound = 0.0;
//! instance.decision_variables.push(x1);
//!
//! let mut x2 = DecisionVariable::default();
//! x2.id = 2;
//! x2.name = "x2".to_string();
//! x2.lower_bound = 0.0;
//! instance.decision_variables.push(x2);
//!
//! // Add constraints:
//! // x1 + 2*x2 <= 10
//! let mut c1 = Constraint::default();
//! c1.id = 1;
//! c1.name = "c1".to_string();
//! c1.sense = Sense::LessThanOrEqual as i32;
//! c1.rhs = 10.0;
//! c1.function = Some(Function {
//!     function: Some(ommx::v1::function::Function::Linear(
//!         Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0)
//!     ))
//! });
//! instance.constraints.push(c1);
//!
//! // 3*x1 + x2 <= 15
//! let mut c2 = Constraint::default();
//! c2.id = 2;
//! c2.name = "c2".to_string();
//! c2.sense = Sense::LessThanOrEqual as i32;
//! c2.rhs = 15.0;
//! c2.function = Some(Function {
//!     function: Some(ommx::v1::function::Function::Linear(
//!         Linear::single_term(1, 3.0) + Linear::single_term(2, 1.0)
//!     ))
//! });
//! instance.constraints.push(c2);
//!
//! // Set objective: maximize 4*x1 + 3*x2
//! instance.objective = Some(Function {
//!     function: Some(ommx::v1::function::Function::Linear(
//!         Linear::single_term(1, 4.0) + Linear::single_term(2, 3.0)
//!     ))
//! });
//! instance.sense = ommx::v1::instance::Sense::Maximize as i32;
//! ```
