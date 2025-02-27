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
//! ### Method 2: Using a helper function
//!
//! ```rust
//! use ommx::v1::{Quadratic, Linear};
//!
//! // Helper function to add a quadratic term
//! fn add_q_term(quadratic: &mut Quadratic, row: u64, col: u64, value: f64) {
//!     quadratic.rows.push(row);
//!     quadratic.columns.push(col);
//!     quadratic.values.push(value);
//! }
//!
//! // Create a quadratic function `x1^2 + 2 x1 x2 + 3 x2^2 + 4 x1 + 5 x2 + 6`
//! let mut quadratic = Quadratic::default();
//!
//! // Add quadratic terms
//! add_q_term(&mut quadratic, 1, 1, 1.0); // x1^2
//! add_q_term(&mut quadratic, 1, 2, 1.0); // x1*x2
//! add_q_term(&mut quadratic, 2, 1, 1.0); // x2*x1
//! add_q_term(&mut quadratic, 2, 2, 3.0); // x2^2
//!
//! // Add linear part
//! quadratic.linear = Some(Linear::single_term(1, 4.0) + Linear::single_term(2, 5.0) + 6.0);
//! ```
//!
//! ## Evaluating Quadratic Functions
//!
//! You can evaluate quadratic functions using the `Evaluate` trait:
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
//! ## Manipulating Quadratic Functions
//!
//! Quadratic functions can be combined using arithmetic operations:
//!
//! ```rust
//! use ommx::v1::{Quadratic, Linear};
//!
//! // Helper function to add a quadratic term
//! fn add_q_term(quadratic: &mut Quadratic, row: u64, col: u64, value: f64) {
//!     quadratic.rows.push(row);
//!     quadratic.columns.push(col);
//!     quadratic.values.push(value);
//! }
//!
//! // Create two quadratic functions
//! let mut quadratic1 = Quadratic::default();
//! add_q_term(&mut quadratic1, 1, 1, 1.0); // x1^2
//! let linear1 = Linear::single_term(1, 2.0) + 3.0;
//! quadratic1.linear = Some(linear1);
//!
//! let mut quadratic2 = Quadratic::default();
//! add_q_term(&mut quadratic2, 2, 2, 4.0); // x2^2
//! let linear2 = Linear::single_term(2, 5.0) + 6.0;
//! quadratic2.linear = Some(linear2);
//!
//! // Add quadratic functions
//! let quadratic_sum = quadratic1.clone() + quadratic2.clone();
//!
//! // Multiply a quadratic function by a scalar
//! let quadratic_scaled = quadratic1.clone() * 2.0;
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
