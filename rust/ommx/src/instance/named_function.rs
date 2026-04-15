use fnv::FnvHashMap;

use crate::{
    Evaluate, Function, Instance, InstanceError, NamedFunction, NamedFunctionID, VariableIDSet,
};

impl Instance {
    /// Get all unique named function names in this instance
    ///
    /// Returns a set of all unique named function names that have at least one named function.
    /// Named functions without names are not included.
    pub fn named_function_names(&self) -> std::collections::BTreeSet<String> {
        self.named_functions
            .values()
            .filter_map(|var| var.name.clone())
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
        self.named_functions
            .values()
            .find(|var| var.name.as_deref() == Some(name) && var.subscripts == subscripts)
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
    ) -> Result<&mut NamedFunction, InstanceError> {
        let variable_ids: VariableIDSet = self.decision_variables.keys().cloned().collect();

        for id in function.required_ids() {
            if !variable_ids.contains(&id) {
                return Err(InstanceError::UndefinedVariableID { id });
            }
        }
        let id = self.next_named_function_id();

        let named_function = NamedFunction {
            id,
            function,
            name,
            subscripts,
            parameters,
            description,
        };
        self.named_functions.insert(id, named_function);
        Ok(self.named_functions.get_mut(&id).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use crate::{coeff, linear, Coefficient, Function, Instance, NamedFunctionID, VariableID};

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
        assert_eq!(found.unwrap().name, Some("f".to_string()));
        assert_eq!(found.unwrap().subscripts, vec![1, 2]);

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

        assert_eq!(nf.id, NamedFunctionID::from(0));
        assert_eq!(nf.name, Some("obj".to_string()));
        assert_eq!(nf.subscripts, vec![0]);
        assert_eq!(nf.description, Some("test function".to_string()));
    }

    #[test]
    fn test_new_named_function_undefined_variable() {
        use crate::InstanceError;

        let mut instance = Instance::default();

        // Try to add a function referencing variable 99 which doesn't exist
        let result = instance.new_named_function(
            Function::Linear(coeff!(1.0) * linear!(99)),
            Some("bad".to_string()),
            vec![],
            Default::default(),
            None,
        );

        assert!(matches!(
            result,
            Err(InstanceError::UndefinedVariableID { id }) if id == VariableID::from(99)
        ));
    }
}
