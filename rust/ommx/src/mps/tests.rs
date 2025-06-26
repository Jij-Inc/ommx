use crate::{mps::*, random::InstanceParameters, v1::Instance};
use approx::AbsDiffEq;
use proptest::prelude::*;

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

#[cfg(test)]
mod format_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_format_detection() {
        let mps_content = r#"NAME TestProblem
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

        let temp_dir = std::env::temp_dir();
        let uncompressed_path = temp_dir.join("test.mps");
        let compressed_path = temp_dir.join("test.mps.gz");

        // Create uncompressed file
        std::fs::write(&uncompressed_path, mps_content).unwrap();

        // Create compressed file
        {
            let file = std::fs::File::create(&compressed_path).unwrap();
            let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            encoder.write_all(mps_content.as_bytes()).unwrap();
            encoder.finish().unwrap();
        }

        // Both should load successfully
        assert!(load_file(&uncompressed_path).is_ok());
        assert!(load_file(&compressed_path).is_ok());

        std::fs::remove_file(uncompressed_path).ok();
        std::fs::remove_file(compressed_path).ok();
    }

    #[test]
    fn test_write_compression() {
        let instance = crate::v1::Instance::arbitrary_with(crate::random::InstanceParameters::default_lp())
            .new_tree(&mut proptest::test_runner::TestRunner::deterministic())
            .unwrap()
            .current();

        let temp_dir = std::env::temp_dir();
        let compressed_path = temp_dir.join("test_compressed.mps");
        let uncompressed_path = temp_dir.join("test_uncompressed.mps");

        // Test both compression modes
        assert!(write_file(&instance, &compressed_path, true).is_ok());
        assert!(write_file(&instance, &uncompressed_path, false).is_ok());

        // Both should be readable
        assert!(load_file(&compressed_path).is_ok());
        assert!(load_file(&uncompressed_path).is_ok());

        std::fs::remove_file(compressed_path).ok();
        std::fs::remove_file(uncompressed_path).ok();
    }
}
