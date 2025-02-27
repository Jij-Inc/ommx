//! # Quadratic Functions
//!
//! This tutorial demonstrates how to create and manipulate quadratic functions using the OMMX Rust API.
//!
//! ## Creating Quadratic Functions
//!
//! There are several ways to create quadratic functions in OMMX:
//!
//! ### Method 1: Using direct field assignment
//!
//! ```rust
//! use ommx::v1::{Quadratic, Linear};
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
//! ```
//!
//! ### Method 2: Using multiplication syntax with clone()
//!
//! ```rust,no_run
//! use ommx::v1::{Quadratic, Linear, Function};
//!
//! // Create linear functions for x1 and x2
//! let x1 = Linear::single_term(1, 1.0);
//! let x2 = Linear::single_term(2, 1.0);
//!
//! // Create a quadratic function using multiplication: x1^2 + 2*x1*x2 + 3*x2^2 + 4*x1 + 5*x2 + 6
//! // Use clone() to avoid consuming the original Linear objects
//! let x1_squared = x1.clone() * x1.clone();
//! let x1_x2 = x1.clone() * x2.clone() * 2.0;
//! let x2_squared = x2.clone() * x2.clone() * 3.0;
//!
//! // Combine all quadratic terms
//! let quadratic_terms = x1_squared + x1_x2 + x2_squared;
//!
//! // Add linear part: 4*x1 + 5*x2 + 6
//! let linear_part = x1.clone() * 4.0 + x2.clone() * 5.0 + 6.0;
//!
//! // Create the final quadratic function with both quadratic and linear terms
//! let mut quadratic = quadratic_terms;
//! quadratic.linear = Some(linear_part);
//!
//! // Create a Function object from the quadratic
//! let mut function = Function::default();
//! function.function = Some(ommx::v1::function::Function::Quadratic(quadratic));
//! ```
//!
//! ## Evaluating Quadratic Functions
//!
//! You can evaluate quadratic functions using the `Evaluate` trait:
//!
//! ```rust
//! use ommx::v1::{Quadratic, Linear, State, Function};
//! use ommx::Evaluate;
//! use maplit::hashmap;
//!
//! // Create linear functions for x1 and x2
//! let x1 = Linear::single_term(1, 1.0);
//! let x2 = Linear::single_term(2, 1.0);
//!
//! // Create a quadratic function using multiplication: x1^2 + 2*x1*x2 + 3*x2^2 + 4*x1 + 5*x2 + 6
//! // Use clone() to avoid consuming the original Linear objects
//! let x1_squared = x1.clone() * x1.clone();
//! let x1_x2 = x1.clone() * x2.clone() * 2.0;
//! let x2_squared = x2.clone() * x2.clone() * 3.0;
//!
//! // Combine all quadratic terms
//! let quadratic_terms = x1_squared + x1_x2 + x2_squared;
//!
//! // Add linear part: 4*x1 + 5*x2 + 6
//! let linear_part = x1.clone() * 4.0 + x2.clone() * 5.0 + 6.0;
//!
//! // Create the final quadratic function with both quadratic and linear terms
//! let mut quadratic = quadratic_terms;
//! quadratic.linear = Some(linear_part);
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
//! ## Example: Creating a Quadratic Optimization Problem
//!
//! Here's an example of creating a quadratic optimization problem:
//!
//! ```rust,no_run
//! use ommx::v1::{Instance, DecisionVariable, Function, Linear, Quadratic, Bound};
//!
//! // Create an optimization problem
//! let mut instance = Instance::default();
//!
//! // Add decision variables: x1, x2
//! let mut x1_var = DecisionVariable::default();
//! x1_var.id = 1;
//! x1_var.name = Some("x1".to_string());
//! let mut bound1 = Bound::default();
//! bound1.lower = 0.0;
//! x1_var.bound = Some(bound1);
//! instance.decision_variables.push(x1_var);
//!
//! let mut x2_var = DecisionVariable::default();
//! x2_var.id = 2;
//! x2_var.name = Some("x2".to_string());
//! let mut bound2 = Bound::default();
//! bound2.lower = 0.0;
//! x2_var.bound = Some(bound2);
//! instance.decision_variables.push(x2_var);
//!
//! // Create a quadratic objective function: 2*x1^2 + x1*x2 + 3*x2^2 - 4*x1 - 5*x2
//! // First, create linear functions for x1 and x2
//! let x1 = Linear::single_term(1, 1.0);
//! let x2 = Linear::single_term(2, 1.0);
//!
//! // Create quadratic terms using multiplication syntax with clone()
//! let x1_squared = x1.clone() * x1.clone() * 2.0;  // 2*x1^2
//! let x1_x2 = x1.clone() * x2.clone();             // x1*x2
//! let x2_squared = x2.clone() * x2.clone() * 3.0;  // 3*x2^2
//!
//! // Combine all quadratic terms
//! let quadratic_terms = x1_squared + x1_x2 + x2_squared;
//!
//! // Add linear part: -4*x1 - 5*x2
//! let linear_part = x1.clone() * (-4.0) + x2.clone() * (-5.0);
//!
//! // Create the final quadratic function with both quadratic and linear terms
//! let mut quadratic = quadratic_terms;
//! quadratic.linear = Some(linear_part);
//!
//! // Set the objective function
//! let mut obj_function = Function::default();
//! obj_function.function = Some(ommx::v1::function::Function::Quadratic(quadratic));
//! instance.objective = Some(obj_function);
//! instance.sense = ommx::v1::instance::Sense::Minimize as i32;
//!
//! // Now you have a quadratic optimization problem
//! ```
//!
//! ## Serialization and Deserialization
//!
//! Quadratic functions can be serialized and deserialized using Protocol Buffers:
//!
//! ```rust
//! use ommx::v1::Quadratic;
//! use prost::Message;
//!
//! // Helper function to add a quadratic term
//! fn add_q_term(quadratic: &mut Quadratic, row: u64, col: u64, value: f64) {
//!     quadratic.rows.push(row);
//!     quadratic.columns.push(col);
//!     quadratic.values.push(value);
//! }
//!
//! // Create a quadratic function
//! let mut quadratic = Quadratic::default();
//! add_q_term(&mut quadratic, 1, 1, 1.0); // x1^2
//! add_q_term(&mut quadratic, 2, 2, 3.0); // x2^2
//!
//! // Serialize to bytes
//! let mut buf = Vec::new();
//! quadratic.encode(&mut buf).unwrap();
//!
//! // Deserialize from bytes
//! let decoded_quadratic = Quadratic::decode(buf.as_slice()).unwrap();
//! ```
