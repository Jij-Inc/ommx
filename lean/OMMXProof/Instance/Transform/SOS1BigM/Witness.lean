import OMMXProof.Instance
import OMMXProof.Instance.Transform.SOS1BigM.Formulation
import Mathlib.Tactic

/-!
# Witness data for SOS1 Big-M lowering

`Witness` identifies one source SOS1 constraint and records the exact bounds
used by its selector formulation. The reused/fresh member partition is derived
from the source domains and is therefore not duplicated in the witness data.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

/-- A witness for lowering one occurrence in the source SOS1 list.

Bounds are indexed only by members of the selected SOS1 constraint. -/
structure Witness (source : Instance n) where
  constraintIndex : Fin source.sos1Constraints.length
  bounds : SelectorBounds
    {i // i ∈ (source.sos1Constraints.get constraintIndex).members}

namespace Witness

def constraint {source : Instance n} (witness : Witness source) : SOS1Constraint n :=
  source.sos1Constraints.get witness.constraintIndex

abbrev Member {source : Instance n} (witness : Witness source) :=
  {i // i ∈ witness.constraint.members}

def memberState {source : Instance n} (witness : Witness source)
    (state : State n) : witness.Member → Rat :=
  fun i => state i

def reusedMembers {source : Instance n} (witness : Witness source) :
    Finset witness.Member :=
  Finset.univ.filter fun i => source.domains i = .binary

def freshMembers {source : Instance n} (witness : Witness source) :
    Finset witness.Member :=
  witness.reusedMembersᶜ

/-- Exact validation performed before the target Instance is trusted.

`Rat` makes finiteness intrinsic. The independent `Domain` syntax has no
semi-continuous or semi-integer cases, so no additional kind rejection is
needed here. -/
def Valid {source : Instance n} (witness : Witness source) : Prop :=
  witness.constraint.members.Nonempty ∧
    (∀ i : witness.Member,
      (source.domains i).bound.lower =
          Endpoint.finite (witness.bounds.lower i) ∧
        (source.domains i).bound.upper =
          Endpoint.finite (witness.bounds.upper i)) ∧
    FreshBoundsContainZero witness.reusedMembers witness.bounds

instance {source : Instance n} (witness : Witness source) :
    Decidable witness.Valid := by
  unfold Valid
  infer_instance

theorem withinBounds_of_domains {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) {state : State n}
    (hdomains : ∀ i, state i ∈ source.domains i) :
    WithinSelectorBounds witness.bounds (witness.memberState state) := by
  intro i
  have hlower := (hvalid.2.1 i).1
  have hupper := (hvalid.2.1 i).2
  exact ⟨Domain.finite_lower_le (hdomains i) hlower,
    Domain.le_finite_upper (hdomains i) hupper⟩

theorem reusedBinary_of_domains {source : Instance n} (witness : Witness source)
    {state : State n} (hdomains : ∀ i, state i ∈ source.domains i) :
    GenericBinaryOn witness.reusedMembers (witness.memberState state) := by
  intro i hi
  have hdomain : source.domains i = .binary :=
    (Finset.mem_filter.mp hi).2
  simpa [memberState, hdomain] using hdomains i

theorem genericSOS1_memberState_iff_holds {source : Instance n}
    (witness : Witness source) (state : State n) :
    GenericSOS1 (witness.memberState state) ↔ witness.constraint.Holds state := by
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

abbrev freshCount {source : Instance n} (witness : Witness source) : Nat :=
  witness.freshMembers.card

def freshMember {source : Instance n} (witness : Witness source)
    (j : Fin witness.freshCount) : witness.Member :=
  (witness.freshMembers.orderIsoOfFin rfl j).val

def freshIndex {source : Instance n} (witness : Witness source)
    (i : witness.Member) (hi : i ∈ witness.freshMembers) :
    Fin witness.freshCount :=
  (witness.freshMembers.orderIsoOfFin rfl).symm ⟨i, hi⟩

@[simp]
theorem freshMember_freshIndex {source : Instance n} (witness : Witness source)
    (i : witness.Member) (hi : i ∈ witness.freshMembers) :
    witness.freshMember (witness.freshIndex i hi) = i := by
  simp [freshMember, freshIndex]

@[simp]
theorem freshIndex_freshMember {source : Instance n} (witness : Witness source)
    (j : Fin witness.freshCount) :
    witness.freshIndex (witness.freshMember j) (by
      exact (witness.freshMembers.orderIsoOfFin rfl j).property) = j := by
  change (witness.freshMembers.orderIsoOfFin rfl).symm
    ((witness.freshMembers.orderIsoOfFin rfl) j) = j
  exact (witness.freshMembers.orderIsoOfFin rfl).symm_apply_apply j

end Witness

end SOS1BigM

end Instance

end OMMXProof
