use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl Instance {
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
}
