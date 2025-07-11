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
    use std::io::Cursor;

    #[test]
    fn test_is_gzipped() {
        // Test with gzip magic number
        let gzip_data = vec![
            /*  Gzip magic number*/ 0x1f, 0x8b, /* Dummy data */ 0x12, 0x34, 0x56, 0x78,
        ];
        let mut cursor = Cursor::new(gzip_data);
        assert_eq!(is_gzipped(&mut cursor).unwrap(), true);
        assert_eq!(cursor.position(), 0, "Cursor should be rewound");

        // Test with non-gzip data
        let plain_data = b"NAME TestProblem";
        let mut cursor = Cursor::new(plain_data.to_vec());
        assert_eq!(is_gzipped(&mut cursor).unwrap(), false);
        assert_eq!(cursor.position(), 0, "Cursor should be rewound");

        // Test with empty data
        let empty_data: Vec<u8> = vec![];
        let mut cursor = Cursor::new(empty_data);
        assert_eq!(is_gzipped(&mut cursor).unwrap(), false);

        // Test with single byte
        let single_byte = vec![0x1f];
        let mut cursor = Cursor::new(single_byte);
        assert_eq!(is_gzipped(&mut cursor).unwrap(), false);
    }
}
