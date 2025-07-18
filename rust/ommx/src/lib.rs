//! # Rust SDK for OMMX (Open Mathematics prograMming eXchange)
//!
//! This crate provides native Rust types and operations for mathematical optimization problems.
//! It offers type-safe, high-performance implementations with convenient macros for expression building.
//!
//! ## [`Linear`], [`Quadratic`], [`Polynomial`], and [`Function`]
//!
//! These types represent mathematical expressions in optimization problems with different degree characteristics:
//!
//! - **[`Linear`]**: Fixed degree 1 polynomials (linear terms + constant)
//! - **[`Quadratic`]**: Up to degree 2 polynomials (may contain only linear terms, no quadratic terms required)
//! - **[`Function`]**: Dynamic degree handling, can represent any polynomial degree at runtime
//!
//! Use the convenience macros [`linear!`], [`quadratic!`], [`coeff!`], and [`monomial!`] for easy expression building.
//!
//! ```rust
//! use ommx::{Linear, Quadratic, Function, linear, quadratic, coeff};
//!
//! // Linear expressions: 2*x1 + 3*x2 + 5 (fixed degree 1)
//! let linear_expr = coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2) + coeff!(5.0);
//!
//! // Quadratic expressions: x1*x2 + 2*x1 + 1 (up to degree 2)
//! let quad_expr = coeff!(1.0) * quadratic!(1, 2) + coeff!(2.0) * quadratic!(1) + coeff!(1.0);
//!
//! // Quadratic with only linear terms (no quadratic terms): 3*x1 + 2
//! let linear_only_quad = coeff!(3.0) * quadratic!(1) + coeff!(2.0);
//!
//! // Functions can dynamically handle any degree
//! let linear_func = Function::from(linear_expr);  // Degree 1
//! let quad_func = Function::from(quad_expr);      // Degree 2
//! ```
//!
//! ## [`Bound`], [`Kind`], and [`DecisionVariable`]
//!
//! Decision variables define the unknowns in optimization problems. Each variable has a [`Kind`]
//! (continuous, binary, integer, etc.) and [`Bound`] (lower/upper limits).
//!
//! ```rust
//! use ommx::{DecisionVariable, Kind, Bound, VariableID};
//!
//! // Create different types of variables
//! let continuous_var = DecisionVariable::continuous(VariableID::from(1));
//! let binary_var = DecisionVariable::binary(VariableID::from(2));
//! let integer_var = DecisionVariable::integer(VariableID::from(3));
//!
//! // Access variable properties
//! let _kind = continuous_var.kind(); // Returns &Kind
//! let _bound = binary_var.bound();   // Returns &Bound
//!
//! // Create bounds
//! let bounded = Bound::new(0.0, 100.0)?;
//! let binary_bound = Bound::of_binary();
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## [`Constraint`] and [`RemovedConstraint`]
//!
//! Constraints define the feasible region of optimization problems. Constraints can be equality
//! or inequality types, and can be temporarily removed while preserving their definition.
//!
//! ```rust
//! use ommx::{Constraint, ConstraintID, Function, Linear, linear, coeff};
//!
//! // Create constraints: x1 + x2 <= 10 (as x1 + x2 - 10 <= 0)
//! let constraint_expr = coeff!(1.0) * linear!(1) + coeff!(1.0) * linear!(2) + Linear::from(coeff!(-10.0));
//! let constraint = Constraint::less_than_or_equal_to_zero(
//!     ConstraintID::from(1),
//!     Function::from(constraint_expr)
//! );
//!
//! // Equality constraint: x1 - x2 = 0 (as f(x) = 0)
//! let eq_expr = coeff!(1.0) * linear!(1) - coeff!(1.0) * linear!(2);
//! let eq_constraint = Constraint::equal_to_zero(
//!     ConstraintID::from(2),
//!     Function::from(eq_expr)
//! );
//! ```
//!
//! ## [`Instance`] and [`ParametricInstance`]
//!
//! The [`Instance`] type represents a complete optimization problem with objective, variables,
//! and constraints. [`ParametricInstance`] allows symbolic parameters for problem families.
//!
//! ```rust
//! use ommx::{Function, linear, coeff, Sense};
//!
//! // Create objective functions
//! let objective = Function::from(coeff!(1.0) * linear!(1) + coeff!(2.0) * linear!(2));
//!
//! // Instances can be built using the new method with appropriate maps
//! // See the mps and random modules for complete instance creation examples
//! assert_eq!(Sense::Minimize, Sense::Minimize);
//! ```
//!
//! ## [`Evaluate`] trait
//!
//! The [`Evaluate`] trait allows evaluation of expressions and functions given variable assignments.
//! This is essential for solution verification and constraint checking.
//!
//! ```rust
//! use ommx::{Evaluate, Function, linear, coeff, ATol};
//! use ommx::v1::State;
//! use std::collections::HashMap;
//!
//! // Create a function: 2*x1 + 3*x2
//! let func = Function::from(coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2));
//!
//! // Create variable assignments
//! let state = State::from(HashMap::from([(1, 4.0), (2, 5.0)]));
//!
//! // Evaluate: 2*4 + 3*5 = 23
//! let result = func.evaluate(&state, ATol::default())?;
//! assert_eq!(result, 23.0);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## [`Solution`] and [`SampleSet`]
//!
//! Solutions represent the results of optimization, including variable values, objective value,
//! and solution metadata. [`SampleSet`] contains multiple solutions for stochastic methods.
//!
//! ```rust
//! use ommx::{Solution, Sense};
//! use std::collections::BTreeMap;
//!
//! // Create a solution with objective value
//! let decision_variables = BTreeMap::new();
//! let constraints = BTreeMap::new();
//! let solution = Solution::new(42.0, constraints, decision_variables, Sense::Minimize);
//!
//! // Access solution properties
//! assert_eq!(*solution.objective(), 42.0);
//! assert!(solution.feasible()); // Check constraint feasibility
//! 
//! // Solutions contain evaluated variables and constraints for verification
//! ```
//!
//! ## [`Substitute`] trait
//!
//! The [`Substitute`] trait enables symbolic substitution of variables with expressions,
//! allowing for problem transformation and preprocessing.
//!
//! ```rust
//! use ommx::{Substitute, Linear, linear, coeff, assign};
//!
//! // Original expression: 2*x1 + 1
//! let expr = coeff!(2.0) * linear!(1) + Linear::one();
//!
//! // Substitute x1 = 0.5*x2 + 1
//! let assignments = assign! {
//!     1 <- coeff!(0.5) * linear!(2) + Linear::one()
//! };
//!
//! let substituted = expr.substitute_acyclic(&assignments)?;
//! // Result: 2*(0.5*x2 + 1) + 1 = x2 + 3
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//!
//! File I/O and Artifacts
//! ----------------------
//! OMMX supports loading optimization instances from standard formats like MPS and QPLIB.
//! See the [`mps`] and [`qplib`] modules for detailed file format support.
//!
//! ### OMMX Artifacts
//!
//! OMMX Artifacts are OCI-compatible containers for storing and sharing optimization instances.
//!
//! - Create an artifact with a random instance
//!
//!   ```no_run
//!   use ommx::{artifact::{Builder, InstanceAnnotations}, random::{random_deterministic, InstanceParameters}};
//!
//!   # fn main() -> anyhow::Result<()> {
//!   // Generate a random linear programming instance
//!   let instance = random_deterministic(InstanceParameters::default_lp());
//!
//!   // Create artifact builder for local file
//!   let mut builder = Builder::new_archive_unnamed("random_lp.ommx".into())?;
//!
//!   // Add metadata annotations
//!   let mut annotations = InstanceAnnotations::default();
//!   annotations.set_title("Random LP Instance".to_string());
//!   annotations.set_created(chrono::Local::now());
//!   
//!   // Add instance to artifact
//!   builder.add_instance(instance, annotations)?;
//!   let _artifact = builder.build()?;
//!   # Ok(()) }
//!   ```
//!
//! - Create and push artifacts to container registries
//!
//!   ```no_run
//!   use ocipkg::ImageName;
//!   use ommx::{artifact::{Builder, InstanceAnnotations}, random::{random_deterministic, InstanceParameters}};
//!
//!   # fn main() -> anyhow::Result<()> {
//!   let instance = random_deterministic(InstanceParameters::default_lp());
//!
//!   // Create builder for remote registry
//!   let mut builder = Builder::new(
//!       ImageName::parse("ghcr.io/jij-inc/ommx/example:latest")?
//!   )?;
//!
//!   // Add artifact metadata
//!   builder.add_source(&url::Url::parse("https://github.com/Jij-Inc/ommx")?);
//!   builder.add_description("Example optimization instance".to_string());
//!
//!   // Add instance with annotations
//!   let mut annotations = InstanceAnnotations::default();
//!   annotations.set_title("Example Instance".to_string());
//!   annotations.set_created(chrono::Local::now());
//!   builder.add_instance(instance, annotations)?;
//!
//!   // Build and push to registry
//!   let mut artifact = builder.build()?;
//!   artifact.push()?;
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
pub mod parse;
pub mod qplib;
pub mod random;

// Internal modules
mod atol;
mod bound;
mod coefficient;
mod constraint;
mod decision_variable;
mod evaluate;
mod format;
mod function;
mod infeasible_detected;
mod instance;
mod macros;
mod polynomial_base;
mod sample_set;
mod sampled;
mod solution;
mod substitute;

pub use atol::*;
pub use bound::*;
pub use coefficient::*;
pub use constraint::*;
pub use decision_variable::*;
pub use evaluate::Evaluate;
pub use function::*;
pub use infeasible_detected::*;
pub use instance::*;
pub use parse::*;
pub use polynomial_base::*;
pub use sample_set::*;
pub use sampled::*;
pub use solution::*;
pub use substitute::*;

/// Module created from `ommx.v1` proto files
#[allow(clippy::doc_overindented_list_items)] // prost breaks markdown indents
pub mod v1 {
    include!("ommx.v1.rs");
}

mod v1_ext {
    mod constraint;
    mod decision_variable;
    mod function;
    mod instance;
    mod linear;
    mod parameter;
    mod parametric_instance;
    mod polynomial;
    mod quadratic;
    mod sample_set;
    mod solution;
    mod state;
}
