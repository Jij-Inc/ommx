//! # State and Evaluation
//!
//! This tutorial demonstrates how to use the `State` struct and the `Evaluate` trait
//! to evaluate functions and constraints in the OMMX Rust API.
//!
//! ## Creating and Using State
//!
//! The `State` struct represents an assignment of values to decision variables:
//!
//! ```rust
//! use ommx::v1::State;
//! use maplit::hashmap;
//!
//! // Create a state with x1 = 2.0, x2 = 3.0, x3 = 4.0
//! let state: State = hashmap! { 1 => 2.0, 2 => 3.0, 3 => 4.0 }.into();
//!
//! // Access values in the state
//! assert_eq!(state.values.get(&1), Some(&2.0));
//! assert_eq!(state.values.get(&2), Some(&3.0));
//! assert_eq!(state.values.get(&3), Some(&4.0));
//! ```
//!
//! ## Evaluating Linear Functions
//!
//! You can evaluate linear functions using the `Evaluate` trait:
//!
//! ```rust
//! use ommx::v1::{Linear, State};
//! use ommx::Evaluate;
//! use maplit::hashmap;
//!
//! // Create a linear function `x1 + 2 x2 + 3`
//! let linear = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
//!
//! // Create a state `x1 = 2.0, x2 = 3.0`
//! let state: State = hashmap! { 1 => 2.0, 2 => 3.0 }.into();
//!
//! // Evaluate the linear function with the state
//! let (value, used_ids) = linear.evaluate(&state).unwrap();
//! assert_eq!(value, 1.0 * 2.0 + 2.0 * 3.0 + 3.0); // 1*2 + 2*3 + 3 = 11
//! ```
//!
//! ## Evaluating Quadratic Functions
//!
//! Similarly, you can evaluate quadratic functions:
//!
//! ```rust
//! use ommx::v1::{Quadratic, Linear, State};
//! use ommx::Evaluate;
//! use maplit::hashmap;
//!
//! // Create a quadratic function `x1^2 + 2 x1 x2 + 3 x2^2 + 4 x1 + 5 x2 + 6`
//! let mut quadratic = Quadratic::default();
//!
//! // Add quadratic terms
//! quadratic.rows.push(1);
//! quadratic.columns.push(1);
//! quadratic.values.push(1.0); // x1^2
//!
//! quadratic.rows.push(1);
//! quadratic.columns.push(2);
//! quadratic.values.push(1.0); // x1*x2
//!
//! quadratic.rows.push(2);
//! quadratic.columns.push(1);
//! quadratic.values.push(1.0); // x2*x1
//!
//! quadratic.rows.push(2);
//! quadratic.columns.push(2);
//! quadratic.values.push(3.0); // x2^2
//!
//! // Add linear part
//! let linear = Linear::single_term(1, 4.0) + Linear::single_term(2, 5.0) + 6.0;
//! quadratic.linear = Some(linear);
//!
//! // Create a state `x1 = 2.0, x2 = 3.0`
//! let state: State = hashmap! { 1 => 2.0, 2 => 3.0 }.into();
//!
//! // Evaluate the quadratic function with the state
//! let (value, used_ids) = quadratic.evaluate(&state).unwrap();
//! // 2^2 + 2*2*3 + 3*3^2 + 4*2 + 5*3 + 6 = 4 + 12 + 27 + 8 + 15 + 6 = 72
//! assert_eq!(value, 72.0);
//! ```
//!
//! ## Evaluating Constraints
//!
//! You can also evaluate constraints to check if they are satisfied:
//!
//! ```rust
//! use ommx::v1::{Constraint, Equality, Function, Linear, State};
//! use ommx::Evaluate;
//! use maplit::hashmap;
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
//! // Create a state `x1 = 5.0, x2 = 4.0`
//! let state: State = hashmap! { 1 => 5.0, 2 => 4.0 }.into();
//!
//! // Evaluate the constraint function with the state
//! let (value, used_ids) = constraint.function.as_ref().unwrap().evaluate(&state).unwrap();
//! // 5 + 2*4 - 15 = 13 - 15 = -2, which is <= 0, so the constraint is satisfied
//! assert!(value <= 0.0);
//! ```
//!
//! ## Partial Evaluation
//!
//! You can perform partial evaluation when not all variables are present in the state:
//!
//! ```rust
//! use ommx::v1::{Linear, State};
//! use ommx::Evaluate;
//! use maplit::hashmap;
//!
//! // Create a linear function `x1 + 2 x2 + 3 x3 + 4`
//! let linear = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) +
//!              Linear::single_term(3, 3.0) + 4.0;
//!
//! // Create a partial state `x1 = 2.0, x3 = 4.0` (x2 is missing)
//! let partial_state: State = hashmap! { 1 => 2.0, 3 => 4.0 }.into();
//!
//! // Evaluate the linear function with the partial state
//! let (value, used_ids) = linear.evaluate(&partial_state).unwrap();
//! // 1*2 + 3*4 + 4 = 2 + 12 + 4 = 18
//! // Note: The term with x2 is not evaluated because x2 is not in the state
//! assert_eq!(value, 18.0);
//! assert!(used_ids.contains(&1));
//! assert!(!used_ids.contains(&2)); // x2 is not used
//! assert!(used_ids.contains(&3));
//! ```
//!
//! ## Sample-based Evaluation
//!
//! You can evaluate functions over multiple samples:
//!
//! ```rust
//! use ommx::v1::{Linear, State};
//! use ommx::Evaluate;
//! use maplit::hashmap;
//!
//! // Create a linear function `x1 + 2 x2 + 3`
//! let linear = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
//!
//! // Create multiple states
//! let state1: State = hashmap! { 1 => 1.0, 2 => 1.0 }.into();
//! let state2: State = hashmap! { 1 => 2.0, 2 => 2.0 }.into();
//! let state3: State = hashmap! { 1 => 3.0, 2 => 3.0 }.into();
//!
//! // Evaluate the linear function with each state
//! let (value1, _) = linear.evaluate(&state1).unwrap();
//! let (value2, _) = linear.evaluate(&state2).unwrap();
//! let (value3, _) = linear.evaluate(&state3).unwrap();
//!
//! // 1*1 + 2*1 + 3 = 6
//! assert_eq!(value1, 6.0);
//! // 1*2 + 2*2 + 3 = 9
//! assert_eq!(value2, 9.0);
//! // 1*3 + 2*3 + 3 = 12
//! assert_eq!(value3, 12.0);
//! ```
//!
//! ## Practical Example: Validating a Solution
//!
//! Here's an example of validating the optimal solution for a production planning problem:
//!
//! ```rust
//! use ommx::v1::{Instance, DecisionVariable, Function, Linear, Constraint, Equality, State, Bound};
//! use ommx::Evaluate;
//! use maplit::hashmap;
//!
//! // Create a production planning problem
//! let mut instance = Instance::default();
//!
//! // Add decision variables: x1, x2 (production quantities)
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
//! // Resource constraint: 2*x1 + x2 - 100 <= 0
//! let mut c1 = Constraint::default();
//! c1.id = 1;
//! c1.name = Some("resource".to_string());
//! c1.equality = Equality::LessThanOrEqualToZero as i32;
//!
//! // Create a function for the constraint: 2*x1 + x2 - 100
//! let linear_func1 = Linear::single_term(1, 2.0) + Linear::single_term(2, 1.0) - 100.0;
//! let mut function1 = Function::default();
//! function1.function = Some(ommx::v1::function::Function::Linear(linear_func1));
//! c1.function = Some(function1);
//! instance.constraints.push(c1);
//!
//! // Demand constraint: x1 - 10 >= 0 (equivalent to -x1 + 10 <= 0)
//! let mut c2 = Constraint::default();
//! c2.id = 2;
//! c2.name = Some("demand_x1".to_string());
//! c2.equality = Equality::LessThanOrEqualToZero as i32;
//!
//! // Create a function for the constraint: -x1 + 10
//! let linear_func2 = Linear::single_term(1, -1.0) + 10.0;
//! let mut function2 = Function::default();
//! function2.function = Some(ommx::v1::function::Function::Linear(linear_func2));
//! c2.function = Some(function2);
//! instance.constraints.push(c2);
//!
//! // Set objective: maximize 5*x1 + 3*x2 (profit)
//! let linear_obj = Linear::single_term(1, 5.0) + Linear::single_term(2, 3.0);
//! let mut obj_function = Function::default();
//! obj_function.function = Some(ommx::v1::function::Function::Linear(linear_obj));
//! instance.objective = Some(obj_function);
//! instance.sense = ommx::v1::instance::Sense::Maximize as i32;
//!
//! // Validate a proposed solution: x1 = 45, x2 = 10
//! let solution: State = hashmap! { 1 => 45.0, 2 => 10.0 }.into();
//!
//! // Check if all constraints are satisfied
//! let mut all_constraints_satisfied = true;
//! for constraint in &instance.constraints {
//!     let (value, _) = constraint.function.as_ref().unwrap().evaluate(&solution).unwrap();
//!     let satisfied = match constraint.equality {
//!         e if e == Equality::LessThanOrEqualToZero as i32 => value <= 0.0,
//!         e if e == Equality::EqualToZero as i32 => value.abs() < 1e-6,
//!         _ => panic!("Unknown constraint equality"),
//!     };
//!     if !satisfied {
//!         all_constraints_satisfied = false;
//!         break;
//!     }
//! }
//!
//! // Calculate objective value
//! let (obj_value, _) = instance.objective.as_ref().unwrap().evaluate(&solution).unwrap();
//!
//! // For this example, the solution x1 = 45, x2 = 10 should be feasible
//! // 2*45 + 10 - 100 = 0 <= 0 (resource constraint)
//! // -45 + 10 = -35 <= 0 (demand constraint)
//! // Objective value = 5*45 + 3*10 = 225 + 30 = 255
//! assert!(all_constraints_satisfied);
//! assert_eq!(obj_value, 255.0);
//! ```
