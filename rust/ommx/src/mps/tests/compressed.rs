use super::super::*;
use approx::AbsDiffEq;
use std::io::{Cursor, Write};
use tempdir::TempDir;

// Test is_gzipped function
#[test]
fn test_is_gzipped_function() {
    // Test with gzip magic number
    let gzip_data = vec![0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut cursor = Cursor::new(&gzip_data);
    assert_eq!(is_gzipped(&mut cursor).unwrap(), true);
    
    // Test with non-gzip data
    let plain_data = b"NAME TestProblem";
    let mut cursor = Cursor::new(&plain_data[..]);
    assert_eq!(is_gzipped(&mut cursor).unwrap(), false);
    
    // Test with empty data
    let empty_data: Vec<u8> = vec![];
    let mut cursor = Cursor::new(&empty_data);
    assert_eq!(is_gzipped(&mut cursor).unwrap(), false);
    
    // Test with single byte
    let single_byte = vec![0x1f];
    let mut cursor = Cursor::new(&single_byte);
    assert_eq!(is_gzipped(&mut cursor).unwrap(), false);
}

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

// Test that write_file actually respects the compress parameter
#[test]
fn test_write_file_compress_parameter() {
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
    let compressed_path = temp_dir.path().join("test_compressed.mps");
    let uncompressed_path = temp_dir.path().join("test_uncompressed.mps");
    
    // Write with compress=true
    write_file(&instance, &compressed_path, true).unwrap();
    
    // Write with compress=false
    write_file(&instance, &uncompressed_path, false).unwrap();
    
    // Check that compressed file starts with gzip magic number
    let compressed_data = std::fs::read(&compressed_path).unwrap();
    assert!(compressed_data.len() >= 2);
    assert_eq!(&compressed_data[0..2], &[0x1f, 0x8b], "File should be gzip compressed");
    
    // Check that uncompressed file does NOT start with gzip magic number
    let uncompressed_data = std::fs::read(&uncompressed_path).unwrap();
    assert!(uncompressed_data.len() >= 2);
    assert_ne!(&uncompressed_data[0..2], &[0x1f, 0x8b], "File should not be gzip compressed");
    
    // Check that uncompressed file starts with valid MPS content
    let uncompressed_str = std::str::from_utf8(&uncompressed_data).unwrap();
    assert!(uncompressed_str.starts_with("NAME TestProblem"), "Uncompressed file should start with NAME");
    
    // Verify both can be loaded and produce the same instance
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