use std::io::{self, Read};

/// Check if a reader starts with gzip magic number (0x1f, 0x8b)
pub fn is_gzipped<R: Read>(mut reader: R) -> io::Result<bool> {
    let mut magic = [0u8; 2];
    match reader.read_exact(&mut magic) {
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            // File is too short to be gzipped
            return Ok(false);
        }
        _ => {}
    }
    Ok(magic == [0x1f, 0x8b])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_gzipped() {
        // Test with gzip magic number
        let gzip_data: Vec<u8> = vec![
            /*  Gzip magic number*/ 0x1f, 0x8b, /* Dummy data */ 0x12, 0x34, 0x56, 0x78,
        ];
        assert!(is_gzipped(gzip_data.as_slice()).unwrap());

        // Test with non-gzip data
        let plain_data = b"NAME TestProblem";
        assert!(!is_gzipped(plain_data.as_slice()).unwrap());

        // Test with empty data
        let empty_data: Vec<u8> = vec![];
        assert!(!is_gzipped(empty_data.as_slice()).unwrap());

        // Test with single byte
        let single_byte = vec![0x1f];
        assert!(!is_gzipped(single_byte.as_slice()).unwrap());
    }
}
