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
//! use ommx::v1::{Instance, DecisionVariable, Function, Linear, Constraint, Equality, Bound};
//!
//! // Create a new instance
//! let mut instance = Instance::default();
//!
//! // Add decision variables
//! let mut x1 = DecisionVariable::default();
//! x1.id = 1;
//! x1.name = Some("x1".to_string());
//! let mut bound1 = Bound::default();
//! bound1.lower = 0.0;
//! bound1.upper = 10.0;
//! x1.bound = Some(bound1);
//! instance.decision_variables.push(x1);
//!
//! let mut x2 = DecisionVariable::default();
//! x2.id = 2;
//! x2.name = Some("x2".to_string());
//! let mut bound2 = Bound::default();
//! bound2.lower = 0.0;
//! bound2.upper = 10.0;
//! x2.bound = Some(bound2);
//! instance.decision_variables.push(x2);
//!
//! // Add constraints: x1 + 2*x2 - 15 <= 0
//! let mut constraint = Constraint::default();
//! constraint.id = 1;
//! constraint.name = Some("constraint1".to_string());
//! constraint.equality = Equality::LessThanOrEqualToZero as i32;
//! 
//! // Create a function for the constraint: x1 + 2*x2 - 15
//! let linear_func = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) - 15.0;
//! let mut function = Function::default();
//! function.function = Some(ommx::v1::function::Function::Linear(linear_func));
//! constraint.function = Some(function);
//! instance.constraints.push(constraint);
//!
//! // Set objective function
//! let linear_obj = Linear::single_term(1, 3.0) + Linear::single_term(2, 4.0);
//! let mut obj_function = Function::default();
//! obj_function.function = Some(ommx::v1::function::Function::Linear(linear_obj));
//! instance.objective = Some(obj_function);
//! instance.sense = ommx::v1::instance::Sense::Maximize as i32;
//! ```
//!
//! ## Defining Decision Variables
//!
//! Decision variables represent the unknowns in an optimization problem:
//!
//! ```rust
//! use ommx::v1::{DecisionVariable, Bound};
//!
//! // Create a continuous variable with bounds [0, 10]
//! let mut x1 = DecisionVariable::default();
//! x1.id = 1;
//! x1.name = Some("x1".to_string());
//! let mut bound1 = Bound::default();
//! bound1.lower = 0.0;
//! bound1.upper = 10.0;
//! x1.bound = Some(bound1);
//!
//! // Create a binary variable (0 or 1)
//! let mut y1 = DecisionVariable::default();
//! y1.id = 2;
//! y1.name = Some("y1".to_string());
//! let mut bound2 = Bound::default();
//! bound2.lower = 0.0;
//! bound2.upper = 1.0;
//! y1.bound = Some(bound2);
//! y1.is_integer = true;
//!
//! // Create an integer variable with bounds [0, 5]
//! let mut z1 = DecisionVariable::default();
//! z1.id = 3;
//! z1.name = Some("z1".to_string());
//! let mut bound3 = Bound::default();
//! bound3.lower = 0.0;
//! bound3.upper = 5.0;
//! z1.bound = Some(bound3);
//! z1.is_integer = true;
//! ```
//!
//! ## Adding Constraints
//!
//! Constraints define the feasible region of the optimization problem:
//!
//! ```rust
//! use ommx::v1::{Constraint, Equality, Function, Linear};
//!
//! // Create a constraint: x1 + 2*x2 - 15 <= 0
//! let mut constraint = Constraint::default();
//! constraint.id = 1;
//! constraint.name = Some("constraint1".to_string());
//! constraint.equality = Equality::LessThanOrEqualToZero as i32;
//! 
//! // Create a function for the constraint: x1 + 2*x2 - 15
//! let linear_func = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) - 15.0;
//! let mut function = Function::default();
//! function.function = Some(ommx::v1::function::Function::Linear(linear_func));
//! constraint.function = Some(function);
//!
//! // Create an equality constraint: x1 - x2 - 5 = 0
//! let mut eq_constraint = Constraint::default();
//! eq_constraint.id = 2;
//! eq_constraint.name = Some("constraint2".to_string());
//! eq_constraint.equality = Equality::EqualToZero as i32;
//! 
//! // Create a function for the constraint: x1 - x2 - 5
//! let eq_linear_func = Linear::single_term(1, 1.0) + Linear::single_term(2, -1.0) - 5.0;
//! let mut eq_function = Function::default();
//! eq_function.function = Some(ommx::v1::function::Function::Linear(eq_linear_func));
//! eq_constraint.function = Some(eq_function);
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
//! let linear_obj = Linear::single_term(1, 3.0) + Linear::single_term(2, 4.0);
//! let mut obj_function = Function::default();
//! obj_function.function = Some(ommx::v1::function::Function::Linear(linear_obj));
//! instance.objective = Some(obj_function);
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
//! use ommx::v1::{Instance, DecisionVariable, Function, Linear, Constraint, Equality, Bound};
//!
//! // Create a new instance
//! let mut instance = Instance::default();
//!
//! // Add decision variables: x1, x2
//! let mut x1 = DecisionVariable::default();
//! x1.id = 1;
//! x1.name = Some("x1".to_string());
//! let mut bound1 = Bound::default();
//! bound1.lower = 0.0;
//! x1.bound = Some(bound1);
//! instance.decision_variables.push(x1);
//!
//! let mut x2 = DecisionVariable::default();
//! x2.id = 2;
//! x2.name = Some("x2".to_string());
//! let mut bound2 = Bound::default();
//! bound2.lower = 0.0;
//! x2.bound = Some(bound2);
//! instance.decision_variables.push(x2);
//!
//! // Add constraints:
//! // x1 + 2*x2 - 10 <= 0
//! let mut c1 = Constraint::default();
//! c1.id = 1;
//! c1.name = Some("c1".to_string());
//! c1.equality = Equality::LessThanOrEqualToZero as i32;
//! 
//! // Create a function for the constraint: x1 + 2*x2 - 10
//! let linear_func1 = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) - 10.0;
//! let mut function1 = Function::default();
//! function1.function = Some(ommx::v1::function::Function::Linear(linear_func1));
//! c1.function = Some(function1);
//! instance.constraints.push(c1);
//!
//! // 3*x1 + x2 - 15 <= 0
//! let mut c2 = Constraint::default();
//! c2.id = 2;
//! c2.name = Some("c2".to_string());
//! c2.equality = Equality::LessThanOrEqualToZero as i32;
//! 
//! // Create a function for the constraint: 3*x1 + x2 - 15
//! let linear_func2 = Linear::single_term(1, 3.0) + Linear::single_term(2, 1.0) - 15.0;
//! let mut function2 = Function::default();
//! function2.function = Some(ommx::v1::function::Function::Linear(linear_func2));
//! c2.function = Some(function2);
//! instance.constraints.push(c2);
//!
//! // Set objective: maximize 4*x1 + 3*x2
//! let linear_obj = Linear::single_term(1, 4.0) + Linear::single_term(2, 3.0);
//! let mut obj_function = Function::default();
//! obj_function.function = Some(ommx::v1::function::Function::Linear(linear_obj));
//! instance.objective = Some(obj_function);
//! instance.sense = ommx::v1::instance::Sense::Maximize as i32;
//! ```
