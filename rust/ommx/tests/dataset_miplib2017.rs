#![cfg(feature = "remote-artifact")]

use ommx::{
    artifact::{ghcr, media_types, set_local_registry_root, ArtifactDraft, LocalArtifact},
    dataset::miplib2017,
    v1::decision_variable::Kind,
    DecisionVariable, Function, Instance, Sense, VariableID,
};
use std::collections::HashMap;

#[test]
fn versioned_distribution_takes_precedence_over_legacy_cache() -> ommx::Result<()> {
    // This integration test runs in its own process because the root is global.
    let registry = tempfile::tempdir()?;
    set_local_registry_root(registry.path())?;
    let name = "neos-2626858-aoos";
    let legacy_ref = ghcr("Jij-Inc", "ommx", "miplib2017", name)?;
    let versioned_ref = ghcr("Jij-Inc", "ommx", "v2.7/miplib2017", name)?;
    let empty = Instance::new(
        Sense::Minimize,
        Function::Zero,
        Default::default(),
        Default::default(),
    )?;
    let corrected = Instance::new(
        Sense::Minimize,
        Function::Zero,
        [(VariableID::from(506), DecisionVariable::binary())].into(),
        Default::default(),
    )?;
    let empty_bytes = empty.to_v1_bytes()?;
    for (image_name, instance) in [(legacy_ref.clone(), empty), (versioned_ref, corrected)] {
        let mut draft = ArtifactDraft::new(image_name)?;
        // Published v2.7 Artifacts have v1 protobuf layers and descriptor annotations.
        draft.add_layer_bytes(
            media_types::v1_instance(),
            instance.to_v1_bytes()?,
            HashMap::from([("org.ommx.v1.instance.title".into(), name.into())]),
        )?;
        draft.commit()?;
    }

    let instance = miplib2017::load(name)?;
    assert_eq!(
        instance.description.as_ref().unwrap().name.as_deref(),
        Some(name)
    );
    assert_eq!(instance.decision_variables.len(), 1);
    let variable = &instance.decision_variables[0];
    assert_eq!(variable.id, 506);
    assert_eq!(variable.kind(), Kind::Binary);
    let bound = variable.bound.as_ref().unwrap();
    assert_eq!(bound.lower, 0.0);
    assert_eq!(bound.upper, 1.0);

    let legacy = LocalArtifact::open(legacy_ref)?;
    assert_eq!(legacy.get_blob(&legacy.layers()?[0])?, empty_bytes);
    Ok(())
}
