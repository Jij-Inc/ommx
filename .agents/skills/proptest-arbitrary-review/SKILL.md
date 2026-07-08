---
name: proptest-arbitrary-review
description: Use when reviewing changes to Arbitrary implementations, proptest strategy parameters, or property-test generators in the OMMX Rust SDK, including changes to what a default strategy generates.
---

# Proptest Arbitrary Review

Use this skill when a diff touches an `Arbitrary` implementation, a
`*Parameters` strategy type, or the space a generator samples from. The core
question is always: **which domain does each property test now quantify
over, and is that the domain the property actually claims?**

This complements `domain-responsibility-review`: that skill judges whether
production code preserves invariants; this skill judges whether the test
generators exercise those invariants honestly.

## Default-Space Blast Radius

Changing what `Default` parameters or `any::<T>()` generate silently changes
the quantification domain of every property test that uses the default.

- Enumerate all call sites before judging the diff:
  `grep -rn "T::arbitrary\|any::<T>" rust/` (and the parameter type name).
  Include non-test consumers: `random_deterministic`, examples, doc examples,
  and Python bindings that expose random generation.
- Classify each site explicitly:
  - The property should hold on the new, broader space → keep the default.
    A site kept on the default is a claim that the property holds for the
    whole space; treat an unchanged site as a positive assertion, not as
    "untouched code".
  - The property has a precondition (format compatibility, algebraic
    restriction) → it must opt into a named subspace constructor.
- Verify by execution, not by reading: check out the branch and run every
  affected proptest suite. Passing N generated cases is the only proof that
  the new default satisfies downstream expectations.

## Named Subspaces Express Preconditions

- Prefer intent-named constructors (`v1_compatible()`, `mps_compatible_qcqp()`)
  over ad-hoc field overrides at test sites. The test site should read as a
  statement of the property's precondition.
- A named subspace constructor is a semantic claim. Require a test that fails
  if the claim breaks (e.g. `v1_compatible` must be pinned by a v1 round-trip
  proptest). An alias like `v1_compatible() = regular_only()` is acceptable
  only while such a pinning test exists, because the two spaces can drift
  apart when either evolves.

## Coverage Honesty

Distinguish **sampled dimensions** from **constant injected structure**.

- Structure that a generator injects deterministically (fixed IDs, fixed
  function bodies, fixed set sizes, constant metadata) covers each new
  dimension at exactly one point. That is smoke coverage, not domain
  coverage. Either sample the dimension from a strategy, or document the
  limitation in the generator's rustdoc so properties over the default space
  do not overclaim.
- Check presence/absence combinations. A generator in which every optional
  feature is always present never tests feature-absent interactions;
  "always everything" is as narrow as "always nothing" in that dimension.
- Injected constant structure never shrinks away, so minimal counterexamples
  carry it as noise. Weigh that cost when deciding whether the structure
  belongs in the default space or in a dedicated named space.
- Non-emptiness assertions over deterministically injected structure verify
  the generator, not the domain. Such tests are generator-regression tests;
  name and comment them as such, and do not count them as evidence that a
  domain property holds.

## Generated Data Validity

- Generators must construct values through the owning validated constructor
  (e.g. `Instance::builder()`), so that every generated case re-checks the
  type's own invariants. Direct field assignment is acceptable only for
  fields whose documentation explicitly states they are validation-free.
- Hand-rolled resource allocation inside a generator (fresh variable IDs,
  next constraint ID) must match the domain's own allocator semantics.
  Compare against the owner's allocation function (e.g.
  `ConstraintCollection::unused_id` considers active **and** removed rows);
  prefer calling the domain allocator over re-implementing it.
- When the generator mutates the value after `build()` (conversions, direct
  field writes), re-check which invariants that post-processing can bypass.

## Review Checklist

- Which property tests changed quantification domain because of this diff,
  including the ones the diff does not touch?
- Does every site left on the default strategy genuinely claim its property
  over the broader space?
- Does every named subspace have a pinning test that fails when its claim
  breaks?
- Which generated dimensions are sampled and which are constant? Is constant
  structure documented as smoke coverage?
- Does generation go through the owning constructor, and does any
  post-`build()` mutation bypass validation?
- Were the affected proptest suites actually executed on the branch?
