use super::super::*;
use approx::AbsDiffEq;
use std::io::Write;
use tempdir::TempDir;

// Test format detection (compressed vs uncompressed)
#[test]
fn test_format_detection() {
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

// Test write and read with compression
#[test]
fn test_write_file_compressed() {
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

    let instance = load_raw_reader(MPS_CONTENT.as_bytes()).unwrap();
    
    let temp_dir = TempDir::new("test_mps_write").unwrap();
    let compressed_path = temp_dir.path().join("test.mps.gz");
    let uncompressed_path = temp_dir.path().join("test.mps");
    
    // Write compressed
    write_file(&instance, &compressed_path, true).unwrap();
    assert!(compressed_path.exists());
    
    // Write uncompressed
    write_file(&instance, &uncompressed_path, false).unwrap();
    assert!(uncompressed_path.exists());
    
    // Both should load to same instance
    let from_compressed = load_file(&compressed_path).unwrap();
    let from_uncompressed = load_file(&uncompressed_path).unwrap();
    
    assert_eq!(from_compressed, from_uncompressed);
}

// Test automatic format detection based on magic number (not file extension)
#[test]
fn test_magic_number_format_detection() {
    const MPS_CONTENT: &str = r#"NAME AutoDetectProblem
ROWS
 N  OBJ
 L  C1
COLUMNS
    X1        OBJ                 2
    X1        C1                  1
RHS
    RHS1      C1                 10
ENDATA
"#;

    let instance = load_raw_reader(MPS_CONTENT.as_bytes()).unwrap();
    let temp_dir = TempDir::new("test_auto_detect").unwrap();
    
    // Write compressed with .gz extension
    let gz_path = temp_dir.path().join("test.mps.gz");
    write_file(&instance, &gz_path, true).unwrap(); // compress=true
    
    // Write uncompressed with .mps extension
    let mps_path = temp_dir.path().join("test.mps");
    write_file(&instance, &mps_path, false).unwrap(); // compress=false
    
    // Write compressed with misleading .mps extension
    let compressed_with_mps_ext = temp_dir.path().join("compressed.mps");
    write_file(&instance, &compressed_with_mps_ext, true).unwrap(); // compress=true
    
    // Write uncompressed with misleading .gz extension
    let uncompressed_with_gz_ext = temp_dir.path().join("uncompressed.gz");
    write_file(&instance, &uncompressed_with_gz_ext, false).unwrap(); // compress=false
    
    // load_file should correctly detect format based on magic number, not extension
    let from_gz = load_file(&gz_path).unwrap();
    let from_mps = load_file(&mps_path).unwrap();
    let from_compressed_mps = load_file(&compressed_with_mps_ext).unwrap();
    let from_uncompressed_gz = load_file(&uncompressed_with_gz_ext).unwrap();
    
    // All should be equivalent
    assert!(from_gz.abs_diff_eq(&from_mps, crate::ATol::default()));
    assert!(from_gz.abs_diff_eq(&from_compressed_mps, crate::ATol::default()));
    assert!(from_gz.abs_diff_eq(&from_uncompressed_gz, crate::ATol::default()));
}

// Test zipped reader functionality
#[test]
fn test_load_zipped_reader() {
    const MPS_CONTENT: &str = r#"NAME ZippedReaderTest
ROWS
 N  OBJ
 E  C1
COLUMNS
    X1        OBJ                 3
    X1        C1                  2
    X2        OBJ                -1
    X2        C1                 -1
RHS
    RHS1      C1                  0
ENDATA
"#;

    // Create compressed data in memory
    let mut compressed_buffer = Vec::new();
    {
        let mut encoder = flate2::write::GzEncoder::new(&mut compressed_buffer, flate2::Compression::default());
        encoder.write_all(MPS_CONTENT.as_bytes()).unwrap();
        encoder.finish().unwrap();
    }
    
    // Load using zipped reader
    let instance_from_zipped = load_zipped_reader(&compressed_buffer[..]).unwrap();
    
    // Load using raw reader for comparison
    let instance_from_raw = load_raw_reader(MPS_CONTENT.as_bytes()).unwrap();
    
    // Check instances are equivalent (term order may differ)
    assert!(instance_from_zipped.abs_diff_eq(&instance_from_raw, crate::ATol::default()));
}