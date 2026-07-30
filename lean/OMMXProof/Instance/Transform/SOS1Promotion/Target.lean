import OMMXProof.Instance.Extend
import OMMXProof.Instance.Transform.SOS1BigM.Formulation
import OMMXProof.Instance.Transform.SOS1Promotion.Witness
import Mathlib.Tactic

/-!
# Target and sufficient validation for SOS1 promotion

This module constructs the promoted SOS1 Instance from a flat source Instance
and an untrusted `SOS1Promotion.Witness`.  The initial accepted shape is
deliberately conservative:

- retained variables and regular constraints form left/prefix blocks,
- fresh binary selectors and selector-formulation constraints form right/suffix
  blocks,
- every selected member has finite declared bounds,
- the suffix is the standard upper/lower Big-M links followed by one
  cardinality constraint,
- retained affine expressions do not depend on fresh selectors, and
- the flat source contains no pre-existing special constraints.

These are sufficient, not necessary, conditions.  Rejection by this initial
checker does not imply that no correct SOS1 promotion exists.
-/

namespace OMMXProof

namespace Instance

namespace SOS1Promotion

open SOS1BigM

namespace Witness

section Source

variable (witness : Witness n)
variable (source : Instance (n + witness.freshCount))

/-- Restrict an affine expression to the retained left block. -/
def prefixAffine (expr : Affine (n + witness.freshCount)) : Affine n where
  coeff := fun i => expr.coeff (Fin.castAdd witness.freshCount i)
  constant := expr.constant

/-- Restrict a regular constraint to the retained left block. -/
def prefixConstraint
    (constraint : LinearConstraint (n + witness.freshCount)) :
    LinearConstraint n where
  expr := witness.prefixAffine constraint.expr
  sense := constraint.sense

/-- An affine expression is independent of every fresh selector component. -/
def FreshIndependent
    (expr : Affine (n + witness.freshCount)) : Prop :=
  ∀ j, expr.coeff (Fin.natAdd n j) = 0

instance (expr : Affine (n + witness.freshCount)) :
    Decidable (witness.FreshIndependent expr) := by
  unfold FreshIndependent
  infer_instance

@[simp]
theorem prefixAffine_extend
    (expr : Affine n) :
    witness.prefixAffine (expr.extend witness.freshCount) = expr := by
  cases expr
  simp [prefixAffine, Affine.extend]

theorem extend_prefixAffine_of_freshIndependent
    {expr : Affine (n + witness.freshCount)}
    (h : witness.FreshIndependent expr) :
    (witness.prefixAffine expr).extend witness.freshCount = expr := by
  cases expr with
  | mk coeff constant =>
      apply Affine.mk.injEq.mpr
      constructor
      · funext i
        refine Fin.addCases ?_ ?_ i
        · intro j
          simp [prefixAffine, Affine.extend]
        · intro j
          simpa [FreshIndependent] using h j
      · rfl

@[simp]
theorem prefixConstraint_extend
    (constraint : LinearConstraint n) :
    witness.prefixConstraint (constraint.extend witness.freshCount) =
      constraint := by
  cases constraint
  simp [prefixConstraint]

theorem extend_prefixConstraint_of_freshIndependent
    {constraint : LinearConstraint (n + witness.freshCount)}
    (h : witness.FreshIndependent constraint.expr) :
    (witness.prefixConstraint constraint).extend witness.freshCount =
      constraint := by
  cases constraint with
  | mk expr sense =>
      simp only [prefixConstraint, LinearConstraint.extend]
      rw [witness.extend_prefixAffine_of_freshIndependent h]

@[simp]
theorem eval_prefixAffine_append_of_freshIndependent
    {expr : Affine (n + witness.freshCount)}
    (h : witness.FreshIndependent expr)
    (state : State n) (selectors : State witness.freshCount) :
    expr.eval (State.append state selectors) =
      (witness.prefixAffine expr).eval state := by
  simp [Affine.eval, prefixAffine, State.append, Fin.sum_univ_add, h]

theorem holds_prefixConstraint_append_of_freshIndependent
    {constraint : LinearConstraint (n + witness.freshCount)}
    (h : witness.FreshIndependent constraint.expr)
    (state : State n) (selectors : State witness.freshCount) :
    constraint.Holds (State.append state selectors) ↔
      (witness.prefixConstraint constraint).Holds state := by
  cases constraint with
  | mk expr sense =>
      cases sense <;>
        simp [LinearConstraint.Holds, prefixConstraint,
          witness.eval_prefixAffine_append_of_freshIndependent h]

/-- The first-class SOS1 constraint created by this promotion. -/
def promotedConstraint : SOS1Constraint n where
  members := witness.members

/-- The promoted target Instance determined unconditionally by the witness.

Invalid witnesses still determine a target.  Correctness properties are proved
only from `StandardBigMForm`. -/
def target : Instance n where
  domains := fun i => source.domains (Fin.castAdd witness.freshCount i)
  constraints :=
    (source.constraints.take witness.retainedConstraintCount).map
      witness.prefixConstraint
  oneHotConstraints := []
  sos1Constraints := [witness.promotedConstraint]
  indicatorConstraints := []
  objective := witness.prefixAffine source.objective
  sense := source.sense

def memberState (state : State n) : witness.Member → Rat :=
  fun i => state i

/-- A virtual selector tuple indexed by every promoted SOS1 member. -/
def freshSelectorState (members : State n)
    (fresh : State witness.freshCount) :
    witness.Member → Rat :=
  fun i =>
    if hi : i ∈ witness.freshMembers then
      fresh (witness.freshIndex i hi)
    else
      canonicalSelector (witness.memberState members) i

/-- Finite declared bounds for every promoted SOS1 member. -/
def FiniteMemberBounds : Prop :=
  ∀ i : witness.Member, ((witness.target source).domains i).bound.IsFinite

instance : Decidable (witness.FiniteMemberBounds source) := by
  unfold FiniteMemberBounds
  infer_instance

/-- Bounds extracted from the retained member domains.

The fallback zero is used only to keep construction total for untrusted
witnesses. `StandardBigMForm.finiteMemberBounds` proves that the fallback is
unreachable in every accepted promotion. -/
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

theorem withinSelectorBounds_of_domains
    (hfinite : witness.FiniteMemberBounds source)
    {state : State n}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i) :
    WithinSelectorBounds (witness.selectorBounds source)
      (witness.memberState state) := by
  intro i
  exact
    ⟨Domain.finite_lower_le (hdomains i)
        (witness.selectorBounds_exact source hfinite i).1,
      Domain.le_finite_upper (hdomains i)
        (witness.selectorBounds_exact source hfinite i).2⟩

/-- The witness classifies reused members exactly as retained binary members. -/
def ReusedMembersMatchDomains : Prop :=
  ∀ i : witness.Member,
    i ∈ witness.reusedMembers ↔ (witness.target source).domains i = .binary

instance : Decidable (witness.ReusedMembersMatchDomains source) := by
  unfold ReusedMembersMatchDomains
  infer_instance

theorem reusedBinary_of_domains
    (hlayout : witness.ReusedMembersMatchDomains source)
    {state : State n}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i) :
    GenericBinaryOn witness.reusedMembers (witness.memberState state) := by
  intro i hi
  have hdomain : (witness.target source).domains i = .binary :=
    (hlayout i).mp hi
  simpa [memberState, hdomain] using hdomains i

theorem genericSOS1_memberState_iff_holds (state : State n) :
    GenericSOS1 (witness.memberState state) ↔
      witness.promotedConstraint.Holds state := by
  classical
  rw [GenericSOS1, Finset.card_le_one]
  simp only [genericSupport, Finset.mem_filter, Finset.mem_univ, true_and]
  constructor
  · intro h i hi j hj hine hjne
    have hij :
        (⟨i, hi⟩ : witness.Member) = (⟨j, hj⟩ : witness.Member) := by
      apply h
      · simpa [memberState] using hine
      · simpa [memberState] using hjne
    exact congrArg Subtype.val hij
  · intro h i hi j hj
    apply Subtype.ext
    apply h i.val i.property j.val j.property
    · simpa [memberState] using hi
    · simpa [memberState] using hj

end Source

end Witness

end SOS1Promotion

end Instance

end OMMXProof
