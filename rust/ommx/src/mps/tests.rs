use crate::{mps::*, v1::Instance};
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_write_mps(instance in Instance::arbitrary_lp()) {
        let mut buffer = Vec::new();
        to_mps::write_mps(&instance, &mut buffer).unwrap();
        let parsed = load_reader(buffer.as_slice()).unwrap();
        assert_eq!(instance, parsed);
    }
}
