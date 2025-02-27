//! # OMMX Artifacts
//!
//! This tutorial demonstrates how to work with OMMX artifacts using the Rust API.
//!
//! ## What are OMMX Artifacts?
//!
//! OMMX artifacts are OCI-compliant container images that store OMMX messages.
//! They provide a standardized way to save, share, and reuse optimization problems
//! and other OMMX messages.
//!
//! ## Creating and Saving Artifacts
//!
//! You can create and save OMMX artifacts using the `Builder`:
//!
//! ```rust,no_run
//! use ommx::artifact::Builder;
//! use ommx::v1::{Linear, Instance};
//! use std::path::Path;
//! use ocipkg::ImageName;
//!
//! // Create a linear function
//! let linear = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
//!
//! // Create an artifact builder
//! let image_name = ImageName::parse("localhost:5000/linear:latest").unwrap();
//! let mut builder = Builder::new(image_name).unwrap();
//!
//! // Add the linear function to the artifact
//! let mut instance = Instance::default();
//! let mut function = ommx::v1::Function::default();
//! function.function = Some(ommx::v1::function::Function::Linear(linear));
//! instance.objective = Some(function);
//! builder.add_instance(instance, ommx::artifact::InstanceAnnotations::default()).unwrap();
//!
//! // Save the artifact to a file
//! let path = Path::new("linear_artifact.oci");
//! builder.build().unwrap().save(path).unwrap();
//! ```
//!
//! ## Adding Metadata to Artifacts
//!
//! You can add metadata to artifacts to provide additional information:
//!
//! ```rust,no_run
//! use ommx::artifact::Builder;
//! use ommx::v1::{Linear, Instance, Function};
//! use std::path::Path;
//! use ocipkg::ImageName;
//!
//! // Create a linear function
//! let linear = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
//!
//! // Create an artifact builder with metadata
//! let image_name = ImageName::parse("localhost:5000/linear:latest").unwrap();
//! let mut builder = Builder::new(image_name).unwrap();
//! builder.add_annotation("description".to_string(), "A simple linear function".to_string());
//! builder.add_annotation("author".to_string(), "OMMX User".to_string());
//! builder.add_annotation("version".to_string(), "1.0".to_string());
//!
//! // Add the linear function to the artifact
//! let mut instance = Instance::default();
//! let mut function = Function::default();
//! function.function = Some(ommx::v1::function::Function::Linear(linear));
//! instance.objective = Some(function);
//! builder.add_instance(instance, ommx::artifact::InstanceAnnotations::default()).unwrap();
//!
//! // Save the artifact to a file
//! let path = Path::new("linear_artifact_with_metadata.oci");
//! builder.build().unwrap().save(path).unwrap();
//! ```
//!
//! ## Pushing Artifacts to a Registry
//!
//! You can push artifacts to an OCI registry:
//!
//! ```rust,no_run
//! use ommx::artifact::Builder;
//! use ommx::v1::{Linear, Instance, Function};
//! use ocipkg::ImageName;
//!
//! // Create a linear function
//! let linear = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) + 3.0;
//!
//! // Create an artifact builder
//! let image_name = ImageName::parse("localhost:5000/linear:latest").unwrap();
//! let mut builder = Builder::new(image_name).unwrap();
//!
//! // Add the linear function to the artifact
//! let mut instance = Instance::default();
//! let mut function = Function::default();
//! function.function = Some(ommx::v1::function::Function::Linear(linear));
//! instance.objective = Some(function);
//! builder.add_instance(instance, ommx::artifact::InstanceAnnotations::default()).unwrap();
//!
//! // Build and push the artifact to a registry
//! let mut artifact = builder.build().unwrap();
//! artifact.push().unwrap();
//! ```
//!
//! ## Loading Artifacts from Files
//!
//! You can load artifacts from files:
//!
//! ```rust,no_run
//! use ommx::artifact::Artifact;
//! use ommx::v1::{Linear, Instance};
//! use prost::Message;
//! use std::path::Path;
//!
//! // Load the artifact from a file
//! let path = Path::new("linear_artifact.oci");
//! let mut artifact = Artifact::from_oci_archive(path).unwrap();
//!
//! // Get the instances from the artifact
//! let instances = artifact.get_instances().unwrap();
//! for (desc, instance) in instances {
//!     println!("Instance: {:?}", instance);
//! }
//! ```
//!
//! ## Pulling Artifacts from a Registry
//!
//! You can pull artifacts from an OCI registry:
//!
//! ```rust,no_run
//! use ommx::artifact::Artifact;
//! use ommx::v1::{Linear, Instance};
//! use prost::Message;
//! use ocipkg::ImageName;
//!
//! // Pull the artifact from a registry
//! let reference = "localhost:5000/linear:latest";
//! let image_name = ImageName::parse(reference).unwrap();
//! let mut artifact = Artifact::from_remote(image_name).unwrap();
//! let mut local_artifact = artifact.pull().unwrap();
//!
//! // Get the instances from the artifact
//! let instances = local_artifact.get_instances().unwrap();
//! for (desc, instance) in instances {
//!     println!("Instance: {:?}", instance);
//! }
//! ```
//!
//! ## Practical Example: Saving and Loading an Optimization Problem
//!
//! Here's a complete example of saving and loading an optimization problem:
//!
//! ```rust,no_run
//! use ommx::artifact::{Builder, Artifact, InstanceAnnotations};
//! use ommx::v1::{Instance, DecisionVariable, Function, Linear, Constraint, Equality, Bound};
//! use prost::Message;
//! use std::path::Path;
//! use ocipkg::ImageName;
//!
//! // Create an optimization problem
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
//! let linear_func = Linear::single_term(1, 1.0) + Linear::single_term(2, 2.0) - 10.0;
//! let mut function = Function::default();
//! function.function = Some(ommx::v1::function::Function::Linear(linear_func));
//! c1.function = Some(function);
//! instance.constraints.push(c1);
//!
//! // Set objective: maximize 4*x1 + 3*x2
//! let linear_obj = Linear::single_term(1, 4.0) + Linear::single_term(2, 3.0);
//! let mut obj_function = Function::default();
//! obj_function.function = Some(ommx::v1::function::Function::Linear(linear_obj));
//! instance.objective = Some(obj_function);
//! instance.sense = ommx::v1::instance::Sense::Maximize as i32;
//!
//! // Create an artifact builder
//! let image_name = ImageName::parse("localhost:5000/lp_problem:latest").unwrap();
//! let mut builder = Builder::new(image_name).unwrap();
//! builder.add_annotation("description".to_string(), "Linear programming example".to_string());
//! builder.add_annotation("author".to_string(), "OMMX User".to_string());
//!
//! // Add the instance to the artifact
//! let annotations = InstanceAnnotations::default();
//! builder.add_instance(instance, annotations).unwrap();
//!
//! // Save the artifact to a file
//! let path = Path::new("lp_problem.oci");
//! builder.build().unwrap().save(path).unwrap();
//!
//! // Later, load the artifact
//! let mut artifact = Artifact::from_oci_archive(path).unwrap();
//!
//! // Get the instances from the artifact
//! let instances = artifact.get_instances().unwrap();
//! for (desc, loaded_instance) in instances {
//!     println!("Loaded instance: {:?}", loaded_instance);
//! }
//! ```
//!
//! ## Sharing Artifacts with Others
//!
//! OMMX artifacts provide a standardized way to share optimization problems with others.
//! By pushing artifacts to a registry, you can easily share your problems with collaborators
//! or the wider community.
//!
//! ```rust,no_run
//! use ommx::artifact::{Builder, InstanceAnnotations};
//! use ommx::v1::Instance;
//! use ocipkg::ImageName;
//!
//! // Create an instance (optimization problem)
//! let instance = Instance::default(); // In practice, this would be a real problem
//!
//! // Create an artifact builder with metadata
//! let image_name = ImageName::parse("ghcr.io/my-username/my-problem:v1.0").unwrap();
//! let mut builder = Builder::new(image_name).unwrap();
//! builder.add_annotation("description".to_string(), "My optimization problem".to_string());
//! builder.add_annotation("author".to_string(), "OMMX User".to_string());
//! builder.add_annotation("version".to_string(), "1.0".to_string());
//!
//! // Add the instance to the artifact
//! let annotations = InstanceAnnotations::default();
//! builder.add_instance(instance, annotations).unwrap();
//!
//! // Build and push the artifact to a public registry
//! let mut artifact = builder.build().unwrap();
//! artifact.push().unwrap();
//!
//! // Now others can pull and use your problem
//! ```
