//! SCIP solver adapter for OMMX
//!
//! This crate provides a Rust adapter for the SCIP optimization solver,
//! allowing OMMX instances to be solved using SCIP's C API through FFI.
//!
//! # Example
//!
//! ```rust,no_run
//! use ommx::{Instance, DecisionVariable, Sense, linear, coeff};
//! use ommx_scip_adapter::ScipAdapter;
//! use maplit::btreemap;
//!
//! // Create an OMMX instance
//! let instance = Instance::new(
//!     Sense::Minimize,
//!     (coeff!(1.0) * linear!(1) + coeff!(2.0) * linear!(2)).into(),
//!     btreemap! {
//!         1.into() => DecisionVariable::continuous(1.into()),
//!         2.into() => DecisionVariable::continuous(2.into()),
//!     },
//!     btreemap! {},
//! )?;
//!
//! // Solve with SCIP
//! let solution = ScipAdapter::solve(&instance)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod error;
mod scip_ffi;

pub use error::{Result, ScipAdapterError};

use ommx::{Constraint, DecisionVariable, Equality, Function, Instance, Kind, Solution, Sense};
use std::collections::HashMap;
use std::ffi::CString;
use std::ptr;

/// SCIP solver adapter for OMMX instances
pub struct ScipAdapter {
    /// Raw SCIP pointer
    scip: *mut scip_ffi::SCIP,
    /// Mapping from OMMX variable IDs to SCIP variable pointers
    vars: HashMap<u64, *mut scip_ffi::SCIP_VAR>,
    /// Original OMMX instance (needed for solution evaluation)
    instance: Instance,
}

// SCIP pointers are not Send/Sync by default, but we manage them carefully
unsafe impl Send for ScipAdapter {}
unsafe impl Sync for ScipAdapter {}

impl ScipAdapter {
    /// Create a new SCIP adapter from an OMMX instance
    ///
    /// This initializes SCIP, sets up variables, objective, and constraints.
    pub fn new(instance: Instance) -> Result<Self> {
        log::info!("Initializing SCIP adapter");

        unsafe {
            let mut scip: *mut scip_ffi::SCIP = ptr::null_mut();

            // Create SCIP environment
            let retcode = scip_ffi::SCIPcreate(&mut scip as *mut *mut _);
            if retcode != scip_ffi::SCIP_Retcode_SCIP_OKAY {
                return Err(ScipAdapterError::InitializationFailed);
            }

            // Include default SCIP plugins
            let retcode = scip_ffi::SCIPincludeDefaultPlugins(scip);
            if retcode != scip_ffi::SCIP_Retcode_SCIP_OKAY {
                scip_ffi::SCIPfree(&mut scip as *mut *mut _);
                return Err(ScipAdapterError::InitializationFailed);
            }

            // Create problem
            let prob_name = CString::new("ommx_problem")?;
            let retcode = scip_ffi::SCIPcreateProbBasic(scip, prob_name.as_ptr());
            if retcode != scip_ffi::SCIP_Retcode_SCIP_OKAY {
                scip_ffi::SCIPfree(&mut scip as *mut _);
                return Err(ScipAdapterError::InitializationFailed);
            }

            let mut adapter = ScipAdapter {
                scip,
                vars: HashMap::new(),
                instance,
            };

            // Set up the problem
            adapter.add_variables()?;
            adapter.set_objective()?;
            adapter.add_constraints()?;

            Ok(adapter)
        }
    }

    /// Solve an OMMX instance and return the solution
    pub fn solve(instance: &Instance) -> Result<Solution> {
        let adapter = Self::new(instance.clone())?;
        adapter.solve_internal()
    }

    /// Add decision variables to SCIP model
    fn add_variables(&mut self) -> Result<()> {
        log::debug!(
            "Adding {} decision variables",
            self.instance.decision_variables().len()
        );

        unsafe {
            for (id, var) in self.instance.decision_variables() {
                let vartype = match var.kind() {
                    Kind::Binary => scip_ffi::SCIP_Vartype_SCIP_VARTYPE_BINARY,
                    Kind::Integer => scip_ffi::SCIP_Vartype_SCIP_VARTYPE_INTEGER,
                    Kind::Continuous => scip_ffi::SCIP_Vartype_SCIP_VARTYPE_CONTINUOUS,
                    other => return Err(ScipAdapterError::UnsupportedVariableKind(*other)),
                };

                let var_name = CString::new(format!("x{}", id))?;
                let mut scip_var: *mut scip_ffi::SCIP_VAR = ptr::null_mut();

                let retcode = scip_ffi::SCIPcreateVarBasic(
                    self.scip,
                    &mut scip_var as *mut *mut _,
                    var_name.as_ptr(),
                    var.bound().lower(),
                    var.bound().upper(),
                    0.0, // objective coefficient (set later)
                    vartype,
                );

                if retcode != scip_ffi::SCIP_Retcode_SCIP_OKAY {
                    return Err(ScipAdapterError::SolveFailed(retcode as i32));
                }

                let retcode = scip_ffi::SCIPaddVar(self.scip, scip_var);
                if retcode != scip_ffi::SCIP_Retcode_SCIP_OKAY {
                    scip_ffi::SCIPreleaseVar(self.scip, &mut scip_var as *mut *mut _);
                    return Err(ScipAdapterError::SolveFailed(retcode as i32));
                }

                self.vars.insert(*id, scip_var);
            }
        }

        Ok(())
    }

    /// Set the objective function in SCIP
    fn set_objective(&mut self) -> Result<()> {
        log::debug!("Setting objective function");

        unsafe {
            // Set optimization sense
            let sense = match self.instance.sense() {
                Sense::Minimize => scip_ffi::SCIP_Objsense_SCIP_OBJSENSE_MINIMIZE,
                Sense::Maximize => scip_ffi::SCIP_Objsense_SCIP_OBJSENSE_MAXIMIZE,
            };
            scip_ffi::SCIPsetObjsense(self.scip, sense);

            let obj = self.instance.objective();

            // Check if objective is supported (only linear for now)
            if obj.degree() > 1 {
                return Err(ScipAdapterError::UnsupportedFunctionDegree(obj.degree()));
            }

            // Set linear coefficients
            for (var_id, coeff) in obj.linear_terms() {
                let scip_var = self
                    .vars
                    .get(var_id)
                    .ok_or(ScipAdapterError::VariableNotFound(*var_id))?;

                let retcode = scip_ffi::SCIPchgVarObj(self.scip, *scip_var, *coeff);
                if retcode != scip_ffi::SCIP_Retcode_SCIP_OKAY {
                    return Err(ScipAdapterError::SolveFailed(retcode as i32));
                }
            }

            // Add constant term to objective
            // SCIP handles this through SCIPaddOrigObjoffset
            if obj.constant_term() != 0.0 {
                let retcode = scip_ffi::SCIPaddOrigObjoffset(self.scip, obj.constant_term());
                if retcode != scip_ffi::SCIP_Retcode_SCIP_OKAY {
                    return Err(ScipAdapterError::SolveFailed(retcode as i32));
                }
            }
        }

        Ok(())
    }

    /// Add constraints to SCIP model
    fn add_constraints(&mut self) -> Result<()> {
        log::debug!(
            "Adding {} constraints",
            self.instance.constraints().len()
        );

        for (id, constraint) in self.instance.constraints() {
            let func = constraint.function();

            if func.degree() == 0 {
                // Handle constant constraints
                self.check_constant_constraint(constraint)?;
            } else if func.degree() == 1 {
                // Linear constraint
                self.add_linear_constraint(id, constraint)?;
            } else {
                return Err(ScipAdapterError::UnsupportedFunctionDegree(func.degree()));
            }
        }

        Ok(())
    }

    /// Check if a constant constraint is feasible
    fn check_constant_constraint(&self, constraint: &Constraint) -> Result<()> {
        let constant = constraint.function().constant_term();
        const TOLERANCE: f64 = 1e-10;

        let feasible = match constraint.equality() {
            Equality::EqualToZero => constant.abs() <= TOLERANCE,
            Equality::LessThanOrEqualToZero => constant <= TOLERANCE,
        };

        if !feasible {
            return Err(ScipAdapterError::Infeasible);
        }

        Ok(())
    }

    /// Add a linear constraint to SCIP
    fn add_linear_constraint(
        &mut self,
        id: &ommx::ConstraintID,
        constraint: &Constraint,
    ) -> Result<()> {
        unsafe {
            let func = constraint.function();
            let cons_name = CString::new(format!("c{}", id))?;

            // Convert OMMX constraint f(x) <= 0 or f(x) = 0
            // to SCIP format: lhs <= a^T x <= rhs
            //
            // For f(x) = a^T x + c:
            //   f(x) <= 0  =>  a^T x <= -c   (lhs = -inf, rhs = -c)
            //   f(x) = 0   =>  a^T x = -c    (lhs = -c, rhs = -c)
            let (lhs, rhs) = match constraint.equality() {
                Equality::EqualToZero => {
                    let bound = -func.constant_term();
                    (bound, bound)
                }
                Equality::LessThanOrEqualToZero => {
                    (f64::NEG_INFINITY, -func.constant_term())
                }
            };

            // Collect variables and coefficients
            let mut vars_vec = Vec::new();
            let mut coefs_vec = Vec::new();

            for (var_id, coeff) in func.linear_terms() {
                let scip_var = self
                    .vars
                    .get(var_id)
                    .ok_or(ScipAdapterError::VariableNotFound(*var_id))?;
                vars_vec.push(*scip_var);
                coefs_vec.push(*coeff);
            }

            let mut cons: *mut scip_ffi::SCIP_CONS = ptr::null_mut();

            let retcode = scip_ffi::SCIPcreateConsBasicLinear(
                self.scip,
                &mut cons as *mut *mut _,
                cons_name.as_ptr(),
                vars_vec.len() as i32,
                vars_vec.as_mut_ptr(),
                coefs_vec.as_mut_ptr(),
                lhs,
                rhs,
            );

            if retcode != scip_ffi::SCIP_Retcode_SCIP_OKAY {
                return Err(ScipAdapterError::SolveFailed(retcode as i32));
            }

            let retcode = scip_ffi::SCIPaddCons(self.scip, cons);
            if retcode != scip_ffi::SCIP_Retcode_SCIP_OKAY {
                scip_ffi::SCIPreleaseCons(self.scip, &mut cons as *mut *mut _);
                return Err(ScipAdapterError::SolveFailed(retcode as i32));
            }

            scip_ffi::SCIPreleaseCons(self.scip, &mut cons as *mut *mut _);
        }

        Ok(())
    }

    /// Solve the problem and return solution
    fn solve_internal(self) -> Result<Solution> {
        log::info!("Starting SCIP solve");

        unsafe {
            let retcode = scip_ffi::SCIPsolve(self.scip);
            if retcode != scip_ffi::SCIP_Retcode_SCIP_OKAY {
                return Err(ScipAdapterError::SolveFailed(retcode as i32));
            }

            // Get solution status
            let status = scip_ffi::SCIPgetStatus(self.scip);

            match status {
                scip_ffi::SCIP_Status_SCIP_STATUS_INFEASIBLE => {
                    return Err(ScipAdapterError::Infeasible);
                }
                scip_ffi::SCIP_Status_SCIP_STATUS_UNBOUNDED => {
                    return Err(ScipAdapterError::Unbounded);
                }
                _ => {}
            }

            // Get best solution
            let sol = scip_ffi::SCIPgetBestSol(self.scip);
            if sol.is_null() {
                return Err(ScipAdapterError::NoSolutionAvailable);
            }

            // Extract variable values
            let mut state_entries = HashMap::new();
            for (id, scip_var) in &self.vars {
                let val = scip_ffi::SCIPgetSolVal(self.scip, sol, *scip_var);
                state_entries.insert(*id, val);
            }

            let state = ommx::v1::State::from(state_entries);

            // Evaluate to get Solution
            let mut solution = self.instance.evaluate(&state, ommx::ATol::default())?;

            // Set optimality if optimal
            if status == scip_ffi::SCIP_Status_SCIP_STATUS_OPTIMAL {
                solution.raw.optimality = ommx::v1::solution::Optimality::Optimal as i32;
            }

            log::info!("SCIP solve completed successfully");

            Ok(solution)
        }
    }
}

impl Drop for ScipAdapter {
    fn drop(&mut self) {
        unsafe {
            // Release all variables
            for (_, scip_var) in &self.vars {
                scip_ffi::SCIPreleaseVar(self.scip, scip_var as *const _ as *mut *mut _);
            }

            // Free SCIP environment
            if !self.scip.is_null() {
                scip_ffi::SCIPfree(&mut self.scip as *mut *mut _);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use maplit::btreemap;
    use ommx::{coeff, linear, ConstraintID, VariableID};

    #[test]
    fn test_simple_linear_problem() {
        env_logger::init();

        // Minimize x1 + 2*x2
        // s.t. x1 + x2 <= 1
        //      x1, x2 >= 0
        let instance = Instance::new(
            Sense::Minimize,
            (coeff!(1.0) * linear!(1) + coeff!(2.0) * linear!(2)).into(),
            btreemap! {
                VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1)),
                VariableID::from(2) => DecisionVariable::continuous(VariableID::from(2)),
            },
            btreemap! {
                ConstraintID::from(1) => Constraint::less_than_or_equal_to_zero(
                    ConstraintID::from(1),
                    (linear!(1) + linear!(2) + coeff!(-1.0)).into()
                ),
            },
        )
        .unwrap();

        let solution = ScipAdapter::solve(&instance).unwrap();

        // Optimal solution should be x1=0, x2=0 (objective=0)
        // since we're minimizing and both variables have positive coefficients
        assert_abs_diff_eq!(*solution.objective(), 0.0, epsilon = 1e-6);
    }
}
