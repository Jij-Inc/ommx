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
    /// * `Some(&DecisionVariable)` if a named function with the given name and subscripts is found
    /// * `None` if no matching named function is found
    ///
    /// # Example
    /// ```
    /// use ommx::Instance;
    ///
    /// let instance = Instance::default();
    /// // Find named function with name "x" and no subscripts
    /// let var = instance.get_named_function_by_name("x", vec![]);
    /// // Find named function with name "y" and subscripts [1, 2]
    /// let var_indexed = instance.get_named_function_by_name("y", vec![1, 2]);
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
