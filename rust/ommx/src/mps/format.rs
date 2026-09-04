use crate::decision_variable::Kind as DecisionVariableKind;
use crate::{
    mps::ObjSense, Coefficient, ConstraintID, Equality, Instance, Sense, VariableID, VariableIDSet,
};
use std::collections::BTreeMap;
use std::io::Write;

pub(crate) const OBJ_NAME: &str = "OBJ";
pub(crate) const CONSTR_PREFIX: &str = "OMMX_CONSTR_";
pub(crate) const VAR_PREFIX: &str = "OMMX_VAR_";

/// Writes out the instance in MPS format to the specified `Write`r.
///
/// This function does not automatically Gzip the output -- that is the
/// responsibility of the Write implementation.
///
/// Decision variables used by the objective or active constraints must be
/// Binary, Integer, or Continuous. Polynomial objectives and regular
/// constraints of degree at most two are supported. Active Indicator, OneHot,
/// and SOS1 constraints must be lowered before export. Unused variables of
/// other kinds are accepted and omitted as described below.
///
/// ## Information Loss and Filtering
///
/// The active objective, active regular constraints, and used decision-variable
/// kinds and bounds are exported. `description.name` is used as the MPS problem
/// name. Other Instance state is not represented, including the remaining
/// description fields, parameters, annotations, output objective, named
/// functions, dependencies, fixed values, and modeling labels.
///
/// **Removed Constraints**: All `removed_constraints` are completely ignored
/// and not written to the MPS file. The MPS format cannot represent the
/// concept of removed constraints, so this information is lost during export.
///
/// **Variable Filtering**: Only decision variables used by the objective or
/// active constraints are written. Variables defined in `decision_variables`
/// but not used by the active model are omitted. This includes variables used
/// only by removed constraints, because those constraints are not exported.
pub fn format<W: Write>(instance: &Instance, out: &mut W) -> crate::Result<()> {
    super::preflight(instance)?;
    write_beginning(instance, out)?;
    write_rows(instance, out)?;
    write_columns(instance, out)?;
    write_rhs(instance, out)?;
    write_bounds(instance, out)?;
    write_quadobj(instance, out)?;
    write_qcmatrix(instance, out)?;
    writeln!(out, "ENDATA\n")?;
    Ok(())
}

/// Converts the instance to a string in MPS format via [`format()`].
pub fn to_string(instance: &Instance) -> crate::Result<String> {
    let mut buffer = Vec::new();
    format(instance, &mut buffer)?;
    Ok(String::from_utf8(buffer).unwrap())
}

fn write_beginning<W: Write>(instance: &Instance, out: &mut W) -> crate::Result<()> {
    let name = instance
        .description
        .clone()
        .and_then(|descr| descr.name)
        .unwrap_or(String::from("Converted OMMX problem"));
    let obj_sense = match instance.sense() {
        Sense::Maximize => ObjSense::Max,
        Sense::Minimize => ObjSense::Min,
    };
    writeln!(out, "NAME {name}")?;
    writeln!(out, "OBJSENSE {obj_sense}")?;
    Ok(())
}

fn write_rows<W: Write>(instance: &Instance, out: &mut W) -> crate::Result<()> {
    writeln!(out, "ROWS")?;
    // each line must be ` Kind  constr_name`, and include objective
    writeln!(out, " N OBJ")?;
    // ommx instances are always <= 0 or = 0, so `Kind` will always be either N or L.
    for (id, constr) in instance.constraints().iter() {
        let kind = match constr.equality {
            Equality::LessThanOrEqualToZero => "L",
            // assuming EqualToZero when unspecified. Error instead?
            _ => "E",
        };
        let name = constr_name(*id);
        writeln!(out, " {kind} {name}")?;
    }
    Ok(())
}

#[derive(Default)]
struct IntorgTracker {
    intorg_block: bool,
    counter: u64,
}

impl IntorgTracker {
    fn intorg<W: Write>(&mut self, out: &mut W) -> crate::Result<()> {
        // only print marker if not already in INTORG block
        if !self.intorg_block {
            self.intorg_block = true;
            writeln!(out, "    MARK{}   'MARKER'      'INTORG'", self.counter)?;
            self.counter += 1;
        }
        Ok(())
    }
    fn intend<W: Write>(&mut self, out: &mut W) -> crate::Result<()> {
        // only print marker if in INTORG block
        if self.intorg_block {
            self.intorg_block = false;
            writeln!(out, "    MARK{}   'MARKER'      'INTEND'", self.counter)?;
            self.counter += 1;
        }
        Ok(())
    }
}

fn write_columns<W: Write>(instance: &Instance, out: &mut W) -> crate::Result<()> {
    writeln!(out, "COLUMNS")?;
    let mut marker_tracker = IntorgTracker::default();

    // Collect all linear terms from objective and constraints
    // Structure: VariableID -> Vec<(row_name, coefficient)>
    let mut variable_entries: BTreeMap<VariableID, Vec<(String, Coefficient)>> = BTreeMap::new();

    // Collect linear terms from objective function
    for (var_id, coeff) in instance
        .objective()
        .linear_terms()
        .expect("MPS function preflight verified a polynomial objective")
    {
        variable_entries
            .entry(var_id)
            .or_default()
            .push((OBJ_NAME.to_string(), coeff));
    }

    // Collect linear terms from constraints
    for (constr_id, constr) in instance.constraints().iter() {
        let row_name = constr_name(*constr_id);
        for (var_id, coeff) in constr
            .function()
            .linear_terms()
            .expect("MPS function preflight verified polynomial constraints")
        {
            variable_entries
                .entry(var_id)
                .or_default()
                .push((row_name.clone(), coeff));
        }
    }

    // Write columns for variables with linear terms
    let mut written_variables = VariableIDSet::new();
    for (var_id, entries) in variable_entries {
        let dvar = instance
            .decision_variables()
            .get(&var_id)
            .expect("Variable ID from linear_terms() must exist in decision_variables");
        written_variables.insert(var_id);
        let var_name = dvar_name(var_id);

        match dvar.kind() {
            // binary or integer var
            DecisionVariableKind::Binary | DecisionVariableKind::Integer => {
                marker_tracker.intorg(out)?
            }
            _ => marker_tracker.intend(out)?,
        }

        // Write all entries for this variable
        for (row_name, coeff) in entries {
            let coeff_value: f64 = coeff.into();
            if coeff_value != 0.0 {
                writeln!(out, "    {var_name}  {row_name}  {coeff_value}")?;
            }
        }
    }

    // Second pass: write variables that only appear in quadratic terms (with zero coefficient)
    let used_ids = instance.used_decision_variable_ids();
    for var_id in used_ids.difference(&written_variables) {
        let dvar = instance.decision_variables().get(var_id).expect(
            "Variable ID from used_decision_variable_ids() must exist in decision_variables",
        );
        let var_name = dvar_name(*var_id);
        match dvar.kind() {
            // binary or integer var
            DecisionVariableKind::Binary | DecisionVariableKind::Integer => {
                marker_tracker.intorg(out)?
            }
            _ => marker_tracker.intend(out)?,
        }
        // Write dummy entry with coefficient 0 for OBJ
        writeln!(out, "    {var_name}  {OBJ_NAME}  0")?;
    }

    // print final INTEND
    marker_tracker.intend(out)?;
    Ok(())
}

fn write_rhs<W: Write>(instance: &Instance, out: &mut W) -> crate::Result<()> {
    writeln!(out, "RHS")?;
    // write out a RHS entry for the objective function if a non-zero constant is present
    let constant = if let Some(linear) = instance.objective().as_linear() {
        linear.constant_term()
    } else if let Some(quadratic) = instance.objective().as_quadratic() {
        quadratic.constant_term()
    } else {
        unreachable!("MPS function preflight verified an objective of degree at most two")
    };

    if constant != 0.0 {
        let rhs = -constant;
        writeln!(out, "  RHS1    {OBJ_NAME}   {rhs}")?;
    }

    for (constr_id, constr) in instance.constraints().iter() {
        let name = constr_name(*constr_id);
        let constant = if let Some(linear) = constr.function().as_linear() {
            linear.constant_term()
        } else if let Some(quadratic) = constr.function().as_quadratic() {
            quadratic.constant_term()
        } else {
            unreachable!("MPS function preflight verified constraints of degree at most two")
        };

        if constant != 0.0 {
            let rhs = -constant;
            writeln!(out, "  RHS1    {name}   {rhs}")?;
        }
    }
    Ok(())
}

fn write_bounds<W: Write>(instance: &Instance, out: &mut W) -> crate::Result<()> {
    writeln!(out, "BOUNDS")?;

    for (var_id, dvar) in instance.used_decision_variables() {
        let name = dvar_name(var_id);
        let bound = dvar.bound();

        // Check special cases for infinity bounds
        if bound.lower() == f64::NEG_INFINITY && bound.upper() == f64::INFINITY {
            // Unbounded variable (-inf, inf)
            writeln!(out, "  FR BND1    {name}")?;
        } else if bound.lower() == f64::NEG_INFINITY {
            // Lower bound is -inf, upper bound is finite
            writeln!(out, "  MI BND1    {name}")?;
            let up_kind = match dvar.kind() {
                DecisionVariableKind::Binary | DecisionVariableKind::Integer => "UI",
                _ => "UP",
            };
            writeln!(out, "  {up_kind} BND1    {name}  {}", bound.upper())?;
        } else if bound.upper() == f64::INFINITY {
            // Upper bound is +inf, lower bound is finite
            writeln!(out, "  PL BND1    {name}")?;
            let low_kind = match dvar.kind() {
                DecisionVariableKind::Binary | DecisionVariableKind::Integer => "LI",
                _ => "LO",
            };
            writeln!(out, "  {low_kind} BND1    {name}  {}", bound.lower())?;
        } else {
            // Both bounds are finite
            let (low_kind, up_kind) = match dvar.kind() {
                // for now ignoring the BV specifier for binary variables
                // due to uncertainty in how widely supported it is.
                DecisionVariableKind::Binary | DecisionVariableKind::Integer => ("LI", "UI"),
                _ => ("LO", "UP"),
            };
            writeln!(out, "  {up_kind} BND1    {name}  {}", bound.upper())?;
            writeln!(out, "  {low_kind} BND1    {name}  {}", bound.lower())?;
        }
    }
    Ok(())
}

/// Generates a name for the constraint based on its ID.
///
/// The constraint's name is ignored, if present.
fn constr_name(constr_id: ConstraintID) -> String {
    format!("{CONSTR_PREFIX}{}", constr_id.into_inner())
}

/// Generates a name for the decision variable based on its ID.
///
/// The decision variable's name is ignored, if present.
fn dvar_name(var_id: VariableID) -> String {
    format!("{VAR_PREFIX}{}", var_id.into_inner())
}

fn write_quadobj<W: Write>(instance: &Instance, out: &mut W) -> crate::Result<()> {
    // Only write QUADOBJ section if the objective has quadratic terms
    if let Some(quadratic) = instance.objective().as_quadratic() {
        let has_quadratic_terms = quadratic
            .iter()
            .any(|(monomial, _)| matches!(monomial, crate::QuadraticMonomial::Pair(_)));

        if has_quadratic_terms {
            writeln!(out, "QUADOBJ")?;

            // Write quadratic terms in sorted order for deterministic output
            let mut quadratic_terms: Vec<_> = quadratic
                .iter()
                .filter_map(|(monomial, coeff)| {
                    if let crate::QuadraticMonomial::Pair(pair) = monomial {
                        Some((pair, coeff))
                    } else {
                        None
                    }
                })
                .collect();

            quadratic_terms.sort_by_key(|(pair, _)| (pair.lower(), pair.upper()));

            for (pair, coeff) in quadratic_terms {
                let var1_name = dvar_name(pair.lower());
                let var2_name = dvar_name(pair.upper());
                let coeff_value: f64 = (*coeff).into();
                if coeff_value != 0.0 {
                    writeln!(out, "    {var1_name}  {var2_name}  {coeff_value}")?;
                }
            }
        }
    }
    Ok(())
}

fn write_qcmatrix<W: Write>(instance: &Instance, out: &mut W) -> crate::Result<()> {
    // Write QCMATRIX sections for each constraint that has quadratic terms
    for (constr_id, constr) in instance.constraints().iter() {
        if let Some(quadratic) = constr.function().as_quadratic() {
            let has_quadratic_terms = quadratic
                .iter()
                .any(|(monomial, _)| matches!(monomial, crate::QuadraticMonomial::Pair(_)));

            if has_quadratic_terms {
                let constraint_name = constr_name(*constr_id);
                writeln!(out, "QCMATRIX {constraint_name}")?;

                // Write quadratic terms in sorted order for deterministic output
                let mut quadratic_terms: Vec<_> = quadratic
                    .iter()
                    .filter_map(|(monomial, coeff)| {
                        if let crate::QuadraticMonomial::Pair(pair) = monomial {
                            Some((pair, coeff))
                        } else {
                            None
                        }
                    })
                    .collect();

                quadratic_terms.sort_by_key(|(pair, _)| (pair.lower(), pair.upper()));

                for (pair, coeff) in quadratic_terms {
                    let var1_name = dvar_name(pair.lower());
                    let var2_name = dvar_name(pair.upper());
                    let coeff_value: f64 = (*coeff).into();
                    if coeff_value != 0.0 {
                        writeln!(out, "    {var1_name}  {var2_name}  {coeff_value}")?;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        decision_variable::Kind, linear, Bound, Constraint, DecisionVariable, Function,
        IndicatorConstraint, IndicatorConstraintID, OneHotConstraint, OneHotConstraintID,
        Sos1Constraint, Sos1ConstraintID,
    };
    use maplit::btreemap;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn test_write_bounds_unbounded() {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                Kind::Continuous,
                Bound::unbounded(),
                crate::ATol::default(),
            ).unwrap(),
        };

        let instance = Instance::new(
            Sense::Minimize,
            Function::from(crate::linear!(0)),
            decision_variables,
            btreemap! {},
        )
        .unwrap();

        let mut buffer = Vec::new();
        write_bounds(&instance, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        insta::assert_snapshot!(output, @r###"
        BOUNDS
          FR BND1    OMMX_VAR_0
        "###);
    }

    #[test]
    fn reports_all_input_class_mismatches_before_writing() {
        let semi_continuous = VariableID::from(1);
        let semi_integer = VariableID::from(2);
        let binary = VariableID::from(3);
        let objective = (Function::from(linear!(semi_continuous))
            + Function::from(linear!(semi_integer)))
        .unwrap()
        .abs();
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(objective)
            .decision_variables(BTreeMap::from([
                (semi_continuous, DecisionVariable::semi_continuous()),
                (semi_integer, DecisionVariable::semi_integer()),
                (binary, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::from([
                (
                    ConstraintID::from(1),
                    Constraint::equal_to_zero(Function::from(linear!(semi_continuous)).signum()),
                ),
                (
                    ConstraintID::from(2),
                    Constraint::less_than_or_equal_to_zero(Function::from(crate::monomial!(
                        semi_integer,
                        semi_integer,
                        semi_integer
                    ))),
                ),
            ]))
            .indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(1),
                IndicatorConstraint::new(
                    binary,
                    Equality::EqualToZero,
                    Function::from(linear!(binary)),
                ),
            )]))
            .one_hot_constraints(BTreeMap::from([(
                OneHotConstraintID::from(1),
                OneHotConstraint::new(BTreeSet::from([binary])).unwrap(),
            )]))
            .sos1_constraints(BTreeMap::from([(
                Sos1ConstraintID::from(1),
                Sos1Constraint::new(BTreeSet::from([binary])).unwrap(),
            )]))
            .build()
            .unwrap();
        let mut buffer = b"unchanged".to_vec();

        let error = format(&instance, &mut buffer).unwrap_err();

        insta::assert_snapshot!(error.to_string(), @r###"
        Instance is outside the MPS input class:
        Instance does not belong to any clause:
        - clause 0 (`MPS`):
          - variable kind SemiContinuous for IDs {VariableID(1)} is not allowed; allowed kinds are {Continuous, Integer, Binary}
          - variable kind SemiInteger for IDs {VariableID(2)} is not allowed; allowed kinds are {Continuous, Integer, Binary}
          - objective function uses the composed expression representation instead of the compact polynomial representation required by this clause
          - regular EqualToZero constraint functions for IDs {ConstraintID(1)} use the composed expression representation instead of the compact polynomial representation required by this clause
          - regular LessThanOrEqualToZero constraint degrees {ConstraintID(2): Degree(3)} exceed degree <= 2
          - indicator constraints {IndicatorConstraintID(1)} are not allowed
          - one-hot constraints {OneHotConstraintID(1)} are not allowed
          - SOS1 constraints {Sos1ConstraintID(1)} are not allowed
        "###);
        assert_eq!(buffer, b"unchanged");
    }

    #[test]
    fn ignores_unused_semi_variable_kinds() {
        let used = VariableID::from(1);
        let unused_semi_continuous = VariableID::from(2);
        let unused_semi_integer = VariableID::from(3);
        let instance = Instance::new(
            Sense::Minimize,
            Function::from(linear!(used)),
            BTreeMap::from([
                (used, DecisionVariable::continuous()),
                (unused_semi_continuous, DecisionVariable::semi_continuous()),
                (unused_semi_integer, DecisionVariable::semi_integer()),
            ]),
            BTreeMap::new(),
        )
        .unwrap();
        let mut buffer = Vec::new();

        format(&instance, &mut buffer).unwrap();

        let output = String::from_utf8(buffer).unwrap();
        assert!(!output.contains(&dvar_name(unused_semi_continuous)));
        assert!(!output.contains(&dvar_name(unused_semi_integer)));
    }

    #[test]
    fn test_write_bounds_positive() {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                Kind::Continuous,
                Bound::positive(),
                crate::ATol::default(),
            ).unwrap(),
        };

        let instance = Instance::new(
            Sense::Minimize,
            Function::from(crate::linear!(0)),
            decision_variables,
            btreemap! {},
        )
        .unwrap();

        let mut buffer = Vec::new();
        write_bounds(&instance, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        insta::assert_snapshot!(output, @r###"
        BOUNDS
          PL BND1    OMMX_VAR_0
          LO BND1    OMMX_VAR_0  0
        "###);
    }

    #[test]
    fn test_write_bounds_negative() {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                Kind::Continuous,
                Bound::negative(),
                crate::ATol::default(),
            ).unwrap(),
        };

        let instance = Instance::new(
            Sense::Minimize,
            Function::from(crate::linear!(0)),
            decision_variables,
            btreemap! {},
        )
        .unwrap();

        let mut buffer = Vec::new();
        write_bounds(&instance, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        insta::assert_snapshot!(output, @r###"
        BOUNDS
          MI BND1    OMMX_VAR_0
          UP BND1    OMMX_VAR_0  0
        "###);
    }

    #[test]
    fn test_write_bounds_integer_types() {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                Kind::Binary,
                Bound::of_binary(),
                crate::ATol::default(),
            ).unwrap(),
            VariableID::from(1) => DecisionVariable::new(
                Kind::Integer,
                Bound::new(-10.0, 20.0).unwrap(),
                crate::ATol::default(),
            ).unwrap(),
        };

        let instance = Instance::new(
            Sense::Minimize,
            Function::from(crate::linear!(0) + crate::linear!(1)),
            decision_variables,
            btreemap! {},
        )
        .unwrap();

        let mut buffer = Vec::new();
        write_bounds(&instance, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        insta::assert_snapshot!(output, @r###"
        BOUNDS
          UI BND1    OMMX_VAR_0  1
          LI BND1    OMMX_VAR_0  0
          UI BND1    OMMX_VAR_1  20
          LI BND1    OMMX_VAR_1  -10
        "###);
    }

    #[test]
    fn test_write_bounds_mixed_types() {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                Kind::Continuous,
                Bound::unbounded(),
                crate::ATol::default(),
            ).unwrap(),
            VariableID::from(1) => DecisionVariable::new(
                Kind::Continuous,
                Bound::positive(),
                crate::ATol::default(),
            ).unwrap(),
            VariableID::from(2) => DecisionVariable::new(
                Kind::Integer,
                Bound::negative(),
                crate::ATol::default(),
            ).unwrap(),
        };

        let instance = Instance::new(
            Sense::Minimize,
            Function::from(
                ((crate::linear!(0) + crate::linear!(1)).unwrap() + crate::linear!(2)).unwrap(),
            ),
            decision_variables,
            btreemap! {},
        )
        .unwrap();

        let mut buffer = Vec::new();
        write_bounds(&instance, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        insta::assert_snapshot!(output, @r###"
        BOUNDS
          FR BND1    OMMX_VAR_0
          PL BND1    OMMX_VAR_1
          LO BND1    OMMX_VAR_1  0
          MI BND1    OMMX_VAR_2
          UI BND1    OMMX_VAR_2  0
        "###);
    }
}
