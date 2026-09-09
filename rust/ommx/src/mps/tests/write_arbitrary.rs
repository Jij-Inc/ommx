use super::super::*;
use crate::{arbitrary::InstanceParameters, Instance};
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
    fn test_write_mps(instance in Instance::arbitrary_with(InstanceParameters::mps_compatible_qcqp())) {
        let mut buffer = Vec::new();
        prop_assert!(format::format(&instance, &mut buffer).is_ok())
    }

    #[test]
    fn test_roundtrip(instance in Instance::arbitrary_with(InstanceParameters::mps_compatible_qcqp())) {
        let mut buffer = Vec::new();
        prop_assert!(format::format(&instance, &mut buffer).is_ok());
        let mps = String::from_utf8(buffer).unwrap();
        let binary_names: std::collections::HashSet<_> = instance
            .decision_variables()
            .iter()
            .filter(|(_, variable)| {
                variable.kind() == crate::decision_variable::Kind::Binary
                    && variable.bound() == crate::Bound::of_binary()
            })
            .map(|(id, _)| format!("{}{}", format::VAR_PREFIX, id.into_inner()))
            .collect();
        let mut in_bounds = false;
        let implicit_binary_bounds = mps
            .lines()
            .filter(|line| {
                if !line.starts_with(' ') {
                    in_bounds = *line == "BOUNDS";
                }
                !in_bounds
                    || !line
                        .split_whitespace()
                        .nth(2)
                        .is_some_and(|name| binary_names.contains(name))
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Exercise both explicit bounds and equivalent integer-marker defaults.
        // Bounds fixing a binary variable to 0 or 1 must remain explicit.
        for input in [&mps, &implicit_binary_bounds] {
            let loaded = parse(input.as_bytes()).unwrap();
            prop_assert!(
                instance.abs_diff_eq(&loaded, crate::ATol::default()),
                "Instance not matching after roundtrip:\n{}\nMPS:\n{}",
                take_diff(&instance, &loaded),
                input
            );
        }
    }
}
