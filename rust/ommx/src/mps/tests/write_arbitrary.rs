use super::super::*;
use crate::Instance;
use approx::AbsDiffEq;
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_write_mps(instance in Instance::arbitrary_with(<Instance as Arbitrary>::Parameters::default_lp())) {
        let mut buffer = Vec::new();
        prop_assert!(format::format(&instance, &mut buffer).is_ok())
    }

    #[test]
    fn test_roundtrip(instance in Instance::arbitrary_with(<Instance as Arbitrary>::Parameters::default_lp())) {
        let mut buffer = Vec::new();
        prop_assert!(format::format(&instance, &mut buffer).is_ok());
        let loaded = parse(&buffer[..]).unwrap();

        prop_assert!(
            loaded.objective().abs_diff_eq(&instance.objective(), crate::ATol::default()),
            "Objective mismatch: {:?} != {:?}", loaded.objective(), instance.objective()
        );
        prop_assert!(
            loaded.constraints().len() == instance.constraints().len(),
            "Constraint count mismatch: {} != {}", loaded.constraints().len(), instance.constraints().len()
        );
        for ((id, constraint), (loaded_id, loaded_constraint)) in
            instance.constraints().iter().zip(loaded.constraints().iter())
        {
            prop_assert_eq!(id, loaded_id, "Constraint ID mismatch");
            prop_assert!(
                loaded_constraint.abs_diff_eq(constraint, crate::ATol::default()),
                "Constraint mismatch for ID {}: {:?} != {:?}", id, loaded_constraint, constraint
            );
        }
        // Some variables may be filtered out, so we only check the ones that are present
        for (id, loaded_variable) in loaded.decision_variables().iter() {
            let original = instance.decision_variables().get(id).unwrap();
            prop_assert_eq!(
                loaded_variable.kind(),
                original.kind(),
                "Variable kind mismatch for ID {}: {:?} != {:?}", id, loaded_variable.kind(), original.kind()
            );
            prop_assert!(
                loaded_variable.bound().abs_diff_eq(&original.bound(), crate::ATol::default()),
                "Variable bound mismatch for ID {}: {:?} != {:?}", id, loaded_variable.bound(), original.bound()
            )
        }

        prop_assert!(instance.abs_diff_eq(&loaded, crate::ATol::default()), "Loaded instance: {:#?}", loaded);
    }
}
