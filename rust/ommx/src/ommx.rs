/// A monomial in the polynomial.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Monomial {
    #[prost(uint64, repeated, tag = "1")]
    pub indices: ::prost::alloc::vec::Vec<u64>,
    #[prost(fixed64, tag = "2")]
    pub coefficient: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Polynomial {
    #[prost(message, repeated, tag = "1")]
    pub terms: ::prost::alloc::vec::Vec<Monomial>,
}
