use super::*;
use std::ffi::CStr;

fn check(test: impl FnOnce(Python<'_>, &Bound<'_, PyDict>) -> PyResult<()>) {
    Python::initialize();
    Python::attach(|py| {
        let scope = PyDict::new(py);
        py.run(
            c"import pandas as pd\nimport numpy as np",
            Some(&scope),
            None,
        )
        .unwrap();
        if let Err(error) = test(py, &scope) {
            error.print(py);
            panic!("DataFrame schema assertion failed");
        }
    });
}

fn rows<'py>(
    py: Python<'py>,
    scope: &Bound<'py, PyDict>,
    code: &CStr,
) -> PyResult<Vec<Bound<'py, PyDict>>> {
    py.eval(code, Some(scope), None)?.extract()
}

#[test]
fn required_scalars_and_empty_operations() {
    check(|py, scope| {
        let columns = [
            Column::of::<u64>("id"),
            Column::of::<f64>("objective"),
            Column::of::<bool>("feasible"),
            Column::of::<i64>("signed"),
        ];
        let schema = DataFrameSchema::new(&columns, Some("id".into()));
        let entries = rows(py, scope, c"[{'feasible': True, 'id': 7, 'objective': 1.5, 'signed': -(2**63)}, {'id': 2**64-1, 'objective': float('inf'), 'feasible': False, 'signed': 2**63-1}]")?;
        scope.set_item("frame", schema.build(py, &entries)?)?;
        scope.set_item("empty", schema.build(py, &[])?)?;
        py.run(
            c"
expected = pd.DataFrame({
    'objective': pd.Series([1.5, float('inf')], dtype='float64'),
    'feasible': pd.Series([True, False], dtype='bool'),
    'signed': pd.Series([-(2**63), 2**63-1], dtype='int64'),
})
expected.columns = pd.Index(['objective', 'feasible', 'signed'], dtype=object)
expected.index = pd.Index([7, 2**64-1], dtype='uint64', name='id')
pd.testing.assert_frame_equal(frame, expected)
pd.testing.assert_frame_equal(empty, expected.iloc[:0])
for left, right, expected in [(empty, frame, frame), (frame, empty, frame), (empty, empty, empty)]:
    pd.testing.assert_frame_equal(pd.concat([left, right]), expected)
pd.testing.assert_frame_equal(frame[frame.feasible], frame.iloc[:1])
pd.testing.assert_frame_equal(empty[empty.feasible], empty)
",
            Some(scope),
            None,
        )
    });
}

#[test]
fn nullable_scalars_preserve_large_integers_and_present_nan() {
    check(|py, scope| {
        let columns = [
            Column::of::<Option<u64>>("unsigned"),
            Column::of::<Option<i64>>("signed"),
            Column::of::<Option<f64>>("value"),
            Column::of::<Option<bool>>("flag"),
        ];
        let schema = DataFrameSchema::new(&columns, None);
        let entries = rows(py, scope, c"[{'unsigned': 2**64-1, 'signed': -(2**63), 'value': float('nan'), 'flag': True}, {'unsigned': 2**63+1, 'signed': 2**63-1, 'value': float('inf'), 'flag': False}, {}, {'unsigned': None, 'signed': pd.NA, 'value': None, 'flag': None}]")?;
        scope.set_item("frame", schema.build(py, &entries)?)?;
        scope.set_item("empty", schema.build(py, &[])?)?;
        scope.set_item("missing", schema.build(py, &rows(py, scope, c"[{}, {}]")?)?)?;
        py.run(c"
assert frame.dtypes.tolist() == [pd.UInt64Dtype(), pd.Int64Dtype(), pd.Float64Dtype(), pd.BooleanDtype()]
assert frame.unsigned.iloc[:2].tolist() == [2**64-1, 2**63+1]
assert frame.signed.iloc[:2].tolist() == [-(2**63), 2**63-1]
assert frame.flag.iloc[:2].tolist() == [True, False]
assert np.isnan(frame.value.iloc[0]) and frame.value.iloc[0] is not pd.NA
assert frame.value.iloc[1] == float('inf')
assert frame.value.isna().tolist() == [False, False, True, True]
for column in frame:
    assert frame[column].iloc[2] is pd.NA
    assert frame[column].iloc[3] is pd.NA
pd.testing.assert_frame_equal(empty, frame.iloc[:0])
pd.testing.assert_frame_equal(empty, missing.iloc[:0])
pd.testing.assert_frame_equal(pd.concat([empty, frame]), frame)
pd.testing.assert_frame_equal(pd.concat([frame, empty]), frame)
", Some(scope), None)
    });
}

#[test]
fn strings_ignore_inference_and_storage_defaults() {
    check(|py, scope| {
        // The pyarrow storage preference must not become an implicit dependency.
        py.run(c"previous_storage = pd.options.mode.string_storage\npd.options.mode.string_storage = 'pyarrow'", Some(scope), None)?;
        let result = (|| {
            for columns in [
                [Column::of::<String>("name")],
                [Column::of::<str>("name")],
                [Column::of::<Option<String>>("name")],
            ] {
                let schema = DataFrameSchema::new(&columns, None);
                for code in [
                    c"[]" as &CStr,
                    c"[{}, {'name': None}, {'name': pd.NA}]",
                    c"[{'name': 'x'}, {'name': None}]",
                    c"[{'name': 'x'}, {'name': '日本語'}]",
                ] {
                    let entries = rows(py, scope, code)?;
                    scope.set_item("frame", schema.build(py, &entries)?)?;
                    scope.set_item("rows", entries)?;
                    py.run(
                        c"
expected_dtype = pd.StringDtype(storage='python')
assert frame.name.dtype == expected_dtype
assert frame.name.dtype.storage == 'python'
assert frame.name.dtype.na_value is pd.NA
for actual, row in zip(frame.name, rows):
    value = row.get('name')
    if value is None or value is pd.NA:
        assert actual is pd.NA
    else:
        assert actual == value
",
                        Some(scope),
                        None,
                    )?;
                }
            }
            Ok(())
        })();
        py.run(
            c"pd.options.mode.string_storage = previous_storage",
            Some(scope),
            None,
        )?;
        result
    });
}

#[test]
fn object_identity_and_declared_dynamic_column_order() {
    check(|py, scope| {
        let id = super::super::typed_constraint_id(7, "regular_constraint_id")?;
        scope.set_item("constraint_id", id)?;
        let key = String::from("parameters.a");
        let columns = [
            Column::of::<Py<PyAny>>("id"),
            Column::of::<String>(key.as_str()),
            Column::of::<f64>(9u64),
            Column::of::<Py<pyo3::types::PySet>>("members"),
        ];
        let schema = DataFrameSchema::new(&columns, Some("id".into()));
        let entries = rows(
            py,
            scope,
            c"[{'members': {1, 2}, 9: 2.0, 'id': constraint_id, 'parameters.a': 'v'}]",
        )?;
        scope.set_item("frame", schema.build(py, &entries)?)?;
        scope.set_item("empty", schema.build(py, &[])?)?;
        py.run(
            c"
assert frame.index.dtype == np.dtype('object')
assert frame.index[0] is constraint_id
assert frame.columns.tolist() == ['parameters.a', 9, 'members']
assert frame.members.iloc[0] == {1, 2}
assert frame.members.dtype == np.dtype('object')
pd.testing.assert_frame_equal(empty, frame.iloc[:0])
assert 9 in empty.columns
",
            Some(scope),
            None,
        )
    });
}

#[test]
fn malformed_schemas_and_undeclared_data_are_rejected() {
    check(|py, scope| {
        let duplicate = [Column::of::<Py<PyAny>>("x"), Column::of::<Py<PyAny>>("x")];
        assert!(DataFrameSchema::new(&duplicate, None)
            .build(py, &[])
            .unwrap_err()
            .is_instance_of::<PyValueError>(py));
        let columns = [Column::of::<u64>("x")];
        assert!(DataFrameSchema::new(&columns, Some("missing".into()))
            .build(py, &[])
            .unwrap_err()
            .is_instance_of::<PyValueError>(py));
        let schema = DataFrameSchema::new(&columns, None);
        assert!(schema
            .build(py, &rows(py, scope, c"[{'x': 1, 'unexpected': 2}]")?)
            .unwrap_err()
            .is_instance_of::<PyValueError>(py));
        assert!(schema.build(py, &rows(py, scope, c"[{}]")?).is_err());
        let empty_schema = DataFrameSchema::new(&[], None);
        let frame = empty_schema.build(py, &rows(py, scope, c"[{}, {}]")?)?;
        assert_eq!(
            frame
                .as_any()
                .getattr("shape")?
                .extract::<(usize, usize)>()?,
            (2, 0)
        );
        Ok(())
    });
}
