---
paths:
  - "rust/**/*.rs"
---

# Rust Documentation Math (KaTeX)

When a rustdoc comment contains math, render it via [`katexit`](https://crates.io/crates/katexit) so it displays as KaTeX in the generated HTML.

## How to apply

Annotate the item (function, struct, impl block, etc.) whose doc comment contains math:

```rust
impl Foo {
    #[cfg_attr(doc, katexit::katexit)]
    /// Compute $f(x) = x^2 - x$ over $x \in [0, 1]$.
    ///
    /// The minimum is
    ///
    /// $$
    /// \min_{x \in [0, 1]} f(x) = -\tfrac{1}{4}.
    /// $$
    pub fn foo(&self) { ... }
}
```

- Inline math: `$...$`
- Display math: `$$...$$` on its own line(s) with a blank line before and after
- The attribute goes directly above the `///` doc comment, after other attributes like `#[pymethods]`-style ones (not applicable here) but before the doc comments themselves
- Gate it with `cfg_attr(doc, ...)` so it only applies during documentation builds — avoids pulling the proc-macro into normal compilation

## When to use

- Any new or edited rustdoc comment in `rust/ommx/` that contains math notation
- If a docstring mixes prose and math, still use KaTeX for every math fragment rather than plain ASCII

## Existing examples

- `rust/ommx/src/function/evaluate_bound.rs`
- `rust/ommx/src/instance/indicator.rs`
- `rust/ommx/src/instance/penalty.rs`
- `rust/ommx/src/instance/sos1.rs`
- `rust/ommx/src/instance/one_hot.rs`

The `katexit` dependency is already declared in `rust/ommx/Cargo.toml` (via workspace).
