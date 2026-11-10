#![cfg(feature = "remote-artifact")]

use ommx::{
    artifact::{ghcr, media_types, set_local_registry_root, ArtifactDraft, LocalArtifact},
    dataset::qplib,
    ATol, Evaluate, Function, Instance, Sense,
};
use std::collections::HashMap;

#[test]
fn corrected_distribution_takes_precedence_over_legacy_cache() -> ommx::Result<()> {
    // The registry root is global, so this test has its own integration-test process.
    let registry = tempfile::tempdir()?;
    set_local_registry_root(registry.path())?;
    let legacy_ref = ghcr("Jij-Inc", "ommx", "qplib", "0018")?;
    let corrected_ref = ghcr("Jij-Inc", "ommx", "v2.8/qplib", "0018")?;
    let empty = Instance::new(
        Sense::Minimize,
        Function::Zero,
        Default::default(),
        Default::default(),
    )?;
    let corrected = ommx::qplib::parse(include_bytes!("fixtures/QPLIB_0018.qplib").as_slice())?;
    let empty_bytes = empty.to_v1_bytes()?;
    for (image, instance) in [(legacy_ref.clone(), empty), (corrected_ref, corrected)] {
        let mut draft = ArtifactDraft::new(image)?;
        // The v2.8 distribution stores legacy v1 protobuf layers and descriptor annotations.
        draft.add_layer_bytes(
            media_types::v1_instance(),
            instance.to_v1_bytes()?,
            HashMap::from([
                ("org.ommx.v1.instance.title".into(), "QPLIB_0018".into()),
                ("org.ommx.qplib.parser_version".into(), "2.8.0".into()),
            ]),
        )?;
        draft.commit()?;
    }

    let instance: Instance = qplib::load("0018")?.try_into()?;
    assert_eq!(instance.decision_variables().len(), 50);
    assert_eq!(
        instance.annotations["org.ommx.qplib.parser_version"],
        "2.8.0"
    );
    let state =
        ommx::qplib::parse_solution(include_bytes!("fixtures/QPLIB_0018.sol").as_slice(), 50)?;
    let solution = instance.evaluate(&state, ATol::new(1e-8)?)?;
    approx::assert_abs_diff_eq!(
        *solution.objective(),
        -6.386_014_981_598_35,
        epsilon = 1e-10
    );
    assert!(solution.feasible());

    let legacy = LocalArtifact::open(legacy_ref)?;
    assert_eq!(legacy.get_blob(&legacy.layers()?[0])?, empty_bytes);
    Ok(())
}
