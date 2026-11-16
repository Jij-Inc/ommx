use super::*;
use crate::{ATol, Constraint, Parse, ParseError};

macro_rules! impl_v2_created_collection {
    ($source:ty => $target:ty) => {
        impl From<ConstraintCollection<$source>> for $target {
            fn from(value: ConstraintCollection<$source>) -> Self {
                let ConstraintCollection {
                    active,
                    inactive,
                    context,
                } = value;
                let (inactive, inactive_metadata) = inactive_entries_to_v2_maps(inactive);
                Self {
                    active: entries_to_v2_map(active),
                    inactive,
                    inactive_metadata,
                    contexts: constraint_context_store_to_v2_map(&context),
                }
            }
        }
    };
}

impl_v2_created_collection!(Constraint => crate::v2::RegularConstraintCollection);
impl_v2_created_collection!(crate::IndicatorConstraint => crate::v2::IndicatorConstraintCollection);
impl_v2_created_collection!(crate::OneHotConstraint => crate::v2::OneHotConstraintCollection);
impl_v2_created_collection!(crate::Sos1Constraint => crate::v2::Sos1ConstraintCollection);

macro_rules! impl_v2_evaluated_collection {
    ($source:ty => $target:ident) => {
        impl EvaluatedCollection<$source> {
            /// Solution/SampleSet supplies the shared wire feasibility tolerance.
            pub(crate) fn into_v2(self, atol: ATol) -> crate::v2::$target {
                let value = self;
                let EvaluatedCollection {
                    constraints,
                    inactive_metadata,
                    context,
                } = value;
                crate::v2::$target {
                    entries: constraints
                        .into_iter()
                        .map(|(id, row)| (id.into(), row.into_v2(atol)))
                        .collect(),
                    inactive_metadata: inactive_metadata_to_v2_map(inactive_metadata),
                    contexts: constraint_context_store_to_v2_map(&context),
                }
            }
        }
    };
}

impl_v2_evaluated_collection!(Constraint => EvaluatedRegularConstraintCollection);
impl_v2_evaluated_collection!(crate::IndicatorConstraint => EvaluatedIndicatorConstraintCollection);
impl_v2_evaluated_collection!(crate::OneHotConstraint => EvaluatedOneHotConstraintCollection);
impl_v2_evaluated_collection!(crate::Sos1Constraint => EvaluatedSos1ConstraintCollection);

macro_rules! impl_v2_sampled_collection {
    ($source:ty => $target:ident) => {
        impl SampledCollection<$source> {
            /// Solution/SampleSet supplies the shared wire feasibility tolerance.
            pub(crate) fn into_v2(self, atol: ATol) -> crate::v2::$target {
                let value = self;
                let SampledCollection {
                    constraints,
                    inactive_metadata,
                    context,
                } = value;
                crate::v2::$target {
                    entries: constraints
                        .into_iter()
                        .map(|(id, row)| (id.into(), row.into_v2(atol)))
                        .collect(),
                    inactive_metadata: inactive_metadata_to_v2_map(inactive_metadata),
                    contexts: constraint_context_store_to_v2_map(&context),
                }
            }
        }
    };
}

impl_v2_sampled_collection!(Constraint => SampledRegularConstraintCollection);
impl_v2_sampled_collection!(crate::IndicatorConstraint => SampledIndicatorConstraintCollection);
impl_v2_sampled_collection!(crate::OneHotConstraint => SampledOneHotConstraintCollection);
impl_v2_sampled_collection!(crate::Sos1Constraint => SampledSos1ConstraintCollection);

fn constraint_context_store_to_v2_map<ID: IDType>(
    store: &ConstraintContextStore<ID>,
) -> BTreeMap<u64, crate::v2::ConstraintContext> {
    store
        .ids()
        .into_iter()
        .map(|id| (id.into(), store.collect_for(id).into()))
        .collect()
}

fn constraint_context_store_from_v2_map<ID: IDType>(
    contexts: BTreeMap<u64, crate::v2::ConstraintContext>,
    message: &'static str,
    field: &'static str,
) -> std::result::Result<ConstraintContextStore<ID>, ParseError> {
    let mut store = ConstraintContextStore::default();
    for (id, context) in contexts {
        store.insert(ID::from(id), context.parse_as(&(), message, field)?);
    }
    Ok(store)
}

macro_rules! impl_parse_v2_created_collection {
    ($source:ty => $target:ty) => {
        impl Parse for $target {
            type Output = ConstraintCollection<$source>;
            type Context = ();

            fn parse(self, _: &Self::Context) -> std::result::Result<Self::Output, ParseError> {
                let message = stringify!($target);
                let active = parse_v2_constraint_entries(self.active, message, "active")?;
                let inactive = parse_v2_inactive_constraint_entries(
                    self.inactive,
                    self.inactive_metadata,
                    message,
                )?;
                let context =
                    constraint_context_store_from_v2_map(self.contexts, message, "contexts")?;
                let owned_ids = active
                    .keys()
                    .chain(inactive.keys())
                    .copied()
                    .collect::<BTreeSet<_>>();
                validate_context_reference_ids(&context, &owned_ids)
                    .map_err(|e| ParseError::new(e).context(message, "contexts"))?;
                ConstraintCollection::with_context(active, inactive, context)
                    .map_err(|e| ParseError::new(e).context(message, "active"))
            }
        }
    };
}

impl_parse_v2_created_collection!(Constraint => crate::v2::RegularConstraintCollection);
impl_parse_v2_created_collection!(crate::IndicatorConstraint => crate::v2::IndicatorConstraintCollection);
impl_parse_v2_created_collection!(crate::OneHotConstraint => crate::v2::OneHotConstraintCollection);
impl_parse_v2_created_collection!(crate::Sos1Constraint => crate::v2::Sos1ConstraintCollection);

macro_rules! impl_parse_v2_evaluated_collection {
    ($source:ty => $target:ty) => {
        impl Parse for $target {
            type Output = EvaluatedCollection<$source>;
            type Context = crate::ATol;

            fn parse(self, atol: &Self::Context) -> std::result::Result<Self::Output, ParseError> {
                let message = stringify!($target);
                let entries = parse_v2_constraint_entries_with_context(
                    self.entries,
                    message,
                    "entries",
                    atol,
                )?;
                let inactive_metadata = parse_v2_inactive_metadata(
                    self.inactive_metadata,
                    message,
                    "inactive_metadata",
                )?;
                let context =
                    constraint_context_store_from_v2_map(self.contexts, message, "contexts")?;
                let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
                validate_context_reference_ids(&context, &owned_ids)
                    .map_err(|e| ParseError::new(e).context(message, "contexts"))?;
                EvaluatedCollection::with_context(entries, inactive_metadata, context)
                    .map_err(|e| ParseError::new(e).context(message, "inactive_metadata"))
            }
        }
    };
}

impl_parse_v2_evaluated_collection!(Constraint => crate::v2::EvaluatedRegularConstraintCollection);
impl_parse_v2_evaluated_collection!(crate::IndicatorConstraint => crate::v2::EvaluatedIndicatorConstraintCollection);

macro_rules! impl_parse_v2_sampled_collection {
    ($source:ty => $target:ty) => {
        impl Parse for $target {
            type Output = SampledCollection<$source>;
            type Context = crate::ATol;

            fn parse(self, atol: &Self::Context) -> std::result::Result<Self::Output, ParseError> {
                let message = stringify!($target);
                let entries = parse_v2_constraint_entries_with_context(
                    self.entries,
                    message,
                    "entries",
                    atol,
                )?;
                let inactive_metadata = parse_v2_inactive_metadata(
                    self.inactive_metadata,
                    message,
                    "inactive_metadata",
                )?;
                let context =
                    constraint_context_store_from_v2_map(self.contexts, message, "contexts")?;
                let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
                validate_context_reference_ids(&context, &owned_ids)
                    .map_err(|e| ParseError::new(e).context(message, "contexts"))?;
                SampledCollection::with_context(entries, inactive_metadata, context)
                    .map_err(|e| ParseError::new(e).context(message, "inactive_metadata"))
            }
        }
    };
}

impl_parse_v2_sampled_collection!(Constraint => crate::v2::SampledRegularConstraintCollection);
impl_parse_v2_sampled_collection!(crate::IndicatorConstraint => crate::v2::SampledIndicatorConstraintCollection);

// Solution and SampleSet supply the cross-table values required to restore a
// structural constraint's metric. The collection owns its IDs and sidecars.
macro_rules! impl_restore_v2_structural_collection {
    ($source:ty => $evaluated:ty, $sampled:ty) => {
        impl $evaluated {
            pub(crate) fn parse_with_values(
                self,
                variables: &crate::EvaluatedDecisionVariableTable,
                atol: ATol,
            ) -> std::result::Result<EvaluatedCollection<$source>, ParseError> {
                let message = stringify!($evaluated);
                let entries = self
                    .entries
                    .into_iter()
                    .map(|(id, row)| {
                        let constraint = row
                            .parse_with_values(
                                |variable_id| {
                                    variables
                                        .get(&variable_id)
                                        .map(|variable| *variable.value())
                                },
                                atol,
                            )
                            .map_err(|error| error.context(message, "entries"))?;
                        Ok((<<$source as ConstraintType>::ID>::from(id), constraint))
                    })
                    .collect::<std::result::Result<BTreeMap<_, _>, ParseError>>()?;
                let inactive_metadata = parse_v2_inactive_metadata(
                    self.inactive_metadata,
                    message,
                    "inactive_metadata",
                )?;
                let context =
                    constraint_context_store_from_v2_map(self.contexts, message, "contexts")?;
                let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
                validate_context_reference_ids(&context, &owned_ids)
                    .map_err(|error| ParseError::new(error).context(message, "contexts"))?;
                EvaluatedCollection::with_context(entries, inactive_metadata, context)
                    .map_err(|error| ParseError::new(error).context(message, "inactive_metadata"))
            }
        }

        impl $sampled {
            pub(crate) fn parse_with_values(
                self,
                variables: &crate::SampledDecisionVariableTable,
                atol: ATol,
            ) -> std::result::Result<SampledCollection<$source>, ParseError> {
                let message = stringify!($sampled);
                let entries = self
                    .entries
                    .into_iter()
                    .map(|(id, row)| {
                        let constraint = row
                            .parse_with_values(
                                |sample_id, variable_id| {
                                    variables
                                        .get(&variable_id)
                                        .and_then(|variable| variable.samples().get(sample_id))
                                        .copied()
                                },
                                atol,
                            )
                            .map_err(|error| error.context(message, "entries"))?;
                        Ok((<<$source as ConstraintType>::ID>::from(id), constraint))
                    })
                    .collect::<std::result::Result<BTreeMap<_, _>, ParseError>>()?;
                let inactive_metadata = parse_v2_inactive_metadata(
                    self.inactive_metadata,
                    message,
                    "inactive_metadata",
                )?;
                let context =
                    constraint_context_store_from_v2_map(self.contexts, message, "contexts")?;
                let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
                validate_context_reference_ids(&context, &owned_ids)
                    .map_err(|error| ParseError::new(error).context(message, "contexts"))?;
                SampledCollection::with_context(entries, inactive_metadata, context)
                    .map_err(|error| ParseError::new(error).context(message, "inactive_metadata"))
            }
        }
    };
}

impl_restore_v2_structural_collection!(crate::OneHotConstraint => crate::v2::EvaluatedOneHotConstraintCollection, crate::v2::SampledOneHotConstraintCollection);
impl_restore_v2_structural_collection!(crate::Sos1Constraint => crate::v2::EvaluatedSos1ConstraintCollection, crate::v2::SampledSos1ConstraintCollection);

fn entries_to_v2_map<ID, Row, V2Row>(entries: BTreeMap<ID, Row>) -> BTreeMap<u64, V2Row>
where
    ID: IDType,
    Row: Into<V2Row>,
{
    entries
        .into_iter()
        .map(|(id, row)| (id.into(), row.into()))
        .collect()
}

fn parse_v2_constraint_entries<ID, V2Row, Row>(
    entries: BTreeMap<u64, V2Row>,
    message: &'static str,
    field: &'static str,
) -> std::result::Result<BTreeMap<ID, Row>, ParseError>
where
    ID: IDType,
    V2Row: Parse<Output = Row, Context = ()>,
{
    entries
        .into_iter()
        .map(|(id, row)| Ok((ID::from(id), row.parse_as(&(), message, field)?)))
        .collect()
}

fn parse_v2_constraint_entries_with_context<ID, V2Row, Row, C>(
    entries: BTreeMap<u64, V2Row>,
    message: &'static str,
    field: &'static str,
    context: &C,
) -> std::result::Result<BTreeMap<ID, Row>, ParseError>
where
    ID: IDType,
    V2Row: Parse<Output = Row, Context = C>,
{
    entries
        .into_iter()
        .map(|(id, row)| Ok((ID::from(id), row.parse_as(context, message, field)?)))
        .collect()
}

fn inactive_entries_to_v2_maps<ID, Row, V2Row>(
    inactive: BTreeMap<ID, (Row, InactiveConstraintMetadata)>,
) -> (
    BTreeMap<u64, V2Row>,
    BTreeMap<u64, crate::v2::InactiveConstraintMetadata>,
)
where
    ID: IDType,
    Row: Into<V2Row>,
{
    let mut rows = BTreeMap::new();
    let mut reasons = BTreeMap::new();
    for (id, (row, reason)) in inactive {
        let id = id.into();
        rows.insert(id, row.into());
        reasons.insert(id, reason.into());
    }
    (rows, reasons)
}

fn parse_v2_inactive_constraint_entries<ID, V2Row, Row>(
    inactive: BTreeMap<u64, V2Row>,
    mut inactive_metadata: BTreeMap<u64, crate::v2::InactiveConstraintMetadata>,
    message: &'static str,
) -> std::result::Result<BTreeMap<ID, (Row, InactiveConstraintMetadata)>, ParseError>
where
    ID: IDType,
    V2Row: Parse<Output = Row, Context = ()>,
{
    let mut out = BTreeMap::new();
    for (id, row) in inactive {
        let reason = inactive_metadata.remove(&id).ok_or_else(|| {
            ParseError::new(crate::error!(
                "Inactive constraint ID {:?} has no inactive metadata",
                ID::from(id)
            ))
            .context(message, "inactive_metadata")
        })?;
        out.insert(
            ID::from(id),
            (row.parse_as(&(), message, "inactive")?, reason.into()),
        );
    }
    if let Some(id) = inactive_metadata.keys().next().copied() {
        return Err(ParseError::new(crate::error!(
            "Inactive metadata references unknown constraint ID {:?}",
            ID::from(id)
        ))
        .context(message, "inactive_metadata"));
    }
    Ok(out)
}

fn inactive_metadata_to_v2_map<ID: IDType>(
    inactive_metadata: BTreeMap<ID, InactiveConstraintMetadata>,
) -> BTreeMap<u64, crate::v2::InactiveConstraintMetadata> {
    inactive_metadata
        .into_iter()
        .map(|(id, reason)| (id.into(), reason.into()))
        .collect()
}

fn parse_v2_inactive_metadata<ID: IDType>(
    inactive_metadata: BTreeMap<u64, crate::v2::InactiveConstraintMetadata>,
    _message: &'static str,
    _field: &'static str,
) -> std::result::Result<BTreeMap<ID, InactiveConstraintMetadata>, ParseError> {
    Ok(inactive_metadata
        .into_iter()
        .map(|(id, reason)| (ID::from(id), reason.into()))
        .collect())
}
