use crate::v1::{function, Function, Linear, Quadratic};

impl Into<Function> for function::Function {
    fn into(self) -> Function {
        Function {
            function: Some(self),
        }
    }
}

impl Into<Function> for Linear {
    fn into(self) -> Function {
        Function {
            function: Some(function::Function::Linear(self)),
        }
    }
}

impl Into<Function> for Quadratic {
    fn into(self) -> Function {
        Function {
            function: Some(function::Function::Quadratic(self)),
        }
    }
}
