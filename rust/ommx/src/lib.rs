//! Rust SDK for OMMX (Open Mathematics prograMming eXchange)
//!
//! OMMX Messages
//! --------------
//! OMMX defines several messages in protobuf schema, and their Rust bindings are in the [`v1`] module.
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
//!   let linear = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
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
//!   let linear = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
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
//! OMMX Artifact is an OCI Artifact, i.e. a container image with arbitrary content, storing the OMMX Messages.
//! It is useful for storing messages on local disk or sharing with others via container registry.
//!
//! ### Examples
//!
//! - Create an artifact as a file with an instance created by [`random::random_lp`]
//!
//!   ```no_run
//!   use ocipkg::ImageName;
//!   use ommx::{artifact::{Builder, InstanceAnnotations}, random::random_lp};
//!   use rand::SeedableRng;
//!
//!   # fn main() -> anyhow::Result<()> {
//!   // Create random LP instance to be saved into an artifact
//!   let lp = random_lp(&mut rand::thread_rng(), 5, 7);
//!
//!   // Builder for creating an artifact as a file (e.g. `random_lp_instance.ommx`)
//!   let mut builder = Builder::new_archive_unnamed("random_lp_instance.ommx".into())?;
//!
//!   // Add the instance with annotations
//!   let mut annotations = InstanceAnnotations::default();
//!   annotations.set_title("random_lp".to_string());
//!   annotations.set_created(chrono::Local::now());
//!   builder.add_instance(lp, annotations)?;
//!
//!   // Build the artifact
//!   let _artifact = builder.build()?;
//!   # Ok(()) }
//!   ```
//!
//! - Create an artifact on local registry, and then push it to remote registry (e.g. GitHub Container Registry)
//!
//!   ```no_run
//!   use ocipkg::ImageName;
//!   use ommx::{artifact::{Builder, InstanceAnnotations}, random::random_lp};
//!   use rand::SeedableRng;
//!
//!   # fn main() -> anyhow::Result<()> {
//!   // Create random LP instance to be saved into an artifact
//!   let lp = random_lp(&mut rand::thread_rng(), 5, 7);
//!
//!   // Builder for creating an artifact in local registry
//!   let mut builder = Builder::new(
//!       ImageName::parse("ghcr.io/jij-inc/ommx/random_lp_instance:testing")?
//!   )?;
//!
//!   // Add annotations for the artifact
//!   builder.add_source(&url::Url::parse("https://github.com/Jij-Inc/ommx")?);
//!   builder.add_description("Test artifact".to_string());
//!
//!   // Add the instance with annotations
//!   let mut annotations = InstanceAnnotations::default();
//!   annotations.set_title("random_lp".to_string());
//!   annotations.set_created(chrono::Local::now());
//!   builder.add_instance(lp, annotations)?;
//!
//!   // Build the artifact
//!   let mut artifact = builder.build()?;
//!
//!   // Push the artifact to remote registry
//!   artifact.push()?;
//!   # Ok(()) }
//!   ```
//!
//! - Pull an artifact from remote registry, and load the instance message
//!
//!   ```no_run
//!   use ocipkg::ImageName;
//!   use ommx::artifact::{Artifact, media_types};
//!
//!   # fn main() -> anyhow::Result<()> {
//!   let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/random_lp_instance:testing")?;
//!
//!   // Pull the artifact from remote registry
//!   let mut remote = Artifact::from_remote(image_name)?;
//!   let mut local = remote.pull()?;
//!
//!   // List the digest of instances
//!   for desc in local.get_layer_descriptors(&media_types::v1_instance())? {
//!       println!("{}", desc.digest());
//!   }
//!   # Ok(()) }
//!   ```
//!

// Re-export the dependencies
pub use ocipkg;
pub use prost::Message;

// Public modules
pub mod artifact;
pub mod dataset;
pub mod mps;
pub mod random;

// Internal modules
mod convert;
mod evaluate;

pub use evaluate::Evaluate;

/// Module created from `ommx.v1` proto files
pub mod v1 {
    include!("ommx.v1.rs");
}
