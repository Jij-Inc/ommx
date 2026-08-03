use crate::{
    logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path},
    v1::State,
    AcyclicAssignments, Evaluate, Function, Parse, ParseError, RawParseError, Sampled, Substitute,
    SubstitutionError, VariableID, VariableIDSet,
};
use std::mem::size_of;

fn exact_nonzero_indicator(value: f64) -> crate::Result<f64> {
    if !value.is_finite() {
        crate::bail!(
            { value },
            "NonzeroIndicator input evaluated to a non-finite value: {value}"
        );
    }
    Ok(if value == 0.0 { 0.0 } else { 1.0 })
}

/// A deterministic expression used to reconstruct a dependent decision variable.
///
/// Unlike [`Function`], this expression is not an optimization-model function:
/// it is evaluated by OMMX after solving and is not accepted as an objective or
/// an active constraint function. This separation permits deterministic
/// reconstruction operations that are not polynomial while keeping the solver
/// input algebraic.
///
/// The initial AST deliberately contains only existing [`Function`] leaves
/// and exact nonzero indicators. More reconstruction operations can be added as
/// new variants without extending the algebra understood by solver adapters.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DependentExpr {
    /// An existing algebraic OMMX function.
    Function(Function),
    /// `0` exactly when the inner expression is `0`, and `1` otherwise.
    ///
    /// This predicate uses exact floating-point equality, including treating
    /// both `0.0` and `-0.0` as zero. [`crate::ATol`] is intentionally not used
    /// to classify the inner value.
    NonzeroIndicator(Box<DependentExpr>),
}

impl DependentExpr {
    /// Wrap an expression in an exact nonzero indicator.
    pub fn nonzero_indicator(expr: impl Into<Self>) -> Self {
        Self::NonzeroIndicator(Box::new(expr.into()))
    }
}

impl Default for DependentExpr {
    fn default() -> Self {
        Self::Function(Function::Zero)
    }
}

impl From<Function> for DependentExpr {
    fn from(function: Function) -> Self {
        Self::Function(function)
    }
}

impl From<DependentExpr> for crate::v2::DependentExpr {
    fn from(value: DependentExpr) -> Self {
        let expression = match value {
            DependentExpr::Function(function) => {
                crate::v2::dependent_expr::Expression::Function(function.into())
            }
            DependentExpr::NonzeroIndicator(inner) => {
                crate::v2::dependent_expr::Expression::NonzeroIndicator(Box::new((*inner).into()))
            }
        };
        Self {
            expression: Some(expression),
        }
    }
}

impl Parse for crate::v2::DependentExpr {
    type Output = DependentExpr;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.DependentExpr";
        match self.expression.ok_or(RawParseError::MissingField {
            message,
            field: "expression",
        })? {
            crate::v2::dependent_expr::Expression::Function(function) => Ok(
                DependentExpr::Function(function.parse_as(&(), message, "function")?),
            ),
            crate::v2::dependent_expr::Expression::NonzeroIndicator(inner) => {
                Ok(DependentExpr::NonzeroIndicator(Box::new(
                    (*inner).parse_as(&(), message, "nonzero_indicator")?,
                )))
            }
        }
    }
}

impl Evaluate for DependentExpr {
    type Output = f64;
    type SampledOutput = Sampled<f64>;

    fn evaluate(&self, state: &State, atol: crate::ATol) -> crate::Result<Self::Output> {
        match self {
            Self::Function(function) => function.evaluate(state, atol),
            Self::NonzeroIndicator(inner) => {
                let value = inner.evaluate(state, atol)?;
                exact_nonzero_indicator(value)
            }
        }
    }

    fn evaluate_samples(
        &self,
        samples: &Sampled<State>,
        atol: crate::ATol,
    ) -> crate::Result<Self::SampledOutput> {
        match self {
            Self::Function(function) => function.evaluate_samples(samples, atol),
            Self::NonzeroIndicator(inner) => inner
                .evaluate_samples(samples, atol)?
                .try_map_ref(|value| exact_nonzero_indicator(*value)),
        }
    }

    fn partial_evaluate(&mut self, state: &State, atol: crate::ATol) -> crate::Result<()> {
        match self {
            Self::Function(function) => function.partial_evaluate(state, atol),
            Self::NonzeroIndicator(inner) => {
                inner.partial_evaluate(state, atol)?;
                if inner.required_ids().is_empty() {
                    let value = inner.evaluate(&State::default(), atol)?;
                    let value = exact_nonzero_indicator(value)?;
                    *self = Self::Function(Function::try_from(value)?);
                }
                Ok(())
            }
        }
    }

    fn required_ids(&self) -> VariableIDSet {
        match self {
            Self::Function(function) => function.required_ids(),
            Self::NonzeroIndicator(inner) => inner.required_ids(),
        }
    }
}

impl Substitute for DependentExpr {
    type Output = Self;

    fn substitute_acyclic(
        self,
        acyclic: &AcyclicAssignments,
    ) -> Result<Self::Output, SubstitutionError> {
        match self {
            Self::Function(function) => Ok(Self::Function(function.substitute_acyclic(acyclic)?)),
            Self::NonzeroIndicator(inner) => Ok(Self::NonzeroIndicator(Box::new(
                inner.substitute_acyclic(acyclic)?,
            ))),
        }
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        function: &Function,
    ) -> Result<Self::Output, SubstitutionError> {
        match self {
            Self::Function(inner) => Ok(Self::Function(inner.substitute_one(assigned, function)?)),
            Self::NonzeroIndicator(inner) => Ok(Self::NonzeroIndicator(Box::new(
                inner.substitute_one(assigned, function)?,
            ))),
        }
    }
}

// A data-carrying recursive enum cannot use the current derive macro. Account
// for variant layout separately and delegate heap ownership to the payload.
impl LogicalMemoryProfile for DependentExpr {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        match self {
            Self::Function(function) => {
                let additional_stack = size_of::<Self>() - size_of::<Function>();
                if additional_stack > 0 {
                    visitor.visit_leaf(
                        &path.with("DependentExpr.Function[additional stack]"),
                        additional_stack,
                    );
                }
                function
                    .visit_logical_memory(path.with("DependentExpr.Function").as_mut(), visitor);
            }
            Self::NonzeroIndicator(inner) => {
                let additional_stack = size_of::<Self>() - size_of::<Box<Self>>();
                if additional_stack > 0 {
                    visitor.visit_leaf(
                        &path.with("DependentExpr.NonzeroIndicator[additional stack]"),
                        additional_stack,
                    );
                }
                inner.visit_logical_memory(
                    path.with("DependentExpr.NonzeroIndicator").as_mut(),
                    visitor,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, ATol, SampleID};

    fn state(entries: impl IntoIterator<Item = (u64, f64)>) -> State {
        entries.into_iter().collect()
    }

    #[test]
    fn nonzero_indicator_uses_exact_zero_independently_of_atol() {
        let expr = DependentExpr::nonzero_indicator(Function::from(linear!(1)));
        let atol = ATol::new(1.0).unwrap();

        assert_eq!(expr.evaluate(&state([(1, 0.0)]), atol).unwrap(), 0.0);
        assert_eq!(expr.evaluate(&state([(1, -0.0)]), atol).unwrap(), 0.0);
        assert_eq!(
            expr.evaluate(&state([(1, f64::MIN_POSITIVE)]), atol)
                .unwrap(),
            1.0
        );
        assert_eq!(
            expr.evaluate(&state([(1, -f64::MIN_POSITIVE)]), atol)
                .unwrap(),
            1.0
        );
    }

    #[test]
    fn nested_nonzero_indicator_is_evaluated_recursively() {
        let expr = DependentExpr::nonzero_indicator(DependentExpr::nonzero_indicator(
            Function::from(linear!(1)),
        ));

        assert_eq!(
            expr.evaluate(&state([(1, -2.0)]), ATol::default()).unwrap(),
            1.0
        );
        assert_eq!(
            expr.evaluate(&state([(1, 0.0)]), ATol::default()).unwrap(),
            0.0
        );
    }

    #[test]
    fn nonzero_indicator_does_not_hide_non_finite_values() {
        let expr = DependentExpr::nonzero_indicator(Function::from(linear!(1)));

        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let error = expr
                .evaluate(&state([(1, value)]), ATol::default())
                .unwrap_err();
            assert!(error.to_string().contains("non-finite"));
        }

        let samples = Sampled::new([[SampleID::from(1)]], [state([(1, f64::INFINITY)])]).unwrap();
        let error = expr
            .evaluate_samples(&samples, ATol::default())
            .unwrap_err();
        assert!(error.to_string().contains("non-finite"));
    }

    #[test]
    fn partial_evaluate_preserves_or_folds_indicator_by_required_ids() {
        let function = Function::from((linear!(1) + linear!(2)).unwrap());
        let mut expr = DependentExpr::nonzero_indicator(function);

        expr.partial_evaluate(&state([(1, -1.0)]), ATol::default())
            .unwrap();
        assert_eq!(expr.required_ids(), VariableIDSet::from([2.into()]));

        expr.partial_evaluate(&state([(2, 1.0)]), ATol::default())
            .unwrap();
        assert_eq!(expr, DependentExpr::Function(Function::Zero));

        let mut nonzero = DependentExpr::nonzero_indicator(Function::from(linear!(3)));
        nonzero
            .partial_evaluate(&state([(3, -4.0)]), ATol::default())
            .unwrap();
        assert_eq!(
            nonzero
                .evaluate(&State::default(), ATol::default())
                .unwrap(),
            1.0
        );
        assert!(nonzero.required_ids().is_empty());
    }

    #[test]
    fn evaluate_samples_preserves_sample_ids_and_grouped_values() {
        let expr = DependentExpr::nonzero_indicator(Function::from(linear!(1)));
        let samples = Sampled::new(
            vec![
                vec![SampleID::from(10), SampleID::from(11)],
                vec![SampleID::from(20)],
            ],
            [state([(1, 0.0)]), state([(1, 2.0)])],
        )
        .unwrap();

        let evaluated = expr.evaluate_samples(&samples, ATol::default()).unwrap();

        assert_eq!(evaluated.num_samples(), 3);
        assert_eq!(evaluated.get(SampleID::from(10)), Some(&0.0));
        assert_eq!(evaluated.get(SampleID::from(11)), Some(&0.0));
        assert_eq!(evaluated.get(SampleID::from(20)), Some(&1.0));
    }

    #[test]
    fn substitution_rewrites_function_leaves_and_preserves_indicator_nodes() {
        let expr =
            DependentExpr::nonzero_indicator(Function::from((linear!(1) + coeff!(1.0)).unwrap()));
        let replacement = Function::from((linear!(2) - coeff!(1.0)).unwrap());

        let substituted = expr
            .substitute_one(VariableID::from(1), &replacement)
            .unwrap();

        assert!(matches!(substituted, DependentExpr::NonzeroIndicator(_)));
        assert_eq!(substituted.required_ids(), VariableIDSet::from([2.into()]));
        assert_eq!(
            substituted
                .evaluate(&state([(2, 0.0)]), ATol::default())
                .unwrap(),
            0.0
        );
        assert_eq!(
            substituted
                .evaluate(&state([(2, 2.0)]), ATol::default())
                .unwrap(),
            1.0
        );
    }

    #[test]
    fn v2_parse_rejects_missing_expression_variant() {
        let error = crate::v2::DependentExpr::default().parse(&()).unwrap_err();
        assert!(
            error.to_string().contains("expression") && error.to_string().contains("missing"),
            "unexpected error: {error}"
        );
    }
}
