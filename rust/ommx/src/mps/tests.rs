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
mod file_format_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_current_behavior_with_compressed_file() {
        // Create a simple MPS content
        let mps_content = "NAME TestProblem\nROWS\n N  OBJ\n L  R1\nCOLUMNS\n    X1        OBJ                 1\n    X1        R1                  1\nRHS\n    RHS1      R1                  5\nBOUNDS\n UP BND1      X1                  4\nENDATA\n";
        
        // Create a temporary compressed file
        let temp_dir = std::env::temp_dir();
        let compressed_path = temp_dir.join("test_compressed.mps.gz");
        {
            let file = std::fs::File::create(&compressed_path).unwrap();
            let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            encoder.write_all(mps_content.as_bytes()).unwrap();
            encoder.finish().unwrap();
        }
        
        // This should work with current implementation
        let result = load_file(&compressed_path);
        assert!(result.is_ok(), "Current implementation should load compressed files");
        
        std::fs::remove_file(compressed_path).ok();
    }

    #[test]
    fn test_current_behavior_with_uncompressed_file() {
        // Create a simple MPS content
        let mps_content = "NAME TestProblem\nROWS\n N  OBJ\n L  R1\nCOLUMNS\n    X1        OBJ                 1\n    X1        R1                  1\nRHS\n    RHS1      R1                  5\nBOUNDS\n UP BND1      X1                  4\nENDATA\n";
        
        // Create a temporary uncompressed file
        let temp_dir = std::env::temp_dir();
        let uncompressed_path = temp_dir.join("test_uncompressed.mps");
        std::fs::write(&uncompressed_path, mps_content).unwrap();
        
        // With the fix, this should now work correctly
        let result = load_file(&uncompressed_path);
        assert!(result.is_ok(), "Fixed implementation should load uncompressed files");
        
        // Verify the loaded instance is not empty
        let instance = result.unwrap();
        assert!(!instance.decision_variables.is_empty() || !instance.constraints.is_empty());
        
        std::fs::remove_file(uncompressed_path).ok();
    }

    #[test]
    fn test_file_format_detection() {
        let mps_content = "NAME TestProblem\nROWS\n N  OBJ\n L  R1\nCOLUMNS\n    X1        OBJ                 1\n    X1        R1                  1\nRHS\n    RHS1      R1                  5\nBOUNDS\n UP BND1      X1                  4\nENDATA\n";
        
        let temp_dir = std::env::temp_dir();
        
        // Test uncompressed file
        let uncompressed_path = temp_dir.join("test_format_uncompressed.mps");
        std::fs::write(&uncompressed_path, mps_content).unwrap();
        
        // Test compressed file
        let compressed_path = temp_dir.join("test_format_compressed.mps.gz");
        {
            let file = std::fs::File::create(&compressed_path).unwrap();
            let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            encoder.write_all(mps_content.as_bytes()).unwrap();
            encoder.finish().unwrap();
        }
        
        // Both should work now
        let uncompressed_result = load_file(&uncompressed_path);
        let compressed_result = load_file(&compressed_path);
        
        assert!(uncompressed_result.is_ok(), "Should load uncompressed file");
        assert!(compressed_result.is_ok(), "Should load compressed file");
        
        // Both should produce equivalent results
        let uncompressed_instance = uncompressed_result.unwrap();
        let compressed_instance = compressed_result.unwrap();
        
        assert_eq!(uncompressed_instance.decision_variables.len(), compressed_instance.decision_variables.len());
        assert_eq!(uncompressed_instance.constraints.len(), compressed_instance.constraints.len());
        
        std::fs::remove_file(uncompressed_path).ok();
        std::fs::remove_file(compressed_path).ok();
    }

    #[test]
    fn test_write_with_and_without_compression() {
        // Use the test data from existing roundtrip test
        let instance = crate::v1::Instance::arbitrary_with(crate::random::InstanceParameters::default_lp()).new_tree(&mut proptest::test_runner::TestRunner::deterministic()).unwrap().current();
        
        let temp_dir = std::env::temp_dir();
        let compressed_path = temp_dir.join("test_write_compressed.mps.gz");
        let uncompressed_path = temp_dir.join("test_write_uncompressed.mps");
        
        // Test writing compressed (default)
        let compressed_result = write_file(&instance, &compressed_path);
        assert!(compressed_result.is_ok(), "Should write compressed file");
        
        // Test writing uncompressed
        let uncompressed_result = write_file_with_compression(&instance, &uncompressed_path, false);
        assert!(uncompressed_result.is_ok(), "Should write uncompressed file");
        
        // Both files should be readable
        let read_compressed = load_file(&compressed_path);
        let read_uncompressed = load_file(&uncompressed_path);
        
        assert!(read_compressed.is_ok(), "Should read compressed file");
        assert!(read_uncompressed.is_ok(), "Should read uncompressed file");
        
        // Check file sizes - compressed should be smaller for non-trivial content
        let compressed_size = std::fs::metadata(&compressed_path).unwrap().len();
        let uncompressed_size = std::fs::metadata(&uncompressed_path).unwrap().len();
        
        // For a non-trivial instance, compressed should be smaller
        if uncompressed_size > 100 {
            assert!(compressed_size < uncompressed_size, "Compressed file should be smaller");
        }
        
        std::fs::remove_file(compressed_path).ok();
        std::fs::remove_file(uncompressed_path).ok();
    }
}
