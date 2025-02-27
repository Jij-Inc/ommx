//! # Quadratic Functions
//!
//! This tutorial demonstrates how to create and manipulate quadratic functions using the OMMX Rust API.
//!
//! ## Creating Quadratic Functions
//!
//! There are several ways to create quadratic functions in OMMX:
//!
//! ### Method 1: Using the builder pattern
//!
//! ```rust
//! use ommx::v1::{Quadratic, Linear};
//!
//! // Create a quadratic function `x1^2 + 2 x1 x2 + 3 x2^2 + 4 x1 + 5 x2 + 6`
//! let quadratic = Quadratic::builder()
//!     .with_q_term(1, 1, 1.0)    // x1^2
//!     .with_q_term(1, 2, 1.0)    // x1 x2 (note: this adds 1.0 to both Q[1,2] and Q[2,1])
//!     .with_q_term(2, 2, 3.0)    // x2^2
//!     .with_linear(Linear::single_term(1, 4.0) + Linear::single_term(2, 5.0) + 6.0)
//!     .build();
//! ```
//!
//! ### Method 2: Using the default constructor and adding terms
//!
//! ```rust
//! use ommx::v1::{Quadratic, Linear, quadratic::QTerm};
//!
//! // Create a quadratic function `x1^2 + 2 x1 x2 + 3 x2^2 + 4 x1 + 5 x2 + 6`
//! let mut quadratic = Quadratic::default();
//!
//! // Add quadratic terms
//! let mut q_term1 = QTerm::default();
//! q_term1.row_id = 1;
//! q_term1.col_id = 1;
//! q_term1.value = 1.0;
//! quadratic.q_terms.push(q_term1);
//!
//! let mut q_term2 = QTerm::default();
//! q_term2.row_id = 1;
//! q_term2.col_id = 2;
//! q_term2.value = 1.0;
//! quadratic.q_terms.push(q_term2);
//!
//! let mut q_term3 = QTerm::default();
//! q_term3.row_id = 2;
//! q_term3.col_id = 1;
//! q_term3.value = 1.0;
//! quadratic.q_terms.push(q_term3);
//!
//! let mut q_term4 = QTerm::default();
//! q_term4.row_id = 2;
//! q_term4.col_id = 2;
//! q_term4.value = 3.0;
//! quadratic.q_terms.push(q_term4);
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
//! let quadratic = Quadratic::builder()
//!     .with_q_term(1, 1, 1.0)
//!     .with_q_term(1, 2, 1.0)
//!     .with_q_term(2, 2, 3.0)
//!     .with_linear(Linear::single_term(1, 4.0) + Linear::single_term(2, 5.0) + 6.0)
//!     .build();
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
//! // Create two quadratic functions
//! let quadratic1 = Quadratic::builder()
//!     .with_q_term(1, 1, 1.0)
//!     .with_linear(Linear::single_term(1, 2.0) + 3.0)
//!     .build();
//!
//! let quadratic2 = Quadratic::builder()
//!     .with_q_term(2, 2, 4.0)
//!     .with_linear(Linear::single_term(2, 5.0) + 6.0)
//!     .build();
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
//! // Create a quadratic function
//! let quadratic = Quadratic::builder()
//!     .with_q_term(1, 1, 1.0)
//!     .with_q_term(2, 2, 3.0)
//!     .build();
//!
//! // Serialize to bytes
//! let mut buf = Vec::new();
//! quadratic.encode(&mut buf).unwrap();
//!
//! // Deserialize from bytes
//! let decoded_quadratic = Quadratic::decode(buf.as_slice()).unwrap();
//! ```
