use crate::{SampledConstraint, SampledDecisionVariable};
use std::collections::BTreeMap;

use super::*;

impl SampleSet {
    /// Extract decision variable values for a given name and sample ID
    ///
    /// Returns a map from subscripts to values for the specified sample
    pub fn extract_decision_variables(
        &self,
        name: &str,
        sample_id: SampleID,
    ) -> Result<BTreeMap<Vec<i64>, f64>, SampleSetError> {
        // Collect all variables with the given name
        let variables_with_name: Vec<&SampledDecisionVariable> = self
            .decision_variables
            .values()
            .filter(|v| v.metadata.name.as_ref() == Some(&name.to_string()))
            .collect();
        if variables_with_name.is_empty() {
            return Err(SampleSetError::UnknownVariableName {
                name: name.to_string(),
            });
        }
        let mut result = BTreeMap::new();
        for variable in &variables_with_name {
            if !variable.metadata.parameters.is_empty() {
                return Err(SampleSetError::ParameterizedVariable);
            }
            let subscripts = variable.metadata.subscripts.clone();
            let value = *variable.samples().get(sample_id)?;
            if result.insert(subscripts.clone(), value).is_some() {
                return Err(SampleSetError::DuplicateSubscripts {
                    name: name.to_string(),
                    subscripts,
                });
            }
        }
        Ok(result)
    }

    /// Extract constraint values for a given name and sample ID
    ///
    /// Returns a map from subscripts to values for the specified sample
    pub fn extract_constraints(
        &self,
        name: &str,
        sample_id: SampleID,
    ) -> Result<BTreeMap<Vec<i64>, f64>, SampleSetError> {
        // Collect all constraints with the given name
        let constraints_with_name: Vec<&SampledConstraint> = self
            .constraints
            .values()
            .filter(|c| c.metadata.name.as_ref() == Some(&name.to_string()))
            .collect();
        if constraints_with_name.is_empty() {
            return Err(SampleSetError::UnknownConstraintName {
                name: name.to_string(),
            });
        }
        let mut result = BTreeMap::new();
        for constraint in &constraints_with_name {
            if !constraint.metadata.parameters.is_empty() {
                return Err(SampleSetError::ParameterizedConstraint);
            }
            let subscripts = constraint.metadata.subscripts.clone();
            let value = *constraint.evaluated_values().get(sample_id)?;
            if result.insert(subscripts.clone(), value).is_some() {
                return Err(SampleSetError::DuplicateSubscripts {
                    name: name.to_string(),
                    subscripts,
                });
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        decision_variable::{DecisionVariable, DecisionVariableMetadata, Kind},
        SampleID, SampledDecisionVariable, Sense, VariableID,
    };

    #[test]
    fn test_extract_decision_variables() {
        // Create sample decision variables with metadata
        let mut decision_variables = BTreeMap::new();

        // Variable x[0]
        let mut dv1 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            crate::Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap(),
            None,
            crate::ATol::default(),
        )
        .unwrap();
        dv1.metadata = DecisionVariableMetadata {
            name: Some("x".to_string()),
            subscripts: vec![0],
            ..Default::default()
        };

        // Variable x[1]
        let mut dv2 = DecisionVariable::new(
            VariableID::from(2),
            Kind::Continuous,
            crate::Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap(),
            None,
            crate::ATol::default(),
        )
        .unwrap();
        dv2.metadata = DecisionVariableMetadata {
            name: Some("x".to_string()),
            subscripts: vec![1],
            ..Default::default()
        };

        // Variable y[0]
        let mut dv3 = DecisionVariable::new(
            VariableID::from(3),
            Kind::Continuous,
            crate::Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap(),
            None,
            crate::ATol::default(),
        )
        .unwrap();
        dv3.metadata = DecisionVariableMetadata {
            name: Some("y".to_string()),
            subscripts: vec![0],
            ..Default::default()
        };

        // Create sampled values
        let mut x0_samples = crate::Sampled::default();
        x0_samples.append([SampleID::from(0)], 1.0).unwrap();
        x0_samples.append([SampleID::from(1)], 2.0).unwrap();

        let mut x1_samples = crate::Sampled::default();
        x1_samples.append([SampleID::from(0)], 3.0).unwrap();
        x1_samples.append([SampleID::from(1)], 4.0).unwrap();

        let mut y0_samples = crate::Sampled::default();
        y0_samples.append([SampleID::from(0)], 5.0).unwrap();
        y0_samples.append([SampleID::from(1)], 6.0).unwrap();

        decision_variables.insert(
            VariableID::from(1),
            SampledDecisionVariable::new(dv1, x0_samples, crate::ATol::default()).unwrap(),
        );
        decision_variables.insert(
            VariableID::from(2),
            SampledDecisionVariable::new(dv2, x1_samples, crate::ATol::default()).unwrap(),
        );
        decision_variables.insert(
            VariableID::from(3),
            SampledDecisionVariable::new(dv3, y0_samples, crate::ATol::default()).unwrap(),
        );

        // Create objectives
        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 10.0).unwrap();
        objectives.append([SampleID::from(1)], 20.0).unwrap();

        // Create sample set
        let sample_set = SampleSet::new(
            decision_variables,
            objectives,
            BTreeMap::new(),
            Sense::Minimize,
        )
        .unwrap();

        // Test extracting variables with name "x" for sample 0
        let sample0_x = sample_set
            .extract_decision_variables("x", SampleID::from(0))
            .unwrap();
        assert_eq!(sample0_x.get(&vec![0]), Some(&1.0));
        assert_eq!(sample0_x.get(&vec![1]), Some(&3.0));
        assert_eq!(sample0_x.len(), 2);

        // Test extracting variables with name "x" for sample 1
        let sample1_x = sample_set
            .extract_decision_variables("x", SampleID::from(1))
            .unwrap();
        assert_eq!(sample1_x.get(&vec![0]), Some(&2.0));
        assert_eq!(sample1_x.get(&vec![1]), Some(&4.0));
        assert_eq!(sample1_x.len(), 2);

        // Test extracting variables with name "y" for sample 0
        let sample0_y = sample_set
            .extract_decision_variables("y", SampleID::from(0))
            .unwrap();
        assert_eq!(sample0_y.get(&vec![0]), Some(&5.0));
        assert_eq!(sample0_y.len(), 1);

        // Test extracting variables with name "y" for sample 1
        let sample1_y = sample_set
            .extract_decision_variables("y", SampleID::from(1))
            .unwrap();
        assert_eq!(sample1_y.get(&vec![0]), Some(&6.0));
        assert_eq!(sample1_y.len(), 1);

        // Test error case: unknown variable name
        let result = sample_set.extract_decision_variables("z", SampleID::from(0));
        assert!(matches!(
            result,
            Err(crate::SampleSetError::UnknownVariableName { name }) if name == "z"
        ));
    }

    #[test]
    fn test_extract_duplicate_subscripts() {
        use crate::{
            decision_variable::{DecisionVariable, DecisionVariableMetadata, Kind},
            SampleID, SampledDecisionVariable, Sense, VariableID,
        };

        // Create two decision variables with the same name and subscripts (should cause error)
        let mut decision_variables = BTreeMap::new();

        let mut dv1 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            crate::Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap(),
            None,
            crate::ATol::default(),
        )
        .unwrap();
        dv1.metadata = DecisionVariableMetadata {
            name: Some("x".to_string()),
            subscripts: vec![0],
            ..Default::default()
        };

        let mut dv2 = DecisionVariable::new(
            VariableID::from(2),
            Kind::Continuous,
            crate::Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap(),
            None,
            crate::ATol::default(),
        )
        .unwrap();
        dv2.metadata = DecisionVariableMetadata {
            name: Some("x".to_string()),
            subscripts: vec![0], // Same subscripts as dv1
            ..Default::default()
        };

        // Create sampled values
        let mut samples1 = crate::Sampled::default();
        samples1.append([SampleID::from(0)], 1.0).unwrap();

        let mut samples2 = crate::Sampled::default();
        samples2.append([SampleID::from(0)], 2.0).unwrap();

        decision_variables.insert(
            VariableID::from(1),
            SampledDecisionVariable::new(dv1, samples1, crate::ATol::default()).unwrap(),
        );
        decision_variables.insert(
            VariableID::from(2),
            SampledDecisionVariable::new(dv2, samples2, crate::ATol::default()).unwrap(),
        );

        // Create objectives
        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 10.0).unwrap();

        // Create sample set
        let sample_set = SampleSet::new(
            decision_variables,
            objectives,
            BTreeMap::new(),
            Sense::Minimize,
        )
        .unwrap();

        // Test that extracting variables with duplicate subscripts fails
        let result = sample_set.extract_decision_variables("x", SampleID::from(0));
        assert!(matches!(
            result,
            Err(crate::SampleSetError::DuplicateSubscripts { name, subscripts })
            if name == "x" && subscripts == vec![0]
        ));
    }

    #[test]
    fn test_extract_constraints_unknown_name() {
        // Create a simple sample set with no constraints
        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 10.0).unwrap();

        let sample_set = SampleSet::new(
            BTreeMap::new(),
            objectives,
            BTreeMap::new(),
            Sense::Minimize,
        )
        .unwrap();

        // Test error case: unknown constraint name
        let result = sample_set.extract_constraints("nonexistent", SampleID::from(0));
        assert!(matches!(
            result,
            Err(crate::SampleSetError::UnknownConstraintName { name }) if name == "nonexistent"
        ));
    }

    #[test]
    fn test_extract_parameterized_variable_error() {
        use crate::{
            decision_variable::{DecisionVariable, DecisionVariableMetadata, Kind},
            SampleID, SampledDecisionVariable, Sense, VariableID,
        };

        // Create a parameterized decision variable (should cause error)
        let mut decision_variables = BTreeMap::new();

        let mut dv = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            crate::Bound::new(f64::NEG_INFINITY, f64::INFINITY).unwrap(),
            None,
            crate::ATol::default(),
        )
        .unwrap();
        dv.metadata = DecisionVariableMetadata {
            name: Some("x".to_string()),
            subscripts: vec![0],
            parameters: {
                let mut params = fnv::FnvHashMap::default();
                params.insert("param1".to_string(), "value1".to_string());
                params
            }, // This makes it parameterized
            ..Default::default()
        };

        // Create sampled values
        let mut samples = crate::Sampled::default();
        samples.append([SampleID::from(0)], 1.0).unwrap();

        decision_variables.insert(
            VariableID::from(1),
            SampledDecisionVariable::new(dv, samples, crate::ATol::default()).unwrap(),
        );

        // Create objectives
        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 10.0).unwrap();

        // Create sample set
        let sample_set = SampleSet::new(
            decision_variables,
            objectives,
            BTreeMap::new(),
            Sense::Minimize,
        )
        .unwrap();

        // Test that extracting parameterized variable fails
        let result = sample_set.extract_decision_variables("x", SampleID::from(0));
        assert!(matches!(
            result,
            Err(crate::SampleSetError::ParameterizedVariable)
        ));
    }
}
