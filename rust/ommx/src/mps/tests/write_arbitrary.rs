use super::super::*;
use crate::Instance;
use approx::AbsDiffEq;
use proptest::prelude::*;
use similar::{ChangeTag, TextDiff};

fn take_diff(expected: &Instance, actual: &Instance) -> String {
    let expected = format!("{expected:#?}");
    let actual = format!("{actual:#?}");
    let diff = TextDiff::from_lines(&expected, &actual);
    let mut diff_string = String::new();
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        diff_string += &format!("{sign}{change}");
    }
    diff_string
}

proptest! {
    #[test]
    fn test_write_mps(instance in Instance::arbitrary_with(<Instance as Arbitrary>::Parameters::default_qcqp())) {
        let mut buffer = Vec::new();
        prop_assert!(format::format(&instance, &mut buffer).is_ok())
    }

    #[test]
    fn test_roundtrip(instance in Instance::arbitrary_with(<Instance as Arbitrary>::Parameters::default_qcqp())) {
        let mut buffer = Vec::new();
        prop_assert!(format::format(&instance, &mut buffer).is_ok());
        let loaded = parse(&buffer[..]).unwrap();
        prop_assert!(
            instance.abs_diff_eq(&loaded, crate::ATol::default()),
            "Instance not matching after roundtrip:\n{}\nMPS:\n{}",
            take_diff(&instance, &loaded),
            String::from_utf8(buffer).unwrap()
        );
    }
}
