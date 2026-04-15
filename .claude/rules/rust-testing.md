---
paths:
  - "rust/**/*.rs"
---

# Rust SDK Testing Guidelines

- Use `assert_abs_diff_eq!` to compare entire polynomials instead of checking individual terms with `get`
- Include clear comments in test cases explaining the intent and expected behavior

## Test Design
- Document what each test is checking with clear test names and comments
- Avoid redundant tests — check for overlapping test coverage
- Consider using helper functions to reduce duplication
- Group related assertions together

## Test Redundancy Prevention
- Before adding a new test, review existing tests to ensure it provides unique value
- If multiple tests share similar setup code, extract it into helper functions
- Consolidate tests that verify the same behavior with different inputs into parameterized tests where appropriate
- Each test should have a single clear purpose
