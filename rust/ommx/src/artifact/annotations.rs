use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct InstanceAnnotations {}

impl From<InstanceAnnotations> for HashMap<String, String> {
    fn from(_: InstanceAnnotations) -> Self {
        HashMap::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolutionAnnotations {}

impl From<SolutionAnnotations> for HashMap<String, String> {
    fn from(_: SolutionAnnotations) -> Self {
        HashMap::new()
    }
}
