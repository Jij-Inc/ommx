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
        prop_assert!(instance.abs_diff_eq(&dbg!(loaded_instance), 1e-6))
    }
}
