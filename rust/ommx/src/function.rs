use crate::{error::Error, v1::State};
use num::{One, Zero};
use std::fmt::Debug;

pub trait Function: Debug + Clone + Zero + One {
    fn degree(&self) -> usize;
    fn evaluate(&self, state: &State) -> Result<f64, Error>;
    fn partial_evaluate(&self, state: &State) -> Result<Self, Error>;
}

impl Function for f64 {
    fn degree(&self) -> usize {
        0
    }

    fn evaluate(&self, _state: &State) -> Result<f64, Error> {
        Ok(*self)
    }

    fn partial_evaluate(&self, _state: &State) -> Result<Self, Error> {
        Ok(*self)
    }
}
