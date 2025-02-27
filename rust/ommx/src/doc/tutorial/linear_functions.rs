//! # Linear Functions
//!
//! This tutorial demonstrates how to create and manipulate linear functions using the OMMX Rust API.
//!
//! ## Creating Linear Functions
//!
//! There are several ways to create linear functions in OMMX:
//!
//! ### Method 1: Using single_term
//!
//! ```rust
//! use ommx::v1::Linear;
//!
//! // Create a linear function `x1 + 2 x2 + 3`
//! let linear = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
//! ```
//!
//! ### Method 2: Using an iterator
//!
//! ```rust
//! use ommx::v1::Linear;
//!
//! // Create a linear function `x1 + 2 x2 + 3`
//! let linear = Linear::new([(1, 1.0), (2, 2.0)].into_iter(), 3.0);
//! ```
//!
//! ### Method 3: Starting with an empty function and adding terms
//!
//! ```rust
//! use ommx::v1::Linear;
//!
//! // Create a linear function `x1 + 2 x2 + 3`
//! let mut linear = Linear::default();
//! linear.constant = 3.0;
//! // Note: Term cannot be created directly, so use Linear::single_term to add terms
//! linear = linear + Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0);
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
//! ## Manipulating Linear Functions
//!
//! Linear functions can be combined using arithmetic operations:
//!
//! ```rust
//! use ommx::v1::Linear;
//!
//! // Create two linear functions
//! let linear1 = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
//! let linear2 = Linear::single_term(2, 1.0) + Linear::single_term(3, 3.0) + 2.0;
//!
//! // Add linear functions
//! let linear_sum = linear1.clone() + linear2.clone();
//! // Result: x1 + 3 x2 + 3 x3 + 5
//!
//! // Multiply a linear function by a scalar
//! let linear_scaled = linear1.clone() * 2.0;
//! // Result: 2 x1 + 4 x2 + 6
//! ```
//!
//! ## Serialization and Deserialization
//!
//! Linear functions can be serialized and deserialized using Protocol Buffers:
//!
//! ```rust
//! use ommx::v1::Linear;
//! use prost::Message;
//!
//! // Create a linear function
//! let linear = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
//!
//! // Serialize to bytes
//! let mut buf = Vec::new();
//! linear.encode(&mut buf).unwrap();
//!
//! // Deserialize from bytes
//! let decoded_linear = Linear::decode(buf.as_slice()).unwrap();
//! ```
