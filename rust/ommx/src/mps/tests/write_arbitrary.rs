use super::super::*;
use crate::Instance;
use approx::AbsDiffEq;
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_write_mps(instance in Instance::arbitrary_with(<Instance as Arbitrary>::Parameters::default_qcqp())) {
        let mut buffer = Vec::new();
        prop_assert!(format::format(&instance, &mut buffer).is_ok())
    }

    #[test]
    fn test_roundtrip(instance in Instance::arbitrary_with(<Instance as Arbitrary>::Parameters::default_qcqp())) {
        let mut buffer = Vec::new();
        prop_assert!(format::format(&instance, &mut buffer).is_ok());
        let loaded = parse(&buffer[..]).unwrap();
        prop_assert!(instance.abs_diff_eq(&loaded, crate::ATol::default()));
    }
}
