use super::*;
use crate::{v1, Message, Parse, ParseError};
use anyhow::Result;

impl Instance {
    /// Serialize this instance as an OMMX v1 protobuf payload.
    ///
    /// # Panics
    /// Panics if an extension annotation uses the reserved `org.ommx.v1.` namespace.
    /// Use [`Self::try_to_bytes`] to handle validation errors.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.try_to_bytes()
            .expect("Cannot serialize invalid OMMX extension annotations")
    }

    /// Serialize this instance after validating extension annotation keys.
    pub fn try_to_bytes(&self) -> Result<Vec<u8>, ParseError> {
        crate::parse::validate_extension_annotations(&self.annotations, "ommx.v1.Instance")?;
        let v1_instance = v1::Instance::from(self.clone());
        Ok(v1_instance.encode_to_vec())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::Instance::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl ParametricInstance {
    /// Serialize this parametric instance as an OMMX v1 protobuf payload.
    ///
    /// # Panics
    /// Panics if an extension annotation uses the reserved `org.ommx.v1.` namespace.
    /// Use [`Self::try_to_bytes`] to handle validation errors.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.try_to_bytes()
            .expect("Cannot serialize invalid OMMX extension annotations")
    }

    /// Serialize this parametric instance after validating extension annotation keys.
    pub fn try_to_bytes(&self) -> Result<Vec<u8>, ParseError> {
        crate::parse::validate_extension_annotations(
            &self.annotations,
            "ommx.v1.ParametricInstance",
        )?;
        let v1_instance = v1::ParametricInstance::from(self.clone());
        Ok(v1_instance.encode_to_vec())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::ParametricInstance::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

#[cfg(test)]
mod annotation_tests {
    use super::*;
    use crate::{ATol, Evaluate, SampleSet, Solution};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn instance_annotations_roundtrip(
            mut instance in Instance::arbitrary(),
            annotations in proptest::collection::hash_map("com\\.example\\.[a-z]{1,8}", ".*", 0..5),
            license in proptest::option::of(".*"),
            dataset in proptest::option::of(".*"),
            created in proptest::option::of(".*"),
        ) {
            instance.annotations = annotations;
            instance.description = Some(v1::instance::Description {
                license,
                dataset,
                created,
                ..Default::default()
            });
            let restored = Instance::from_bytes(&instance.to_bytes()).unwrap();
            prop_assert_eq!(&restored, &instance);

            let parametric: ParametricInstance = instance.clone().into();
            let restored = ParametricInstance::from_bytes(&parametric.to_bytes()).unwrap();
            prop_assert_eq!(&restored, &parametric);
            let materialized = restored.with_parameters(v1::Parameters::default()).unwrap();
            prop_assert_eq!(materialized.annotations, instance.annotations);
            prop_assert_eq!(materialized.description, instance.description);
        }

        #[test]
        fn process_annotations_roundtrip(
            annotations in proptest::collection::hash_map("com\\.example\\.[a-z]{1,8}", ".*", 0..5),
            instance in proptest::option::of(".*"),
            solver in proptest::option::of(".*"),
            parameters in proptest::option::of(".*"),
            start in proptest::option::of(".*"),
            end in proptest::option::of(".*"),
        ) {
            let metadata = Some(v1::ProcessMetadata { instance, solver, parameters, start, end });
            let mut solution = Instance::default().evaluate(&v1::State::default(), ATol::default()).unwrap();
            solution.annotations = annotations.clone();
            solution.metadata = metadata.clone();
            let restored = Solution::from_bytes(&solution.to_bytes()).unwrap();
            prop_assert_eq!(restored, solution);

            let mut sample_set = SampleSet::builder()
                .decision_variables(Default::default())
                .constraints(Default::default())
                .objectives(crate::Sampled::from(0.0))
                .sense(crate::Sense::Minimize)
                .build().unwrap();
            sample_set.annotations = annotations.clone();
            sample_set.metadata = metadata.clone();
            let restored = SampleSet::from_bytes(&sample_set.to_bytes()).unwrap();
            prop_assert_eq!(&restored.annotations, &annotations);
            prop_assert_eq!(&restored.metadata, &metadata);
            let projected = restored.get(0.into()).unwrap();
            prop_assert_eq!(projected.annotations, annotations);
            prop_assert_eq!(projected.metadata, metadata);
        }
    }

    #[test]
    fn reserved_annotations_are_rejected_on_all_roots() {
        let annotations =
            std::collections::HashMap::from([("org.ommx.v1.custom".into(), "invalid".into())]);
        let instance = v1::Instance {
            annotations: annotations.clone(),
            ..Default::default()
        };
        assert!(instance
            .parse(&())
            .unwrap_err()
            .to_string()
            .contains("reserved"));
        let instance = v1::ParametricInstance {
            annotations: annotations.clone(),
            ..Default::default()
        };
        assert!(instance
            .parse(&())
            .unwrap_err()
            .to_string()
            .contains("reserved"));
        let solution = v1::Solution {
            annotations: annotations.clone(),
            ..Default::default()
        };
        assert!(solution
            .parse(&())
            .unwrap_err()
            .to_string()
            .contains("reserved"));
        let sample_set = v1::SampleSet {
            annotations,
            ..Default::default()
        };
        assert!(sample_set
            .parse(&())
            .unwrap_err()
            .to_string()
            .contains("reserved"));
    }

    #[test]
    fn reserved_annotations_are_rejected_before_serializing_all_roots() {
        macro_rules! check_root {
            ($root:expr, $ty:ty, $message:literal) => {{
                let mut root = $root;
                let key = "org.ommx.v1.custom";
                root.annotations.insert(key.into(), "invalid".into());

                let error = root.try_to_bytes().unwrap_err();
                assert!(matches!(
                    error.error,
                    crate::RawParseError::ReservedAnnotationKey { key: reserved } if reserved == key
                ));
                assert_eq!(error.context[0].message, $message);
                assert_eq!(error.context[0].field, "annotations");
                assert!(std::panic::catch_unwind(|| root.to_bytes()).is_err());
                assert_eq!(root.annotations[key], "invalid");

                // A rejected write does not prevent recovery and a valid round-trip.
                root.annotations.remove(key);
                root.annotations.insert("com.example.owner".into(), "Alice".into());
                let restored = <$ty>::from_bytes(&root.try_to_bytes().unwrap()).unwrap();
                assert_eq!(restored.annotations, root.annotations);
            }};
        }

        check_root!(Instance::default(), Instance, "ommx.v1.Instance");
        check_root!(
            ParametricInstance::from(Instance::default()),
            ParametricInstance,
            "ommx.v1.ParametricInstance"
        );
        check_root!(
            Instance::default()
                .evaluate(&v1::State::default(), ATol::default())
                .unwrap(),
            Solution,
            "ommx.v1.Solution"
        );
        check_root!(
            SampleSet::builder()
                .decision_variables(Default::default())
                .constraints(Default::default())
                .objectives(crate::Sampled::from(0.0))
                .sense(crate::Sense::Minimize)
                .build()
                .unwrap(),
            SampleSet,
            "ommx.v1.SampleSet"
        );
    }
}
