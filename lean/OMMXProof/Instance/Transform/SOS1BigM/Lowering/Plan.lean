import OMMXProof.Instance
import OMMXProof.Instance.Transform.SOS1BigM.SelectorFormulation
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

/-- The shared selector layout induced by the selected source constraint.

Binary source members are reused directly; every other member receives a fresh
selector in the order determined by the resulting `Finset`. -/
def selectorLayout {source : Instance n} (plan : Plan source) :
    SelectorLayout n where
  members := plan.constraint.members
  freshMembers :=
    (Finset.univ.filter fun i => source.domains i = .binary)ᶜ

abbrev Member {source : Instance n} (plan : Plan source) :=
  plan.selectorLayout.Member

def memberState {source : Instance n} (plan : Plan source)
    (state : State n) : plan.Member → Rat :=
  plan.selectorLayout.memberState state

def reusedMembers {source : Instance n} (plan : Plan source) :
    Finset plan.Member :=
  plan.selectorLayout.reusedMembers

def freshMembers {source : Instance n} (plan : Plan source) :
    Finset plan.Member :=
  plan.selectorLayout.freshMembers

/-- Exact validation performed before rational selector bounds are extracted. -/
def Valid {source : Instance n} (plan : Plan source) : Prop :=
  ∀ i : plan.Member, (source.domains i).bound.IsFinite

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
    (source.domains i).bound.finiteLower (validated.valid i)
  upper := fun i =>
    (source.domains i).bound.finiteUpper (validated.valid i)

theorem bounds_exact {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) (i : plan.Member) :
    (source.domains i).bound.lower =
        .finite (validated.bounds.lower i) ∧
      (source.domains i).bound.upper =
        .finite (validated.bounds.upper i) := by
  exact ⟨Bound.lower_eq_finiteLower _ (validated.valid i),
    Bound.upper_eq_finiteUpper _ (validated.valid i)⟩

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
  have hdomain : source.domains i = .binary := by
    have hmem := hi
    change
      i ∈ ((Finset.univ.filter
        fun i : plan.Member => source.domains i = .binary)ᶜ)ᶜ at hmem
    rw [compl_compl] at hmem
    exact (Finset.mem_filter.mp hmem).2
  simpa [memberState, SelectorLayout.memberState, hdomain] using hdomains i

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
      · simpa [memberState, SelectorLayout.memberState] using hine
      · simpa [memberState, SelectorLayout.memberState] using hjne
    exact congrArg Subtype.val hij
  · intro h i hi j hj
    apply Subtype.ext
    apply h i.val i.property j.val j.property
    · simpa [memberState, SelectorLayout.memberState] using hi
    · simpa [memberState, SelectorLayout.memberState] using hj

abbrev freshCount {source : Instance n} (plan : Plan source) : Nat :=
  plan.selectorLayout.freshCount

def freshMember {source : Instance n} (plan : Plan source)
    (j : Fin plan.freshCount) : plan.Member :=
  plan.selectorLayout.freshMember j

def freshIndex {source : Instance n} (plan : Plan source)
    (i : plan.Member) (hi : i ∈ plan.freshMembers) :
    Fin plan.freshCount :=
  plan.selectorLayout.freshIndex i hi

@[simp]
theorem freshMember_freshIndex {source : Instance n} (plan : Plan source)
    (i : plan.Member) (hi : i ∈ plan.freshMembers) :
    plan.freshMember (plan.freshIndex i hi) = i := by
  exact plan.selectorLayout.freshMember_freshIndex i hi

@[simp]
theorem freshIndex_freshMember {source : Instance n} (plan : Plan source)
    (j : Fin plan.freshCount) :
    plan.freshIndex (plan.freshMember j) (by
      exact (plan.freshMembers.orderIsoOfFin rfl j).property) = j := by
  exact plan.selectorLayout.freshIndex_freshMember j

end Plan

end SOS1BigM

end Instance

end OMMXProof
