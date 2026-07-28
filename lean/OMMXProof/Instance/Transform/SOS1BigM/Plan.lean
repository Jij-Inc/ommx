import OMMXProof.Instance
import OMMXProof.Instance.Transform.SOS1BigM.Formulation
import Mathlib.Tactic

/-!
# Plan data for SOS1 Big-M lowering

`Plan` identifies one source SOS1 constraint. Validation checks whether the
source domains provide every finite bound required by the lowering.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

/-- A plan for lowering one occurrence in the source SOS1 list. -/
structure Plan (source : Instance n) where
  constraintIndex : Fin source.sos1Constraints.length

namespace Plan

def constraint {source : Instance n} (plan : Plan source) : SOS1Constraint n :=
  source.sos1Constraints.get plan.constraintIndex

abbrev Member {source : Instance n} (plan : Plan source) :=
  {i // i ∈ plan.constraint.members}

def memberState {source : Instance n} (plan : Plan source)
    (state : State n) : plan.Member → Rat :=
  fun i => state i

def reusedMembers {source : Instance n} (plan : Plan source) :
    Finset plan.Member :=
  Finset.univ.filter fun i => source.domains i = .binary

def freshMembers {source : Instance n} (plan : Plan source) :
    Finset plan.Member :=
  plan.reusedMembersᶜ

/-- Exact validation performed before the target Instance is trusted.

The extracted selector bounds are rational, so validation requires both source
endpoints to be finite. The independent `Domain` syntax has no semi-continuous
or semi-integer cases, so no additional kind rejection is needed here. -/
def Valid {source : Instance n} (plan : Plan source) : Prop :=
  plan.constraint.members.Nonempty ∧
    (∀ i : plan.Member, (source.domains i).bound.IsFinite) ∧
    ∀ i : plan.Member, i ∉ plan.reusedMembers →
      (source.domains i).bound.lower ≤ .finite 0 ∧
        .finite 0 ≤ (source.domains i).bound.upper

instance {source : Instance n} (plan : Plan source) :
    Decidable plan.Valid := by
  unfold Valid
  infer_instance

/-- A plan together with evidence that lowering is defined. -/
structure Validated {source : Instance n} (plan : Plan source) : Type where
  valid : plan.Valid

/-- Validate every precondition needed to lower the selected SOS1 constraint. -/
def validate {source : Instance n} (plan : Plan source) :
    Option plan.Validated :=
  if hvalid : plan.Valid then some ⟨hvalid⟩ else none

namespace Validated

/-- Selector bounds recomputed from the validated source domains. -/
def bounds {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) : SelectorBounds plan.Member where
  lower := fun i =>
    (source.domains i).bound.finiteLower (validated.valid.2.1 i)
  upper := fun i =>
    (source.domains i).bound.finiteUpper (validated.valid.2.1 i)

theorem bounds_exact {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) (i : plan.Member) :
    (source.domains i).bound.lower =
        .finite (validated.bounds.lower i) ∧
      (source.domains i).bound.upper =
        .finite (validated.bounds.upper i) := by
  exact ⟨Bound.lower_eq_finiteLower _ (validated.valid.2.1 i),
    Bound.upper_eq_finiteUpper _ (validated.valid.2.1 i)⟩

theorem freshBoundsContainZero {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) :
    FreshBoundsContainZero plan.reusedMembers validated.bounds := by
  intro i hi
  have hzero := validated.valid.2.2 i hi
  rw [(validated.bounds_exact i).1, (validated.bounds_exact i).2] at hzero
  exact ⟨Endpoint.finite_le_finite.mp hzero.1,
    Endpoint.finite_le_finite.mp hzero.2⟩

theorem withinBounds_of_domains {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) {state : State n}
    (hdomains : ∀ i, state i ∈ source.domains i) :
    WithinSelectorBounds validated.bounds (plan.memberState state) := by
  intro i
  exact ⟨Domain.finite_lower_le (hdomains i) (validated.bounds_exact i).1,
    Domain.le_finite_upper (hdomains i) (validated.bounds_exact i).2⟩

end Validated

theorem reusedBinary_of_domains {source : Instance n} (plan : Plan source)
    {state : State n} (hdomains : ∀ i, state i ∈ source.domains i) :
    GenericBinaryOn plan.reusedMembers (plan.memberState state) := by
  intro i hi
  have hdomain : source.domains i = .binary :=
    (Finset.mem_filter.mp hi).2
  simpa [memberState, hdomain] using hdomains i

theorem genericSOS1_memberState_iff_holds {source : Instance n}
    (plan : Plan source) (state : State n) :
    GenericSOS1 (plan.memberState state) ↔ plan.constraint.Holds state := by
  classical
  rw [GenericSOS1, Finset.card_le_one]
  simp only [genericSupport, Finset.mem_filter, Finset.mem_univ, true_and]
  constructor
  · intro h i hi j hj hine hjne
    have hij :
        (⟨i, hi⟩ : plan.Member) = (⟨j, hj⟩ : plan.Member) := by
      apply h
      · simpa [memberState] using hine
      · simpa [memberState] using hjne
    exact congrArg Subtype.val hij
  · intro h i hi j hj
    apply Subtype.ext
    apply h i.val i.property j.val j.property
    · simpa [memberState] using hi
    · simpa [memberState] using hj

abbrev freshCount {source : Instance n} (plan : Plan source) : Nat :=
  plan.freshMembers.card

def freshMember {source : Instance n} (plan : Plan source)
    (j : Fin plan.freshCount) : plan.Member :=
  (plan.freshMembers.orderIsoOfFin rfl j).val

def freshIndex {source : Instance n} (plan : Plan source)
    (i : plan.Member) (hi : i ∈ plan.freshMembers) :
    Fin plan.freshCount :=
  (plan.freshMembers.orderIsoOfFin rfl).symm ⟨i, hi⟩

@[simp]
theorem freshMember_freshIndex {source : Instance n} (plan : Plan source)
    (i : plan.Member) (hi : i ∈ plan.freshMembers) :
    plan.freshMember (plan.freshIndex i hi) = i := by
  simp [freshMember, freshIndex]

@[simp]
theorem freshIndex_freshMember {source : Instance n} (plan : Plan source)
    (j : Fin plan.freshCount) :
    plan.freshIndex (plan.freshMember j) (by
      exact (plan.freshMembers.orderIsoOfFin rfl j).property) = j := by
  change (plan.freshMembers.orderIsoOfFin rfl).symm
    ((plan.freshMembers.orderIsoOfFin rfl) j) = j
  exact (plan.freshMembers.orderIsoOfFin rfl).symm_apply_apply j

end Plan

end SOS1BigM

end Instance

end OMMXProof
