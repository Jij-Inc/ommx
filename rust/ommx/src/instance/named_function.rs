use fnv::FnvHashMap;

use crate::{Evaluate, Function, Instance, NamedFunction, NamedFunctionID, VariableIDSet};

impl Instance {
    /// Get all unique named function names in this instance
    ///
    /// Returns a set of all unique named function names that have at least one named function.
    /// Named functions without names are not included.
    pub fn named_function_names(&self) -> std::collections::BTreeSet<String> {
        self.named_functions
            .keys()
            .filter_map(|id| self.named_function_metadata().name(*id).map(str::to_owned))
            .collect()
    }

    /// Get a named function by name and subscripts
    ///
    /// # Arguments
    /// * `name` - The name of the named function to find
    /// * `subscripts` - The subscripts of the named function (can be empty)
    ///
    /// # Returns
    /// * `Some(&NamedFunction)` if a named function with the given name and subscripts is found
    /// * `None` if no matching named function is found
    ///
    /// # Example
    /// ```
    /// use ommx::Instance;
    ///
    /// let instance = Instance::default();
    /// // Find named function with name "x" and no subscripts
    /// let nf = instance.get_named_function_by_name("x", vec![]);
    /// // Find named function with name "y" and subscripts [1, 2]
    /// let nf_indexed = instance.get_named_function_by_name("y", vec![1, 2]);
    /// ```
    pub fn get_named_function_by_name(
        &self,
        name: &str,
        subscripts: Vec<i64>,
    ) -> Option<&NamedFunction> {
        self.named_functions.iter().find_map(|(id, nf)| {
            let store = self.named_function_metadata();
            if store.name(*id) == Some(name) && store.subscripts(*id) == subscripts.as_slice() {
                Some(nf)
            } else {
                None
            }
        })
    }

    /// Returns the next available NamedFunctionID.
    ///
    /// Finds the maximum ID from named functions, then adds 1.
    /// If there are no named functions, returns NamedFunctionID(0).
    ///
    /// Note: This method does not track which IDs have been allocated.
    /// Consecutive calls will return the same ID until a named function is actually added.
    pub fn next_named_function_id(&self) -> NamedFunctionID {
        self.named_functions
            .last_key_value()
            .map(|(id, _)| NamedFunctionID::from(id.into_inner() + 1))
            .unwrap_or(NamedFunctionID::from(0))
    }

    pub fn new_named_function(
        &mut self,
        function: Function,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: FnvHashMap<String, String>,
        description: Option<String>,
    ) -> crate::Result<&mut NamedFunction> {
        let variable_ids: VariableIDSet = self.decision_variables.keys().cloned().collect();

        for id in function.required_ids() {
            if !variable_ids.contains(&id) {
                crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
            }
        }
        let id = self.next_named_function_id();

        let named_function = NamedFunction { id, function };
        self.named_functions.insert(id, named_function);
        let metadata = crate::named_function::NamedFunctionMetadata {
            name,
            subscripts,
            parameters,
            description,
        };
        self.named_function_metadata_mut().insert(id, metadata);
        Ok(self.named_functions.get_mut(&id).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use crate::{coeff, linear, Coefficient, Function, Instance, NamedFunctionID};

    #[test]
    fn test_named_function_names() {
        // Instance with named and unnamed functions → names() returns only named, deduplicated
        let mut instance = Instance::default();

        // Add a decision variable so functions can reference it
        instance
            .new_named_function(
                Function::Constant(Coefficient::try_from(1.0).unwrap()),
                Some("alpha".to_string()),
                vec![0],
                Default::default(),
                None,
            )
            .unwrap();

        instance
            .new_named_function(
                Function::Constant(Coefficient::try_from(2.0).unwrap()),
                Some("alpha".to_string()),
                vec![1],
                Default::default(),
                None,
            )
            .unwrap();

        instance
            .new_named_function(
                Function::Constant(Coefficient::try_from(3.0).unwrap()),
                Some("beta".to_string()),
                vec![],
                Default::default(),
                None,
            )
            .unwrap();

        // Unnamed function
        instance
            .new_named_function(
                Function::Constant(Coefficient::try_from(4.0).unwrap()),
                None,
                vec![],
                Default::default(),
                None,
            )
            .unwrap();

        let names = instance.named_function_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains("alpha"));
        assert!(names.contains("beta"));
    }

    #[test]
    fn test_get_named_function_by_name() {
        let mut instance = Instance::default();

        instance
            .new_named_function(
                Function::Constant(Coefficient::try_from(1.0).unwrap()),
                Some("f".to_string()),
                vec![1, 2],
                Default::default(),
                None,
            )
            .unwrap();

        // Lookup by correct name and subscripts
        let found = instance.get_named_function_by_name("f", vec![1, 2]);
        assert!(found.is_some());
        let found_id = found.unwrap().id;
        assert_eq!(instance.named_function_metadata().name(found_id), Some("f"));
        assert_eq!(
            instance.named_function_metadata().subscripts(found_id),
            &[1, 2]
        );

        // Wrong subscripts → None
        let not_found = instance.get_named_function_by_name("f", vec![3]);
        assert!(not_found.is_none());

        // Wrong name → None
        let not_found = instance.get_named_function_by_name("g", vec![1, 2]);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_next_named_function_id_empty() {
        let instance = Instance::default();
        assert_eq!(instance.next_named_function_id(), NamedFunctionID::from(0));
    }

    #[test]
    fn test_next_named_function_id_sparse() {
        let mut instance = Instance::default();

        // Add function with ID 0 (first call)
        instance
            .new_named_function(
                Function::Constant(Coefficient::try_from(1.0).unwrap()),
                None,
                vec![],
                Default::default(),
                None,
            )
            .unwrap();

        // Add function with ID 1 (second call)
        instance
            .new_named_function(
                Function::Constant(Coefficient::try_from(2.0).unwrap()),
                None,
                vec![],
                Default::default(),
                None,
            )
            .unwrap();

        // Next ID should be 2
        assert_eq!(instance.next_named_function_id(), NamedFunctionID::from(2));
    }

    #[test]
    fn test_new_named_function_success() {
        let mut instance = Instance::default();

        // Add a decision variable that the function will reference (gets ID 0)
        instance.new_continuous();

        // Add a named function that references variable 0
        let nf = instance
            .new_named_function(
                Function::Linear(coeff!(2.0) * linear!(0)),
                Some("obj".to_string()),
                vec![0],
                Default::default(),
                Some("test function".to_string()),
            )
            .unwrap();

        let nf_id = nf.id;
        assert_eq!(nf_id, NamedFunctionID::from(0));
        let store = instance.named_function_metadata();
        assert_eq!(store.name(nf_id), Some("obj"));
        assert_eq!(store.subscripts(nf_id), &[0]);
        assert_eq!(store.description(nf_id), Some("test function"));
    }

    #[test]
    fn test_new_named_function_undefined_variable() {
        let mut instance = Instance::default();

        // Try to add a function referencing variable 99 which doesn't exist
        let err = instance
            .new_named_function(
                Function::Linear(coeff!(1.0) * linear!(99)),
                Some("bad".to_string()),
                vec![],
                Default::default(),
                None,
            )
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("Undefined variable ID") && msg.contains("99"),
            "unexpected error: {msg}"
        );
    }
}
