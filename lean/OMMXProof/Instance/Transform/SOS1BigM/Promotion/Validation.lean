import OMMXProof.Instance.Transform.SOS1BigM.CanonicalRows
import OMMXProof.Instance.Transform.SOS1BigM.Promotion.Target
import Mathlib.Tactic

/-!
# Sufficient validation for SOS1 Big-M promotion

This module defines the conservative, decidable sufficient conditions accepted
by SOS1 Big-M promotion and derives the structural equalities available from a
validated witness. The initial accepted shape is deliberately conservative:

- retained variables and regular constraints form left/prefix blocks,
- fresh binary selectors and selector-formulation constraints form right/suffix
  blocks,
- every selected member has finite declared bounds,
- the suffix is the standard upper/lower Big-M links followed by one
  cardinality constraint,
- retained affine expressions do not depend on fresh selectors, and
- every pre-existing special constraint is independent of fresh selectors and
  can therefore be preserved in the promoted Instance.

These are sufficient, not necessary, conditions. Rejection by this initial
checker does not imply that no correct promotion exists through a broader
Big-M recognizer or another SOS1 formulation.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

namespace Witness

section Source

variable (witness : Witness n)
variable (source : Instance (n + witness.freshCount))

/-! ## Source-dependent inputs to the canonical row specification -/

/-- Finite declared bounds for every promoted SOS1 member. -/
def FiniteMemberBounds : Prop :=
  ∀ i : witness.Member, ((witness.target source).domains i).bound.IsFinite

instance : Decidable (witness.FiniteMemberBounds source) := by
  unfold FiniteMemberBounds
  infer_instance

/-- Bounds extracted from the retained member domains.

The fallback zero keeps the expected-row specification total for untrusted
witnesses. `StandardBigMForm.finiteMemberBounds` proves that it is unreachable
for every accepted promotion. -/
def selectorBounds : SelectorBounds witness.Member where
  lower := fun i =>
    if h : ((witness.target source).domains i).bound.IsFinite then
      ((witness.target source).domains i).bound.finiteLower h
    else 0
  upper := fun i =>
    if h : ((witness.target source).domains i).bound.IsFinite then
      ((witness.target source).domains i).bound.finiteUpper h
    else 0

theorem selectorBounds_exact
    (hfinite : witness.FiniteMemberBounds source)
    (i : witness.Member) :
    ((witness.target source).domains i).bound.lower =
        .finite ((witness.selectorBounds source).lower i) ∧
      ((witness.target source).domains i).bound.upper =
        .finite ((witness.selectorBounds source).upper i) := by
  have hi := hfinite i
  simp only [selectorBounds, hi, ↓reduceDIte]
  exact ⟨Bound.lower_eq_finiteLower _ hi,
    Bound.upper_eq_finiteUpper _ hi⟩

/-- The witness classifies reused members exactly as retained binary members. -/
def ReusedMembersMatchDomains : Prop :=
  ∀ i : witness.Member,
    i ∈ witness.reusedMembers ↔ (witness.target source).domains i = .binary

instance : Decidable (witness.ReusedMembersMatchDomains source) := by
  unfold ReusedMembersMatchDomains
  infer_instance

/-! ## Conservative validation of the flat source shape -/

/-- Pointwise, computable equality for regular constraint rows.

`LinearConstraint` does not have a global `DecidableEq` instance because its
coefficient vector is a function.  Promotion only needs equality over the
finite current dimension, so validation compares the sense, constant, and
every coefficient explicitly. -/
def SameRow (lhs rhs : LinearConstraint m) : Prop :=
  lhs.sense = rhs.sense ∧
    lhs.expr.constant = rhs.expr.constant ∧
    ∀ i, lhs.expr.coeff i = rhs.expr.coeff i

instance (lhs rhs : LinearConstraint m) : Decidable (SameRow lhs rhs) := by
  unfold SameRow
  infer_instance

theorem sameRow_iff_eq {lhs rhs : LinearConstraint m} :
    SameRow lhs rhs ↔ lhs = rhs := by
  constructor
  · rcases lhs with ⟨⟨lhsCoeff, lhsConstant⟩, lhsSense⟩
    rcases rhs with ⟨⟨rhsCoeff, rhsConstant⟩, rhsSense⟩
    rintro ⟨hsense, hconstant, hcoeff⟩
    simp only at hsense hconstant hcoeff
    subst rhsSense
    subst rhsConstant
    have hcoefficients : lhsCoeff = rhsCoeff := funext hcoeff
    subst rhsCoeff
    rfl
  · rintro rfl
    exact ⟨rfl, rfl, fun _ => rfl⟩

/-- A computable row-by-row comparison for two ordered constraint blocks. -/
def RowsMatch (actual expected : List (LinearConstraint m)) : Prop :=
  List.Forall₂ SameRow actual expected

instance (actual expected : List (LinearConstraint m)) :
    Decidable (RowsMatch actual expected) := by
  unfold RowsMatch
  infer_instance

theorem rowsMatch_iff_eq {actual expected : List (LinearConstraint m)} :
    RowsMatch actual expected ↔ actual = expected := by
  constructor
  · intro h
    induction h with
    | nil => rfl
    | cons hrow _ ih =>
        rw [sameRow_iff_eq] at hrow
        simp [hrow, ih]
  · rintro rfl
    induction actual with
    | nil => exact .nil
    | cons head tail ih =>
        exact .cons (sameRow_iff_eq.mpr rfl) ih

/-- Initial sufficient conditions recognized by SOS1 Big-M promotion.

The condition deliberately fixes a prefix/suffix layout.  It is not intended
to characterize every flat formulation equivalent to an SOS1 constraint.
Failure therefore means "unsupported by this validator", not that the proposed
promotion is mathematically invalid. -/
structure StandardBigMForm : Prop where
  finiteMemberBounds : witness.FiniteMemberBounds source
  reusedMembersMatchDomains : witness.ReusedMembersMatchDomains source
  freshSelectorDomains :
    ∀ j, source.domains (Fin.natAdd n j) = .binary
  retainedConstraintCount_le :
    witness.retainedConstraintCount ≤ source.constraints.length
  retainedConstraintsFreshIndependent :
    (source.constraints.take witness.retainedConstraintCount).Forall
      fun constraint => witness.FreshIndependent constraint.expr
  formulationSuffixMatches :
    RowsMatch
      (source.constraints.drop witness.retainedConstraintCount)
      (witness.selectorLayout.canonicalRows (witness.selectorBounds source))
  oneHotConstraintsFreshIndependent :
    source.oneHotConstraints.Forall witness.OneHotFreshIndependent
  sos1ConstraintsFreshIndependent :
    source.sos1Constraints.Forall witness.SOS1FreshIndependent
  indicatorConstraintsFreshIndependent :
    source.indicatorConstraints.Forall witness.IndicatorFreshIndependent
  objectiveFreshIndependent :
    witness.FreshIndependent source.objective

private def standardBigMFormConditions : Prop :=
  witness.FiniteMemberBounds source ∧
    witness.ReusedMembersMatchDomains source ∧
    (∀ j, source.domains (Fin.natAdd n j) = .binary) ∧
    witness.retainedConstraintCount ≤ source.constraints.length ∧
    (source.constraints.take witness.retainedConstraintCount).Forall
      (fun constraint => witness.FreshIndependent constraint.expr) ∧
    RowsMatch
      (source.constraints.drop witness.retainedConstraintCount)
      (witness.selectorLayout.canonicalRows (witness.selectorBounds source)) ∧
    source.oneHotConstraints.Forall witness.OneHotFreshIndependent ∧
    source.sos1Constraints.Forall witness.SOS1FreshIndependent ∧
    source.indicatorConstraints.Forall witness.IndicatorFreshIndependent ∧
    witness.FreshIndependent source.objective

private instance : Decidable (witness.standardBigMFormConditions source) := by
  unfold standardBigMFormConditions
  infer_instance

private theorem standardBigMForm_iff_conditions :
    witness.StandardBigMForm source ↔
      witness.standardBigMFormConditions source := by
  constructor
  · intro h
    exact ⟨h.finiteMemberBounds, h.reusedMembersMatchDomains,
      h.freshSelectorDomains, h.retainedConstraintCount_le,
      h.retainedConstraintsFreshIndependent, h.formulationSuffixMatches,
      h.oneHotConstraintsFreshIndependent, h.sos1ConstraintsFreshIndependent,
      h.indicatorConstraintsFreshIndependent, h.objectiveFreshIndependent⟩
  · rintro ⟨hfinite, hlayout, hfreshDomains, hcount, hretained,
      hsuffix, honeHot, hsos1, hindicator, hobjective⟩
    exact
      { finiteMemberBounds := hfinite
        reusedMembersMatchDomains := hlayout
        freshSelectorDomains := hfreshDomains
        retainedConstraintCount_le := hcount
        retainedConstraintsFreshIndependent := hretained
        formulationSuffixMatches := hsuffix
        oneHotConstraintsFreshIndependent := honeHot
        sos1ConstraintsFreshIndependent := hsos1
        indicatorConstraintsFreshIndependent := hindicator
        objectiveFreshIndependent := hobjective }

instance : Decidable (witness.StandardBigMForm source) :=
  decidable_of_iff (witness.standardBigMFormConditions source)
    (witness.standardBigMForm_iff_conditions source).symm

/-- A witness bundled with evidence that the current flat source has the
initial supported selector-formulation shape. -/
structure Validated : Type where
  standardBigMForm : witness.StandardBigMForm source

/-- Check the current flat Instance against the conservative supported shape. -/
def validate : Option (witness.Validated source) :=
  if hvalid : witness.StandardBigMForm source then
    some ⟨hvalid⟩
  else
    none

@[simp]
theorem validate_isSome_iff_standardBigMForm :
    (witness.validate source).isSome ↔
      witness.StandardBigMForm source := by
  simp [validate]

@[simp]
theorem validate_eq_none_iff_not_standardBigMForm :
    witness.validate source = none ↔
      ¬witness.StandardBigMForm source := by
  simp [validate]

namespace Validated

theorem extendedPrefixConstraints_eq
    (validated : witness.Validated source) :
    ((source.constraints.take witness.retainedConstraintCount).map
        witness.prefixConstraint).map
          (fun constraint => constraint.extend witness.freshCount) =
      source.constraints.take witness.retainedConstraintCount := by
  rw [List.map_map]
  calc
    List.map
        ((fun constraint => constraint.extend witness.freshCount) ∘
          witness.prefixConstraint)
        (source.constraints.take witness.retainedConstraintCount) =
      List.map id
        (source.constraints.take witness.retainedConstraintCount) := by
      rw [List.map_eq_map_iff]
      intro constraint hconstraint
      apply witness.extend_prefixConstraint_of_freshIndependent
      exact
        (List.forall_iff_forall_mem.mp
          validated.standardBigMForm.retainedConstraintsFreshIndependent)
          constraint hconstraint
    _ = source.constraints.take witness.retainedConstraintCount :=
      List.map_id _

theorem formulationSuffix_eq
    (validated : witness.Validated source) :
    source.constraints.drop witness.retainedConstraintCount =
      witness.selectorLayout.canonicalRows (witness.selectorBounds source) := by
  exact rowsMatch_iff_eq.mp
    validated.standardBigMForm.formulationSuffixMatches

theorem sourceDomains_eq
    (validated : witness.Validated source) :
    source.domains =
      Domain.append (witness.target source).domains (fun _ => .binary) := by
  funext i
  refine Fin.addCases ?_ ?_ i
  · intro j
    simp [target, Domain.append]
  · intro j
    simp [Domain.append,
      validated.standardBigMForm.freshSelectorDomains j]

theorem sourceConstraints_eq
    (validated : witness.Validated source) :
    source.constraints =
      ((witness.target source).constraints.map
        fun constraint => constraint.extend witness.freshCount) ++
        witness.selectorLayout.canonicalRows
          (witness.selectorBounds source) := by
  calc
    source.constraints =
        source.constraints.take witness.retainedConstraintCount ++
          source.constraints.drop witness.retainedConstraintCount :=
      (List.take_append_drop witness.retainedConstraintCount
        source.constraints).symm
    _ =
        source.constraints.take witness.retainedConstraintCount ++
          witness.selectorLayout.canonicalRows
            (witness.selectorBounds source) := by
      rw [validated.formulationSuffix_eq]
    _ =
        ((witness.target source).constraints.map
          fun constraint => constraint.extend witness.freshCount) ++
          witness.selectorLayout.canonicalRows
            (witness.selectorBounds source) := by
      rw [target, validated.extendedPrefixConstraints_eq]

theorem sourceOneHotConstraints_eq
    (validated : witness.Validated source) :
    source.oneHotConstraints =
      (witness.retainedOneHotConstraints source).map
        (fun constraint ↦ constraint.extend witness.freshCount) := by
  symm
  simpa [retainedOneHotConstraints] using
    witness.map_extend_filterMap_prefixOneHot
      validated.standardBigMForm.oneHotConstraintsFreshIndependent

theorem sourceSOS1Constraints_eq
    (validated : witness.Validated source) :
    source.sos1Constraints =
      (witness.retainedSOS1Constraints source).map
        (fun constraint ↦ constraint.extend witness.freshCount) := by
  symm
  simpa [retainedSOS1Constraints] using
    witness.map_extend_filterMap_prefixSOS1
      validated.standardBigMForm.sos1ConstraintsFreshIndependent

theorem sourceIndicatorConstraints_eq
    (validated : witness.Validated source) :
    source.indicatorConstraints =
      (witness.retainedIndicatorConstraints source).map
        (fun constraint ↦ constraint.extend witness.freshCount) := by
  symm
  simpa [retainedIndicatorConstraints] using
    witness.map_extend_filterMap_prefixIndicator
      validated.standardBigMForm.indicatorConstraintsFreshIndependent

theorem sourceObjective_eq
    (validated : witness.Validated source) :
    source.objective =
      (witness.target source).objective.extend witness.freshCount := by
  symm
  simpa [target] using
    witness.extend_prefixAffine_of_freshIndependent
      validated.standardBigMForm.objectiveFreshIndependent

end Validated

end Source

end Witness

end SOS1BigM

end Instance

end OMMXProof
