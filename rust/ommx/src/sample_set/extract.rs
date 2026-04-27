use crate::{SampledConstraint, SampledDecisionVariable};
use std::collections::BTreeMap;

use super::*;

impl SampleSet {
    /// Get all unique decision variable names in this sample set
    ///
    /// Returns a set of all unique variable names that have at least one named variable.
    /// Variables without names are not included.
    pub fn decision_variable_names(&self) -> std::collections::BTreeSet<String> {
        self.decision_variables
            .keys()
            .filter_map(|id| self.variable_metadata.name(*id).map(|s| s.to_owned()))
            .collect()
    }

    /// Get all unique named function names in this sample set
    ///
    /// Returns a set of all unique named function names that have at least one named function.
    pub fn named_function_names(&self) -> std::collections::BTreeSet<String> {
        self.named_functions
            .values()
            .filter_map(|nf| nf.name.clone())
            .collect()
    }

    /// Extract decision variable values for a given name and sample ID
    ///
    /// Returns a map from subscripts to values for the specified sample.
    ///
    /// Note: Parameters in decision variable metadata are ignored. Only subscripts are used as keys.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No decision variables with the given name are found
    /// - The same subscript is found multiple times (which can happen when parameters differ)
    /// - The sample ID is not found
    pub fn extract_decision_variables(
        &self,
        name: &str,
        sample_id: SampleID,
    ) -> Result<BTreeMap<Vec<i64>, f64>, SampleSetError> {
        // Collect all variables with the given name (looked up via the metadata store)
        let variables_with_name: Vec<(VariableID, &SampledDecisionVariable)> = self
            .decision_variables
            .iter()
            .filter(|(id, _)| self.variable_metadata.name(**id) == Some(name))
            .map(|(id, v)| (*id, v))
            .collect();
        if variables_with_name.is_empty() {
            return Err(SampleSetError::UnknownVariableName {
                name: name.to_string(),
            });
        }
        let mut result = BTreeMap::new();
        for (id, variable) in &variables_with_name {
            let subscripts = self.variable_metadata.subscripts(*id).to_vec();
            let value = *variable
                .samples()
                .get(sample_id)
                .ok_or(SampleSetError::UnknownSampleID { id: sample_id })?;
            if result.insert(subscripts.clone(), value).is_some() {
                return Err(SampleSetError::DuplicateSubscripts {
                    name: name.to_string(),
                    subscripts,
                });
            }
        }
        Ok(result)
    }

    /// Extract all decision variables grouped by name for a given sample ID
    ///
    /// Returns a mapping from variable name to a mapping from subscripts to values.
    /// This is useful for extracting all variables at once in a structured format.
    /// Variables without names are not included in the result.
    ///
    /// Note: Parameters in decision variable metadata are ignored. Only subscripts are used as keys.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The same name and subscript combination is found multiple times (which can happen when parameters differ)
    /// - The sample ID is invalid
    ///
    pub fn extract_all_decision_variables(
        &self,
        sample_id: SampleID,
    ) -> Result<BTreeMap<String, BTreeMap<Vec<i64>, f64>>, SampleSetError> {
        let mut result: BTreeMap<String, BTreeMap<Vec<i64>, f64>> = BTreeMap::new();

        for (id, variable) in self.decision_variables.iter() {
            let name = match self.variable_metadata.name(*id) {
                Some(n) => n.to_owned(),
                None => continue, // Skip variables without names
            };

            let subscripts = self.variable_metadata.subscripts(*id).to_vec();
            let value = *variable
                .samples()
                .get(sample_id)
                .ok_or(SampleSetError::UnknownSampleID { id: sample_id })?;

            let vars_map = result.entry(name.clone()).or_default();
            if vars_map.contains_key(&subscripts) {
                return Err(SampleSetError::DuplicateSubscripts { name, subscripts });
            }
            vars_map.insert(subscripts, value);
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
        // Collect all constraints with the given name (via the collection's metadata store)
        let metadata = self.constraints.metadata();
        let matches: Vec<(crate::ConstraintID, &SampledConstraint)> = self
            .constraints
            .iter()
            .filter(|(id, _)| metadata.name(**id) == Some(name))
            .map(|(id, c)| (*id, c))
            .collect();
        if matches.is_empty() {
            return Err(SampleSetError::UnknownConstraintName {
                name: name.to_string(),
            });
        }
        let mut result = BTreeMap::new();
        for (id, constraint) in &matches {
            if !metadata.parameters(*id).is_empty() {
                return Err(SampleSetError::ParameterizedConstraint);
            }
            let subscripts = metadata.subscripts(*id).to_vec();
            let value = *constraint
                .stage
                .evaluated_values
                .get(sample_id)
                .ok_or(SampleSetError::UnknownSampleID { id: sample_id })?;
            if result.insert(subscripts.clone(), value).is_some() {
                return Err(SampleSetError::DuplicateSubscripts {
                    name: name.to_string(),
                    subscripts,
                });
            }
        }
        Ok(result)
    }

    /// Extract all named functions grouped by name for a given sample ID
    ///
    /// Returns a mapping from function name to a mapping from subscripts to values.
    /// This is useful for extracting all functions at once in a structured format.
    /// Functions without names are not included in the result.
    ///
    /// Note: Parameters in named function are ignored. Only subscripts are used as keys.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The same name and subscript combination is found multiple times
    /// - The sample ID is invalid
    ///
    pub fn extract_all_named_functions(
        &self,
        sample_id: SampleID,
    ) -> Result<BTreeMap<String, BTreeMap<Vec<i64>, f64>>, SampleSetError> {
        let mut result: BTreeMap<String, BTreeMap<Vec<i64>, f64>> = BTreeMap::new();

        for nf in self.named_functions.values() {
            let name = match &nf.name {
                Some(n) => n.clone(),
                None => continue, // Skip functions without names
            };

            let subscripts = nf.subscripts.clone();
            let value = *nf
                .evaluated_values()
                .get(sample_id)
                .ok_or(SampleSetError::UnknownSampleID { id: sample_id })?;

            let vars_map = result.entry(name.clone()).or_default();
            if vars_map.contains_key(&subscripts) {
                return Err(SampleSetError::DuplicateSubscripts { name, subscripts });
            }
            vars_map.insert(subscripts, value);
        }

        Ok(result)
    }

    /// Extract named function values for a given name and sample ID
    ///
    /// Returns a map from subscripts to values for the specified sample.
    ///
    /// Note: Parameters in named function are ignored. Only subscripts are used as keys.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The same name and subscript combination is found multiple times
    /// - The sample ID is invalid
    /// - No named function with the given name is found
    pub fn extract_named_functions(
        &self,
        name: &str,
        sample_id: SampleID,
    ) -> Result<BTreeMap<Vec<i64>, f64>, SampleSetError> {
        // Collect all named functions with the given name
        let named_functions_with_name: Vec<&SampledNamedFunction> = self
            .named_functions
            .values()
            .filter(|nf| nf.name.as_deref() == Some(name))
            .collect();
        if named_functions_with_name.is_empty() {
            return Err(SampleSetError::UnknownNamedFunctionName {
                name: name.to_string(),
            });
        }
        let mut result = BTreeMap::new();
        for nf in &named_functions_with_name {
            let subscripts = nf.subscripts().clone();
            let value = *nf
                .evaluated_values()
                .get(sample_id)
                .ok_or(SampleSetError::UnknownSampleID { id: sample_id })?;
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
        let sample_set = SampleSet::builder()
            .decision_variables(decision_variables)
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
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
        let sample_set = SampleSet::builder()
            .decision_variables(decision_variables)
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
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

        let sample_set = SampleSet::builder()
            .decision_variables(BTreeMap::new())
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        // Test error case: unknown constraint name
        let result = sample_set.extract_constraints("nonexistent", SampleID::from(0));
        assert!(matches!(
            result,
            Err(crate::SampleSetError::UnknownConstraintName { name }) if name == "nonexistent"
        ));
    }

    #[test]
    fn test_decision_variable_names() {
        use crate::{
            decision_variable::{DecisionVariable, DecisionVariableMetadata, Kind},
            SampleID, SampledDecisionVariable, Sense, VariableID,
        };

        // Create sample decision variables with different names
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
            subscripts: vec![1],
            ..Default::default()
        };

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

        let mut samples1 = crate::Sampled::default();
        samples1.append([SampleID::from(0)], 1.0).unwrap();

        let mut samples2 = crate::Sampled::default();
        samples2.append([SampleID::from(0)], 2.0).unwrap();

        let mut samples3 = crate::Sampled::default();
        samples3.append([SampleID::from(0)], 3.0).unwrap();

        decision_variables.insert(
            VariableID::from(1),
            SampledDecisionVariable::new(dv1, samples1, crate::ATol::default()).unwrap(),
        );
        decision_variables.insert(
            VariableID::from(2),
            SampledDecisionVariable::new(dv2, samples2, crate::ATol::default()).unwrap(),
        );
        decision_variables.insert(
            VariableID::from(3),
            SampledDecisionVariable::new(dv3, samples3, crate::ATol::default()).unwrap(),
        );

        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 10.0).unwrap();

        let sample_set = SampleSet::builder()
            .decision_variables(decision_variables)
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        // Test that we get both "x" and "y" names
        let names = sample_set.decision_variable_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains("x"));
        assert!(names.contains("y"));
    }

    #[test]
    fn test_extract_all_decision_variables() {
        use crate::{
            decision_variable::{DecisionVariable, DecisionVariableMetadata, Kind},
            SampleID, SampledDecisionVariable, Sense, VariableID,
        };

        // Create sample decision variables with different names
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
            subscripts: vec![1],
            ..Default::default()
        };

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

        let mut samples1 = crate::Sampled::default();
        samples1.append([SampleID::from(0)], 1.0).unwrap();

        let mut samples2 = crate::Sampled::default();
        samples2.append([SampleID::from(0)], 3.0).unwrap();

        let mut samples3 = crate::Sampled::default();
        samples3.append([SampleID::from(0)], 5.0).unwrap();

        decision_variables.insert(
            VariableID::from(1),
            SampledDecisionVariable::new(dv1, samples1, crate::ATol::default()).unwrap(),
        );
        decision_variables.insert(
            VariableID::from(2),
            SampledDecisionVariable::new(dv2, samples2, crate::ATol::default()).unwrap(),
        );
        decision_variables.insert(
            VariableID::from(3),
            SampledDecisionVariable::new(dv3, samples3, crate::ATol::default()).unwrap(),
        );

        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 10.0).unwrap();

        let sample_set = SampleSet::builder()
            .decision_variables(decision_variables)
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        // Test extract_all_decision_variables
        let all_vars = sample_set
            .extract_all_decision_variables(SampleID::from(0))
            .unwrap();

        // Check we have both "x" and "y"
        assert_eq!(all_vars.len(), 2);
        assert!(all_vars.contains_key("x"));
        assert!(all_vars.contains_key("y"));

        // Check "x" has 2 subscripts
        let x_vars = all_vars.get("x").unwrap();
        assert_eq!(x_vars.len(), 2);
        assert_eq!(x_vars.get(&vec![0]), Some(&1.0));
        assert_eq!(x_vars.get(&vec![1]), Some(&3.0));

        // Check "y" has 1 subscript
        let y_vars = all_vars.get("y").unwrap();
        assert_eq!(y_vars.len(), 1);
        assert_eq!(y_vars.get(&vec![0]), Some(&5.0));
    }

    #[test]
    fn test_extract_parameterized_variable_success() {
        use crate::{
            decision_variable::{DecisionVariable, DecisionVariableMetadata, Kind},
            SampleID, SampledDecisionVariable, Sense, VariableID,
        };

        // Create a parameterized decision variable (should succeed - parameters are ignored)
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
            }, // Parameters are ignored
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
        let sample_set = SampleSet::builder()
            .decision_variables(decision_variables)
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        // Test that extracting parameterized variable succeeds (parameters are ignored)
        let result = sample_set.extract_decision_variables("x", SampleID::from(0));
        assert!(result.is_ok());
        let vars = result.unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[&vec![0]], 1.0);
    }

    #[test]
    fn test_extract_duplicate_subscripts_error() {
        use crate::{
            decision_variable::{DecisionVariable, DecisionVariableMetadata, Kind},
            SampleID, SampledDecisionVariable, Sense, VariableID,
        };

        // Create two variables with same name and subscripts but different parameters
        // This should cause a DuplicateSubscripts error
        let mut decision_variables = BTreeMap::new();

        // First variable with param1
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
            parameters: {
                let mut params = fnv::FnvHashMap::default();
                params.insert("param".to_string(), "value1".to_string());
                params
            },
            ..Default::default()
        };

        let mut samples1 = crate::Sampled::default();
        samples1.append([SampleID::from(0)], 1.0).unwrap();

        decision_variables.insert(
            VariableID::from(1),
            SampledDecisionVariable::new(dv1, samples1, crate::ATol::default()).unwrap(),
        );

        // Second variable with param2 but same name and subscripts
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
            parameters: {
                let mut params = fnv::FnvHashMap::default();
                params.insert("param".to_string(), "value2".to_string()); // Different parameter value
                params
            },
            ..Default::default()
        };

        let mut samples2 = crate::Sampled::default();
        samples2.append([SampleID::from(0)], 2.0).unwrap();

        decision_variables.insert(
            VariableID::from(2),
            SampledDecisionVariable::new(dv2, samples2, crate::ATol::default()).unwrap(),
        );

        // Create objectives
        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 10.0).unwrap();

        // Create sample set
        let sample_set = SampleSet::builder()
            .decision_variables(decision_variables)
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        // Test that extracting variables with duplicate subscripts fails
        let result = sample_set.extract_decision_variables("x", SampleID::from(0));
        assert!(matches!(
            result,
            Err(crate::SampleSetError::DuplicateSubscripts { .. })
        ));
    }

    /// Helper to create a SampledNamedFunction via the Parse trait from v1 types.
    fn make_sampled_named_function(
        id: u64,
        entries: Vec<(Vec<u64>, f64)>,
        name: Option<&str>,
        subscripts: Vec<i64>,
        parameters: std::collections::HashMap<String, String>,
        description: Option<&str>,
    ) -> crate::SampledNamedFunction {
        use crate::parse::Parse;
        let v1_entries = entries
            .into_iter()
            .map(|(ids, value)| crate::v1::sampled_values::SampledValuesEntry { ids, value })
            .collect();
        let v1_snf = crate::v1::SampledNamedFunction {
            id,
            evaluated_values: Some(crate::v1::SampledValues {
                entries: v1_entries,
            }),
            name: name.map(|s| s.to_string()),
            subscripts,
            parameters,
            description: description.map(|s| s.to_string()),
            used_decision_variable_ids: vec![],
        };
        v1_snf.parse(&()).unwrap()
    }

    #[test]
    fn test_extract_named_functions() {
        // SampleSet with named functions: extract by name + sample_id
        let mut named_functions = BTreeMap::new();

        // Named function "cost" with subscript [0]
        let snf1 = make_sampled_named_function(
            1,
            vec![(vec![0], 10.0), (vec![1], 11.0)],
            Some("cost"),
            vec![0],
            Default::default(),
            None,
        );
        named_functions.insert(crate::NamedFunctionID::from(1), snf1);

        // Named function "cost" with subscript [1]
        let snf2 = make_sampled_named_function(
            2,
            vec![(vec![0], 20.0), (vec![1], 21.0)],
            Some("cost"),
            vec![1],
            Default::default(),
            None,
        );
        named_functions.insert(crate::NamedFunctionID::from(2), snf2);

        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 0.0).unwrap();
        objectives.append([SampleID::from(1)], 0.0).unwrap();

        let sample_set = SampleSet::builder()
            .decision_variables(BTreeMap::new())
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .named_functions(named_functions)
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        // Extract for sample 0
        let result = sample_set
            .extract_named_functions("cost", SampleID::from(0))
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[&vec![0]], 10.0);
        assert_eq!(result[&vec![1]], 20.0);

        // Extract for sample 1
        let result = sample_set
            .extract_named_functions("cost", SampleID::from(1))
            .unwrap();
        assert_eq!(result[&vec![0]], 11.0);
        assert_eq!(result[&vec![1]], 21.0);
    }

    #[test]
    fn test_extract_named_functions_unknown_name() {
        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 0.0).unwrap();

        let sample_set = SampleSet::builder()
            .decision_variables(BTreeMap::new())
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        let result = sample_set.extract_named_functions("nonexistent", SampleID::from(0));
        assert!(matches!(
            result,
            Err(crate::SampleSetError::UnknownNamedFunctionName { name }) if name == "nonexistent"
        ));
    }

    #[test]
    fn test_extract_named_functions_with_parameters() {
        // Parameters are now allowed (ignored) - only subscripts are used as keys
        let mut named_functions = BTreeMap::new();

        let snf = make_sampled_named_function(
            1,
            vec![(vec![0], 5.0)],
            Some("f"),
            vec![0],
            [("param".to_string(), "value".to_string())]
                .into_iter()
                .collect(),
            None,
        );
        named_functions.insert(crate::NamedFunctionID::from(1), snf);

        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 0.0).unwrap();

        let sample_set = SampleSet::builder()
            .decision_variables(BTreeMap::new())
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .named_functions(named_functions)
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        // Should succeed - parameters are ignored
        let result = sample_set
            .extract_named_functions("f", SampleID::from(0))
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&vec![0]], 5.0);
    }

    #[test]
    fn test_extract_all_named_functions() {
        let mut named_functions = BTreeMap::new();

        // "cost" [0]
        let snf1 = make_sampled_named_function(
            1,
            vec![(vec![0], 10.0)],
            Some("cost"),
            vec![0],
            Default::default(),
            None,
        );
        named_functions.insert(crate::NamedFunctionID::from(1), snf1);

        // "penalty" [0]
        let snf2 = make_sampled_named_function(
            2,
            vec![(vec![0], 5.0)],
            Some("penalty"),
            vec![0],
            Default::default(),
            None,
        );
        named_functions.insert(crate::NamedFunctionID::from(2), snf2);

        // Unnamed (should be skipped)
        let snf3 = make_sampled_named_function(
            3,
            vec![(vec![0], 99.0)],
            None,
            vec![],
            Default::default(),
            None,
        );
        named_functions.insert(crate::NamedFunctionID::from(3), snf3);

        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 0.0).unwrap();

        let sample_set = SampleSet::builder()
            .decision_variables(BTreeMap::new())
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .named_functions(named_functions)
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        let result = sample_set
            .extract_all_named_functions(SampleID::from(0))
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("cost"));
        assert!(result.contains_key("penalty"));
    }

    #[test]
    fn test_named_function_names() {
        let mut named_functions = BTreeMap::new();

        let snf1 = make_sampled_named_function(
            1,
            vec![(vec![0], 1.0)],
            Some("alpha"),
            vec![],
            Default::default(),
            None,
        );
        named_functions.insert(crate::NamedFunctionID::from(1), snf1);

        let snf2 = make_sampled_named_function(
            2,
            vec![(vec![0], 2.0)],
            Some("beta"),
            vec![],
            Default::default(),
            None,
        );
        named_functions.insert(crate::NamedFunctionID::from(2), snf2);

        let mut objectives = crate::Sampled::default();
        objectives.append([SampleID::from(0)], 0.0).unwrap();

        let sample_set = SampleSet::builder()
            .decision_variables(BTreeMap::new())
            .objectives(objectives)
            .constraints(BTreeMap::new())
            .named_functions(named_functions)
            .sense(Sense::Minimize)
            .build()
            .unwrap();

        let names = sample_set.named_function_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains("alpha"));
        assert!(names.contains("beta"));
    }
}
