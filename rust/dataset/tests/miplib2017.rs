use anyhow::Result;
use ommx::artifact::{self, Builder, InstanceAnnotations};
use std::{fs, io::Write, path::Path, process::Command, time::SystemTime};
use zip::{write::SimpleFileOptions, ZipWriter};

#[test]
fn package_and_load_versioned_miplib_with_legacy_cache() -> Result<()> {
    let root = std::env::temp_dir().join(format!(
        "ommx-miplib-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&root)?;
    let registry = root.join("registry");
    artifact::set_local_registry_root(&registry)?;
    let name = "neos-2626858-aoos";

    // The old distribution is already cached, with a different model.
    let mut legacy = Builder::for_github("Jij-Inc", "ommx", "miplib2017", name)?;
    legacy.add_instance(
        ommx::v1::Instance::default(),
        InstanceAnnotations::default(),
    )?;
    legacy.build()?;

    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ommx/tests/data/mps/neos-2626858-aoos.mps.gz");
    let input = root.join("collection.zip");
    let mut zip = ZipWriter::new(fs::File::create(&input)?);
    let options = SimpleFileOptions::default();
    zip.start_file(format!("{name}.mps.gz"), options)?;
    zip.write_all(&fs::read(source)?)?;
    zip.start_file("air05.mps.gz", options)?;
    zip.write_all(b"NAME BAD\nINDICATORS\nENDATA\n")?;
    zip.start_file("unknown.mps.gz", options)?;
    zip.write_all(b"NAME UNKNOWN\nENDATA\n")?;
    zip.finish()?;

    let report = root.join("report.csv");
    let output = Command::new(env!("CARGO_BIN_EXE_dataset"))
        .arg("miplib2017")
        .arg(&input)
        .arg("--report")
        .arg(&report)
        .env("OMMX_LOCAL_REGISTRY_ROOT", &registry)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut reader = csv::Reader::from_path(&report)?;
    let rows = reader
        .records()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(rows.len(), 3);
    assert_eq!(&rows[0][2], "packaged");
    assert_eq!(&rows[0][5], "209");
    assert_eq!(&rows[0][6], "315");
    assert_eq!(&rows[1][2], "failed");
    assert!(rows[1][3].contains("INDICATORS"));
    assert_eq!(&rows[2][2], "failed");
    assert!(rows[2][3].contains("No metadata"));

    // Rust's default loader must use the newly generated distribution.
    let (instance, _) = ommx::dataset::miplib2017::load(name)?;
    assert_eq!(instance.decision_variables.len(), 524);
    assert_eq!(instance.constraints.len(), 342);
    let targets: Vec<_> = instance
        .decision_variables
        .iter()
        .filter(|v| {
            v.name
                .as_deref()
                .is_some_and(|name| name == "C0524" || ("C0493"..="C0508").contains(&name))
        })
        .collect();
    assert_eq!(targets.len(), 17);
    for variable in targets {
        assert_eq!(
            variable.kind,
            ommx::v1::decision_variable::Kind::Binary as i32
        );
        let bound = variable.bound.as_ref().unwrap();
        assert_eq!((bound.lower, bound.upper), (0.0, 1.0));
    }
    let failed_image = artifact::ghcr("Jij-Inc", "ommx", "v2.7/miplib2017", "air05")?;
    assert!(!artifact::get_image_dir(&failed_image).exists());
    fs::remove_dir_all(&root)?;
    Ok(())
}
