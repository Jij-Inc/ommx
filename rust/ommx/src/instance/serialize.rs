use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl Instance {
    /// Serialize this instance as an OMMX v1 protobuf payload.
    ///
    /// Reserved `org.ommx.v1.*` keys in the extension map are omitted.
    /// Other keys and their string values are preserved without modification.
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_instance = v1::Instance::from(self.clone());
        v1_instance.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::Instance::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl ParametricInstance {
    /// Serialize this parametric instance as an OMMX v1 protobuf payload.
    ///
    /// Reserved `org.ommx.v1.*` keys in the extension map are omitted.
    /// Other keys and their string values are preserved without modification.
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_instance = v1::ParametricInstance::from(self.clone());
        v1_instance.encode_to_vec()
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
    fn serialization_filters_reserved_keys_and_preserves_arbitrary_strings_on_all_roots() {
        let extensions = std::collections::HashMap::from([
            ("".into(), "".into()),
            ("plain key".into(), "not JSON, nor a date\n\0".into()),
            ("日本語\n\0".into(), "任意の値 🦀\n\0".into()),
            ("org.ommx.v1".into(), "No trailing dot".into()),
            ("org.ommx.v10.custom".into(), "Different prefix".into()),
        ]);
        macro_rules! check_root {
            ($root:expr, $ty:ty, $wire_ty:ty) => {{
                let mut root = $root;
                root.annotations = extensions.clone();
                for key in [
                    "org.ommx.v1.",
                    "org.ommx.v1.custom",
                    "org.ommx.v1.instance.title",
                    "org.ommx.v1.solution.solver",
                ] {
                    root.annotations.insert(key.into(), "ignored".into());
                }
                let original = root.annotations.clone();

                let message = <$wire_ty>::from(root.clone());
                assert_eq!(message.annotations, extensions);
                let bytes = root.to_bytes();
                let decoded = <$wire_ty>::decode(bytes.as_slice()).unwrap();
                assert_eq!(decoded.annotations, extensions);
                let restored = <$ty>::from_bytes(&bytes).unwrap();
                assert_eq!(restored.annotations, extensions);
                assert_eq!(root.annotations, original);

                // Typed OMMX metadata remains authoritative over reserved map entries.
                let reencoded = <$wire_ty>::decode(restored.to_bytes().as_slice()).unwrap();
                assert_eq!(reencoded, message);
            }};
        }

        let instance = Instance {
            description: Some(v1::instance::Description {
                name: Some("Typed title".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        check_root!(instance.clone(), Instance, v1::Instance);
        check_root!(
            ParametricInstance::from(instance),
            ParametricInstance,
            v1::ParametricInstance
        );
        let metadata = Some(v1::ProcessMetadata {
            solver: Some("Typed solver".into()),
            ..Default::default()
        });
        let mut solution = Instance::default()
            .evaluate(&v1::State::default(), ATol::default())
            .unwrap();
        solution.metadata = metadata.clone();
        check_root!(solution, Solution, v1::Solution);

        let mut sample_set = SampleSet::builder()
            .decision_variables(Default::default())
            .constraints(Default::default())
            .objectives(crate::Sampled::from(0.0))
            .sense(crate::Sense::Minimize)
            .build()
            .unwrap();
        sample_set.metadata = metadata;
        check_root!(sample_set, SampleSet, v1::SampleSet);
    }
}
