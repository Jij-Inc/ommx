use super::super::*;
use crate::{random::InstanceParameters, v1::Instance};
use approx::AbsDiffEq;
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_write_mps(instance in Instance::arbitrary_with(InstanceParameters::default_lp())) {
        let mut buffer = Vec::new();
        prop_assert!(format::format(&instance, &mut buffer).is_ok())
    }

    #[test]
    fn test_roundtrip(instance in Instance::arbitrary_with(InstanceParameters::default_lp())) {
        let mut buffer = Vec::new();
        prop_assert!(format::format(&instance, &mut buffer).is_ok());
        let loaded_instance = parse(&buffer[..]).unwrap();
        prop_assert!(instance.abs_diff_eq(&loaded_instance, crate::ATol::default()));
    }
}
