//! # Rust SDK for OMMX (Open Mathematics prograMming eXchange)
//!
//! This crate provides native Rust types and operations for mathematical optimization problems.
//! It offers type-safe, high-performance implementations with convenient macros for expression building.
//!
//! ## [`Linear`], [`Quadratic`], [`Polynomial`], and [`Function`]
//!
//! These types represent mathematical expressions in optimization problems with different degree characteristics:
//!
//! - **[`Linear`]**: Up to degree 1 polynomials (linear terms + constant)
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
//! assert_eq!(quad_expr.degree(), 2);
//!
//! // Quadratic with only linear terms (no quadratic terms): 3*x1 + 2
//! let linear_only_quad = coeff!(3.0) * quadratic!(1) + coeff!(2.0);
//! assert_eq!(linear_only_quad.degree(), 1);
//!
//! // Functions can dynamically handle any degree
//! let linear_func = Function::from(linear_expr);  // Degree 1
//! assert_eq!(linear_func.degree(), 1);
//! let quad_func = Function::from(quad_expr);      // Degree 2
//! assert_eq!(quad_func.degree(), 2);
//! ```
//!
//! See also [`PolynomialBase`] which is a base for [`Linear`], [`Quadratic`], and [`Polynomial`].
//!
//! ## [`Bound`], [`Kind`], and [`DecisionVariable`]
//!
//! Decision variables define the unknowns in optimization problems. Each variable has a [`Kind`]
//! (continuous, binary, integer, etc.) and [`Bound`] (lower/upper limits).
//!
//! ```rust
//! use ommx::{DecisionVariable, Kind, Bound, VariableID, ATol};
//!
//! // Binary decision variable with ID 1
//! let binary_var = DecisionVariable::binary(VariableID::from(1));
//! assert_eq!(binary_var.kind(), Kind::Binary);
//! assert_eq!(binary_var.bound(), Bound::new(0.0, 1.0)?); // Default binary bound is [0, 1]
//!
//! // Integer variable with bound [0, 3]
//! let integer_var = DecisionVariable::integer(VariableID::from(2))
//!     .with_bound(Bound::new(0.0, 3.0)?, ATol::default())?;
//! assert_eq!(integer_var.kind(), Kind::Integer);
//! assert_eq!(integer_var.bound(), Bound::new(0.0, 3.0)?);
//!
//! // Continuous variable with ID 3
//! let continuous_var = DecisionVariable::continuous(VariableID::from(3));
//! assert_eq!(continuous_var.kind(), Kind::Continuous);
//! assert_eq!(continuous_var.bound(), Bound::unbounded()); // Default is unbounded (-inf, inf)
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## [`Constraint`]
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
//! ## [`Instance`]
//!
//! The [`Instance`] type represents a complete optimization problem with objective, variables,
//! and constraints. All variables used in the objective and constraints must be defined in the
//! decision variables map.
//!
//! ```rust
//! use ommx::{Instance, DecisionVariable, VariableID, Constraint, ConstraintID, Function, Sense, Linear, linear, coeff};
//! use maplit::btreemap;
//!
//! // Create decision variables
//! let decision_variables = btreemap! {
//!     VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
//!     VariableID::from(2) => DecisionVariable::continuous(VariableID::from(2)),
//! };
//!
//! // Create objective function: minimize x1 + 2*x2
//! let objective = Function::from(linear!(1) + coeff!(2.0) * linear!(2));
//!
//! // Create constraints
//! let constraints = btreemap! {
//!     // x1 + x2 = 1
//!     ConstraintID::from(1) => Constraint::equal_to_zero(
//!         ConstraintID::from(1),
//!         Function::from(linear!(1) + linear!(2) + Linear::from(coeff!(-1.0)))
//!     ),
//!     // x2 <= 5
//!     ConstraintID::from(2) => Constraint::less_than_or_equal_to_zero(
//!         ConstraintID::from(2),
//!         Function::from(linear!(2) + Linear::from(coeff!(-5.0)))
//!     ),
//! };
//!
//! // Create the instance
//! let instance = Instance::new(
//!     Sense::Minimize,
//!     objective,
//!     decision_variables,
//!     constraints,
//! )?;
//!
//! assert_eq!(instance.sense(), Sense::Minimize);
//! assert_eq!(instance.decision_variables().len(), 2);
//! assert_eq!(instance.constraints().len(), 2);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! The `new` method validates that all variable IDs used in the objective function and
//! constraints are defined in the decision variables map, returning an error if any
//! undefined variables are referenced.
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
//! use ommx::{Instance, DecisionVariable, VariableID, Constraint, ConstraintID, Function, Sense, Linear, Evaluate, ATol, linear, coeff};
//! use ommx::v1::State;
//! use maplit::btreemap;
//! use std::collections::HashMap;
//!
//! // Create an instance with variables and constraints
//! let decision_variables = btreemap! {
//!     VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1)),
//!     VariableID::from(2) => DecisionVariable::continuous(VariableID::from(2)),
//! };
//!
//! let objective = Function::from(linear!(1) + coeff!(2.0) * linear!(2));
//!
//! let constraints = btreemap! {
//!     // x1 + x2 <= 10
//!     ConstraintID::from(1) => Constraint::less_than_or_equal_to_zero(
//!         ConstraintID::from(1),
//!         Function::from(linear!(1) + linear!(2) + Linear::from(coeff!(-10.0)))
//!     ),
//!     // x1 >= 1 (as -x1 + 1 <= 0)
//!     ConstraintID::from(2) => Constraint::less_than_or_equal_to_zero(
//!         ConstraintID::from(2),
//!         Function::from(coeff!(-1.0) * linear!(1) + Linear::from(coeff!(1.0)))
//!     ),
//! };
//!
//! let instance = Instance::new(
//!     Sense::Minimize,
//!     objective,
//!     decision_variables,
//!     constraints,
//! )?;
//!
//! // Create a state with variable values that satisfy constraints
//! let state = State::from(HashMap::from([(1, 3.0), (2, 4.0)]));
//!
//! // Evaluate the instance to get a solution
//! let solution = instance.evaluate(&state, ATol::default())?;
//!
//! // Access solution properties
//! assert_eq!(*solution.objective(), 11.0); // 3 + 2*4 = 11
//! assert!(solution.feasible()); // All constraints satisfied
//!
//! // Check evaluated constraints
//! let evaluated_constraints = solution.evaluated_constraints();
//! assert_eq!(evaluated_constraints.len(), 2);
//!
//! // Constraint 1: x1 + x2 - 10 <= 0, evaluated to 3 + 4 - 10 = -3
//! let constraint1 = &evaluated_constraints[&ConstraintID::from(1)];
//! assert_eq!(constraint1.evaluated_value(), &-3.0);
//! assert!(constraint1.feasible()); // -3 <= 0 ✓
//!
//! // Constraint 2: -x1 + 1 <= 0, evaluated to -3 + 1 = -2
//! let constraint2 = &evaluated_constraints[&ConstraintID::from(2)];
//! assert_eq!(constraint2.evaluated_value(), &-2.0);
//! assert!(constraint2.feasible()); // -2 <= 0 ✓
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## [`Substitute`] trait
//!
//! The [`Substitute`] trait enables symbolic substitution of variables with expressions,
//! allowing for problem transformation and preprocessing.
//!
//! ```rust
//! use ommx::{Substitute, Function, Linear, linear, coeff, assign, ATol};
//! use approx::assert_abs_diff_eq;
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
//! assert_abs_diff_eq!(
//!   substituted,
//!   Function::from(linear!(2) + coeff!(3.0))  // Result: 2*(0.5*x2 + 1) + 1 = x2 + 3
//! );
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

// Re-export the dependencies
pub use ocipkg;
pub use prost::Message;

// Public modules
pub mod artifact;
#[cfg(feature = "remote-artifact")]
pub mod dataset;
pub mod logical_memory;
pub mod mps;
pub mod parse;
pub mod qplib;
pub mod random;

// Internal modules
mod atol;
mod bound;
mod coefficient;
mod constraint;
mod constraint_hints;
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
pub use constraint_hints::*;
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
