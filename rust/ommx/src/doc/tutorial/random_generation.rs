//! # Random Generation
//!
//! This tutorial demonstrates how to generate random optimization problems and functions
//! using the OMMX Rust API.
//!
//! ## Random Deterministic Generation
//!
//! OMMX provides a `random_deterministic` function for creating reproducible random objects:
//!
//! ```rust,no_run
//! use ommx::random;
//! use ommx::v1::Linear;
//!
//! // Generate a random linear function
//! let linear: Linear = random::random_deterministic(random::LinearParameters {
//!     num_terms: 5,
//!     max_id: 10,
//! });
//!
//! // The function will be the same every time with the same parameters
//! let linear2: Linear = random::random_deterministic(random::LinearParameters {
//!     num_terms: 5,
//!     max_id: 10,
//! });
//! assert_eq!(linear, linear2);
//! ```
//!
//! ## Generating Random Linear Functions
//!
//! You can generate random linear functions with specific parameters:
//!
//! ```rust,no_run
//! use ommx::random;
//! use ommx::v1::Linear;
//!
//! // Create parameters for the random linear function
//! let params = random::LinearParameters {
//!     num_terms: 5,
//!     max_id: 10,
//! };
//!
//! // Generate a random linear function with the specified parameters
//! let linear: Linear = random::random_deterministic(params);
//!
//! // The function will have terms with variable IDs between 1 and 10
//! assert!(linear.terms.len() <= 5);
//! ```
//!
//! ## Generating Random Quadratic Functions
//!
//! Similarly, you can generate random quadratic functions:
//!
//! ```rust,no_run
//! use ommx::random;
//! use ommx::v1::Quadratic;
//!
//! // Create parameters for the random quadratic function
//! let params = random::QuadraticParameters {
//!     num_terms: 5,
//!     max_id: 10,
//! };
//!
//! // Generate a random quadratic function with the specified parameters
//! let quadratic: Quadratic = random::random_deterministic(params);
//!
//! // The function will have quadratic terms with the specified parameters
//! assert!(quadratic.rows.len() <= 5);
//! ```
//!
//! ## Generating Random Optimization Problems
//!
//! You can generate complete random optimization problems:
//!
//! ```rust,no_run
//! use ommx::random;
//! use ommx::v1::Instance;
//!
//! // Create parameters for the random instance
//! let params = random::InstanceParameters {
//!     num_constraints: 3,
//!     num_terms: 5,
//!     max_degree: 1,
//!     max_id: 10,
//!     ..Default::default()
//! };
//!
//! // Generate a random instance with the specified parameters
//! let instance: Instance = random::random_deterministic(params);
//!
//! // The instance will have the specified number of variables and constraints
//! assert!(instance.decision_variables.len() > 0);
//! assert_eq!(instance.constraints.len(), 3);
//! ```
//!
//! ## Generating Random States
//!
//! You can generate random states for evaluating functions:
//!
//! ```rust,no_run
//! use ommx::random;
//! use ommx::v1::State;
//!
//! // Create parameters for the random state
//! let params = random::StateParameters {
//!     num_entries: 5,
//!     max_id: 10,
//!     ..Default::default()
//! };
//!
//! // Generate a random state with the specified parameters
//! let state: State = random::random_deterministic(params);
//!
//! // The state will have entries with values
//! assert_eq!(state.entries.len(), 5);
//! ```
//!
//! ## Practical Example: Generating Random QUBO Models
//!
//! Here's an example of generating a random QUBO (Quadratic Unconstrained Binary Optimization) model:
//!
//! ```rust,no_run
//! use ommx::random;
//! use ommx::v1::{Quadratic, Instance, DecisionVariable, Function, instance::Sense, Bound};
//!
//! // Create parameters for a random QUBO function
//! let params = random::QuadraticParameters {
//!     num_terms: 15,
//!     max_id: 10,
//! };
//!
//! // Generate a random quadratic function
//! let quadratic: Quadratic = random::random_deterministic(params);
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
