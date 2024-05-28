//! Rust SDK for OMMX (Open Mathematics prograMming eXchange)
//!
//! Messages defined by protobuf schema
//! ------------------------------------
//! OMMX defines several messages, and their Rust bindings are in the [`v1`] module.
//!
//! ### Examples
//!
//! - Create [`v1::Linear`] message in Rust, and serialize/deserialize it
//!
//!   ```rust
//!   use ommx::v1::{Linear, linear::Term};
//!   use prost::Message; // For `encode` and `decode` methods
//!
//!   // Create a linear function `x1 + 2 x2 + 3`
//!   let linear = Linear {
//!       terms: vec![ Term { id: 1, coefficient: 1.0 }, Term { id: 2, coefficient: 2.0 } ],
//!       constant: 3.0,
//!   };
//!
//!   // Serialize the message to a byte stream
//!   let mut buf = Vec::new();
//!   linear.encode(&mut buf).unwrap();
//!
//!   // Deserialize the byte stream back into a linear function message
//!   let decoded_linear = Linear::decode(buf.as_slice()).unwrap();
//!
//!   // Print the deserialized message
//!   println!("{:?}", decoded_linear);
//!   ```
//!
//! - [`Evaluate`] a [`v1::Linear`] with [`v1::State`] into `f64`
//!
//!   ```rust
//!   use ommx::{Evaluate, v1::{Linear, State, linear::Term}};
//!   use maplit::{hashmap, btreeset};
//!
//!   // Create a linear function `x1 + 2 x2 + 3`
//!   let linear = Linear {
//!       terms: vec![
//!           Term { id: 1, coefficient: 1.0 },
//!           Term { id: 2, coefficient: 2.0 }
//!       ],
//!       constant: 3.0,
//!   };
//!
//!   // Create a state `x1 = 4`, `x2 = 5`, and `x3 = 6`
//!   let state: State = hashmap! { 1 => 4.0, 2 => 5.0, 3 => 6.0 }.into();
//!
//!   // Evaluate the linear function with the state, and get the value and used variable ids
//!   let (value, used_ids) = linear.evaluate(&state).unwrap();
//!
//!   assert_eq!(value, 1.0 * 4.0 + 2.0 * 5.0 + 3.0);
//!   assert_eq!(used_ids, btreeset!{ 1, 2 }) // x3 is not used
//!   ```
//!
//! OMMX Artifact
//! --------------
//! OMMX defines a protobuf schema. Then the [`v1::Instance`] and [`v1::Solution`] are serialized as protobuf messages,
//! i.e. a byte stream satisfying the schema.
//! This is enough for in process communication, but not enough for out of process communication.
//! For storing message on local disk or sharing with other applications via cloud storage,
//! OMMX also defines a metadata for each message, such as who and when this message was created by which application, and so on,
//! and treats the pair of metadata and message as OCI Artifact.
//!

pub mod artifact;
pub mod random;
pub use prost::Message;
mod arbitrary;
mod convert;
mod evaluate;

pub use evaluate::Evaluate;

/// Module created from `ommx.v1` proto files
pub mod v1 {
    include!("ommx.v1.rs");
}
