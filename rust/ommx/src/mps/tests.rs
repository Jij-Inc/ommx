use crate::{mps::*, v1::Instance};
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_write_mps(instance in Instance::arbitrary_lp()) {
        let mut buffer = Vec::new();
        prop_assert!(to_mps::write_mps(&instance, &mut buffer).is_ok())
    }
}
