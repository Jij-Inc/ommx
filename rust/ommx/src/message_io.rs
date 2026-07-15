use crate::{Message, ParseError, RawParseError};

/// Decode one protobuf message while preserving the OMMX-owned parse signal.
pub fn decode<M>(bytes: &[u8], root: &'static str) -> Result<M, ParseError>
where
    M: Message + Default,
{
    M::decode(bytes).map_err(|error| RawParseError::from(error).context(root, "bytes"))
}

#[cfg(test)]
mod tests {
    use crate::{ParseError, RawParseError};
    use std::error::Error as _;

    #[test]
    fn sdk_byte_decoders_preserve_the_parse_error_signal() {
        let invalid_wire = [0x80];
        let cases: Vec<(&str, crate::Error)> = vec![
            (
                "ommx.v1.Function",
                crate::Function::from_bytes(&invalid_wire)
                    .err()
                    .expect("invalid Function wire data must fail"),
            ),
            (
                "ommx.v1.Linear",
                crate::Linear::from_bytes(&invalid_wire)
                    .err()
                    .expect("invalid Linear wire data must fail"),
            ),
            (
                "ommx.v1.Quadratic",
                crate::Quadratic::from_bytes(&invalid_wire)
                    .err()
                    .expect("invalid Quadratic wire data must fail"),
            ),
            (
                "ommx.v1.Polynomial",
                crate::Polynomial::from_bytes(&invalid_wire)
                    .err()
                    .expect("invalid Polynomial wire data must fail"),
            ),
            (
                "ommx.v1.Instance",
                crate::Instance::from_v1_bytes(&invalid_wire)
                    .err()
                    .expect("invalid v1 Instance wire data must fail"),
            ),
            (
                "ommx.v2.Instance",
                crate::Instance::from_v2_bytes(&invalid_wire)
                    .err()
                    .expect("invalid v2 Instance wire data must fail"),
            ),
            (
                "ommx.v1.ParametricInstance",
                crate::ParametricInstance::from_v1_bytes(&invalid_wire)
                    .err()
                    .expect("invalid v1 ParametricInstance wire data must fail"),
            ),
            (
                "ommx.v2.ParametricInstance",
                crate::ParametricInstance::from_v2_bytes(&invalid_wire)
                    .err()
                    .expect("invalid v2 ParametricInstance wire data must fail"),
            ),
            (
                "ommx.v1.Solution",
                crate::Solution::from_v1_bytes(&invalid_wire)
                    .err()
                    .expect("invalid v1 Solution wire data must fail"),
            ),
            (
                "ommx.v2.Solution",
                crate::Solution::from_v2_bytes(&invalid_wire)
                    .err()
                    .expect("invalid v2 Solution wire data must fail"),
            ),
            (
                "ommx.v1.SampleSet",
                crate::SampleSet::from_v1_bytes(&invalid_wire)
                    .err()
                    .expect("invalid v1 SampleSet wire data must fail"),
            ),
            (
                "ommx.v2.SampleSet",
                crate::SampleSet::from_v2_bytes(&invalid_wire)
                    .err()
                    .expect("invalid v2 SampleSet wire data must fail"),
            ),
        ];

        for (message, error) in cases {
            let parse_error = error
                .downcast_ref::<ParseError>()
                .expect("wire decode failures must remain downcastable as ParseError");
            assert!(matches!(&parse_error.error, RawParseError::DecodeError(_)));
            assert_eq!(parse_error.context.len(), 1);
            assert_eq!(parse_error.context[0].message, message);
            assert_eq!(parse_error.context[0].field, "bytes");

            let raw_source = parse_error
                .source()
                .expect("ParseError must expose its RawParseError source");
            assert!(raw_source.downcast_ref::<RawParseError>().is_some());
            let decode_source = raw_source
                .source()
                .expect("RawParseError::DecodeError must expose the protobuf source");
            assert!(decode_source.downcast_ref::<prost::DecodeError>().is_some());
        }
    }
}
