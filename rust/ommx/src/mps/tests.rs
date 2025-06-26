use super::*;
use crate::{random::InstanceParameters, v1::Instance};
use approx::AbsDiffEq;
use proptest::prelude::*;
use std::io::Write;
use tempdir::TempDir;

proptest! {
    #[test]
    fn test_write_mps(instance in Instance::arbitrary_with(InstanceParameters::default_lp())) {
        let mut buffer = Vec::new();
        prop_assert!(to_mps::write_mps(&instance, &mut buffer).is_ok())
    }

    #[test]
    fn test_roundtrip(instance in Instance::arbitrary_with(InstanceParameters::default_lp())) {
        let mut buffer = Vec::new();
        prop_assert!(to_mps::write_mps(&instance, &mut buffer).is_ok());
        let loaded_instance = load_raw_reader(&buffer[..]).unwrap();
        dbg!(&instance);
        prop_assert!(instance.abs_diff_eq(&dbg!(loaded_instance), crate::ATol::default()));
    }
}

const MPS_CONTENT: &str = r#"NAME TestProblem
ROWS
 N  OBJ
 L  R1
COLUMNS
    X1        OBJ                 1
    X1        R1                  1
RHS
    RHS1      R1                  5
BOUNDS
 UP BND1      X1                  4
ENDATA
"#;

#[test]
fn test_format_detection() {
    let temp_dir = TempDir::new("test_mps_format_detection").unwrap();
    let temp_dir_path = temp_dir.path();
    let uncompressed_path = temp_dir_path.join("test.mps");
    let compressed_path = temp_dir_path.join("test.mps.gz");

    // Create uncompressed file
    std::fs::write(&uncompressed_path, MPS_CONTENT).unwrap();

    // Create compressed file
    {
        let file = std::fs::File::create(&compressed_path).unwrap();
        let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        encoder.write_all(MPS_CONTENT.as_bytes()).unwrap();
        encoder.finish().unwrap();
    }

    let uncompressed = load_file(&uncompressed_path).unwrap();
    let compressed = load_file(&compressed_path).unwrap();
    assert_eq!(compressed, uncompressed);
}
