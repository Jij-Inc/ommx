use anyhow::Result;
use ommx::{
    artifact::{self, Builder, InstanceAnnotations},
    v1::State,
    ATol, Evaluate,
};
use std::{fs, io::Write, path::Path, process::Command, time::SystemTime};
use zip::{write::SimpleFileOptions, ZipWriter};

#[test]
fn package_and_load_versioned_qplib_with_legacy_cache() -> Result<()> {
    let root = std::env::temp_dir().join(format!(
        "ommx-qplib-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&root)?;
    let registry = root.join("registry");
    artifact::set_local_registry_root(&registry)?;
    let mut legacy = Builder::for_github("Jij-Inc", "ommx", "qplib", "0018")?;
    legacy.add_instance(
        ommx::v1::Instance::default(),
        InstanceAnnotations::default(),
    )?;
    legacy.build()?;

    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ommx/tests/fixtures");
    let input = root.join("qplib.zip");
    let mut zip = ZipWriter::new(fs::File::create(&input)?);
    let options = SimpleFileOptions::default();
    zip.start_file("qplib/html/qplib/QPLIB_0018.qplib", options)?;
    zip.write_all(&fs::read(fixtures.join("QPLIB_0018.qplib"))?)?;
    zip.start_file("QPLIB_0031.qplib", options)?;
    zip.write_all(b"invalid\n")?;
    zip.start_file("QPLIB_99999.qplib", options)?;
    zip.write_all(b"invalid\n")?;
    zip.start_file("QPLIB_0032.qplib", options)?;
    zip.write_all(&fs::read(fixtures.join("quadratic_scaling.qplib"))?)?;
    zip.finish()?;

    let report = root.join("report.csv");
    let generate = || {
        Command::new(env!("CARGO_BIN_EXE_dataset"))
            .arg("qplib")
            .arg(&input)
            .arg("--report")
            .arg(&report)
            .env("OMMX_LOCAL_REGISTRY_ROOT", &registry)
            .output()
    };
    let output = generate()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = csv::Reader::from_path(&report)?
        .records()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(rows.len(), 4);
    assert_eq!(&rows[0][1], "ghcr.io/jij-inc/ommx/v2.8/qplib:0018");
    assert_eq!(&rows[0][2], "packaged");
    assert_eq!(&rows[0][4], "50");
    for row in &rows[1..] {
        assert_eq!(&row[2], "failed");
        assert!(row[9].is_empty());
    }
    assert!(rows[2][3].contains("No metadata"));
    assert!(rows[3][3].contains("Variable count mismatch"));

    let (instance, annotations) = ommx::dataset::qplib::load("0018")?;
    assert_eq!(annotations.title()?, "QPLIB_0018");
    assert_eq!(annotations.license()?, "CC-BY-4.0");
    assert_eq!(instance.decision_variables.len(), 50);
    let mut state = State::from(
        (0..50)
            .map(|id| (id, 0.0))
            .collect::<std::collections::HashMap<_, _>>(),
    );
    for (id, value) in [
        (13, 0.209636569541294),
        (16, 0.275230558068530),
        (38, 0.226997921553671),
        (40, 0.288134950836505),
    ] {
        state.entries.insert(id, value);
    }
    let solution = instance.evaluate(&state, ATol::new(1e-8)?)?;
    assert!((solution.objective - (-6.386_014_981_598_35)).abs() < 1e-10);
    assert!(solution.feasible);
    for tag in ["0031", "99999", "0032"] {
        let image = artifact::ghcr("Jij-Inc", "ommx", "v2.8/qplib", tag)?;
        assert!(!artifact::get_image_dir(&image).exists());
    }

    // A repeated run must report existing outputs as failures, not successes.
    assert!(generate()?.status.success());
    let rows = csv::Reader::from_path(&report)?
        .records()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert!(rows.iter().all(|row| &row[2] == "failed"));
    fs::remove_dir_all(root)?;
    Ok(())
}
