//! # Random Generation
//!
//! This tutorial demonstrates how to generate random optimization problems and functions
//! using the OMMX Rust API.
//!
//! ## Random Deterministic Generation
//!
//! OMMX provides a `random_deterministic` function for creating reproducible random objects:
//!
//! ```rust
//! use ommx::random;
//! use ommx::v1::Linear;
//!
//! // Generate a random linear function with seed 42
//! let seed = 42;
//! let num_terms = 5;
//! let linear = random::linear::random_deterministic(seed, num_terms, None);
//!
//! // The function will be the same every time with the same seed
//! let linear2 = random::linear::random_deterministic(seed, num_terms, None);
//! assert_eq!(linear, linear2);
//! ```
//!
//! ## Generating Random Linear Functions
//!
//! You can generate random linear functions with specific parameters:
//!
//! ```rust
//! use ommx::random;
//! use ommx::random::linear::LinearParams;
//! use ommx::v1::Linear;
//!
//! // Create parameters for the random linear function
//! let params = LinearParams {
//!     min_id: 1,
//!     max_id: 10,
//!     min_coefficient: -5.0,
//!     max_coefficient: 5.0,
//!     min_constant: -10.0,
//!     max_constant: 10.0,
//! };
//!
//! // Generate a random linear function with 5 terms and the specified parameters
//! let seed = 42;
//! let num_terms = 5;
//! let linear = random::linear::random_deterministic(seed, num_terms, Some(params));
//!
//! // The function will have 5 terms with coefficients between -5.0 and 5.0,
//! // variable IDs between 1 and 10, and a constant between -10.0 and 10.0
//! assert!(linear.terms.len() <= num_terms);
//! ```
//!
//! ## Generating Random Quadratic Functions
//!
//! Similarly, you can generate random quadratic functions:
//!
//! ```rust
//! use ommx::random;
//! use ommx::random::quadratic::QuadraticParams;
//! use ommx::v1::Quadratic;
//!
//! // Create parameters for the random quadratic function
//! let params = QuadraticParams {
//!     min_id: 1,
//!     max_id: 10,
//!     min_q_coefficient: -5.0,
//!     max_q_coefficient: 5.0,
//!     min_linear_coefficient: -5.0,
//!     max_linear_coefficient: 5.0,
//!     min_constant: -10.0,
//!     max_constant: 10.0,
//!     symmetric: true,
//! };
//!
//! // Generate a random quadratic function with 5 quadratic terms, 3 linear terms,
//! // and the specified parameters
//! let seed = 42;
//! let num_q_terms = 5;
//! let num_linear_terms = 3;
//! let quadratic = random::quadratic::random_deterministic(
//!     seed, num_q_terms, num_linear_terms, Some(params)
//! );
//!
//! // The function will have quadratic and linear terms with the specified parameters
//! assert!(quadratic.rows.len() <= num_q_terms);
//! if let Some(linear) = &quadratic.linear {
//!     assert!(linear.terms.len() <= num_linear_terms);
//! }
//! ```
//!
//! ## Generating Random Optimization Problems
//!
//! You can generate complete random optimization problems:
//!
//! ```rust,no_run
//! use ommx::random;
//! use ommx::v1::{Instance, instance::Sense};
//!
//! // Generate a random instance with 5 variables, 3 constraints
//! let seed = 42;
//! let num_vars = 5;
//! let num_constraints = 3;
//! let num_objective_terms = 4;
//! let num_constraint_terms = 2;
//! let instance = random::random_instance(
//!     seed, num_vars, num_constraints, num_objective_terms, num_constraint_terms, None
//! );
//!
//! // The instance will have the specified number of variables and constraints
//! assert_eq!(instance.decision_variables.len(), num_vars as usize);
//! assert_eq!(instance.constraints.len(), num_constraints as usize);
//! ```
//!
//! ## Generating Random States
//!
//! You can generate random states for evaluating functions:
//!
//! ```rust
//! use ommx::random;
//! use ommx::random::state::StateParams;
//! use ommx::v1::State;
//!
//! // Create parameters for the random state
//! let params = StateParams {
//!     min_id: 1,
//!     max_id: 10,
//!     min_value: 0.0,
//!     max_value: 10.0,
//! };
//!
//! // Generate a random state with 5 variables and the specified parameters
//! let seed = 42;
//! let num_vars = 5;
//! let state = random::state::random_deterministic(seed, num_vars, Some(params));
//!
//! // The state will have 5 variables with values between 0.0 and 10.0
//! assert_eq!(state.entries.len(), num_vars as usize);
//! for (_, value) in state.entries.iter() {
//!     assert!(*value >= 0.0 && *value <= 10.0);
//! }
//! ```
//!
//! ## Practical Example: Generating Random QUBO Models
//!
//! Here's an example of generating a random QUBO (Quadratic Unconstrained Binary Optimization) model:
//!
//! ```rust,no_run
//! use ommx::random;
//! use ommx::random::quadratic::QuadraticParams;
//! use ommx::v1::{Quadratic, Instance, DecisionVariable, Function, instance::Sense, Bound};
//!
//! // Create parameters for a random QUBO function
//! let params = QuadraticParams {
//!     min_id: 1,
//!     max_id: 10,
//!     min_q_coefficient: -5.0,
//!     max_q_coefficient: 5.0,
//!     min_linear_coefficient: -5.0,
//!     max_linear_coefficient: 5.0,
//!     min_constant: -10.0,
//!     max_constant: 10.0,
//!     symmetric: true,
//! };
//!
//! // Generate a random quadratic function
//! let seed = 42;
//! let num_q_terms = 15;
//! let num_linear_terms = 10;
//! let quadratic = random::quadratic::random_deterministic(
//!     seed, num_q_terms, num_linear_terms, Some(params)
//! );
//!
//! // Create a QUBO instance
//! let mut instance = Instance::default();
//! instance.sense = Sense::Minimize as i32;
//!
//! // Add binary decision variables
//! for i in 1..=10 {
//!     let mut var = DecisionVariable::default();
//!     var.id = i;
//!     var.name = Some(format!("x{}", i));
//!     let mut bound = Bound::default();
//!     bound.lower = 0.0;
//!     bound.upper = Some(1.0);
//!     var.bound = Some(bound);
//!     var.is_integer = true;
//!     instance.decision_variables.push(var);
//! }
//!
//! // Set the objective function
//! let mut function = Function::default();
//! function.function = Some(ommx::v1::function::Function::Quadratic(quadratic));
//! instance.objective = Some(function);
//!
//! // The instance now represents a random QUBO problem
//! assert_eq!(instance.decision_variables.len(), 10);
//! assert!(instance.objective.is_some());
//! ```
