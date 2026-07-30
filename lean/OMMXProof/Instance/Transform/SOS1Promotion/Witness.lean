import OMMXProof.Instance
import Mathlib.Tactic

/-!
# Witness data for SOS1 promotion

An `SOS1Promotion.Witness n` describes one standard selector formulation whose
retained decision variables occupy the left `n` components of a flat Instance.
Fresh selectors occupy the right block, and the regular constraints retained by
the promotion precede the selector-formulation constraints.

The witness is intentionally untrusted.  It determines a promoted target and
state maps unconditionally; `StandardBigMForm` in the target module records a
conservative sufficient condition under which those maps preserve feasibility
and objective values.
-/

namespace OMMXProof

namespace Instance

namespace SOS1Promotion

/-- Untrusted layout data for promoting one regular selector formulation to SOS1.

`freshMembers` records exactly those SOS1 members which the witness claims use
fresh selectors.  Their order determines the order of the right selector block.
Members outside `freshMembers` are claimed to be reused selectors.

`retainedConstraintCount` splits the regular-constraint list into a retained
prefix and a selector-formulation suffix. -/
structure Witness (n : Nat) where
  members : Finset (Fin n)
  freshMembers : Finset {i // i ∈ members}
  retainedConstraintCount : Nat

namespace Witness

/-- An SOS1 member selected by the witness. -/
abbrev Member (witness : Witness n) :=
  {i // i ∈ witness.members}

/-- Members which the witness claims are reused directly as binary selectors. -/
def reusedMembers (witness : Witness n) : Finset witness.Member :=
  witness.freshMembersᶜ

/-- Number of fresh selector variables in the flat source Instance. -/
abbrev freshCount (witness : Witness n) : Nat :=
  witness.freshMembers.card

/-- The member corresponding to one fresh selector in the right block. -/
def freshMember (witness : Witness n)
    (j : Fin witness.freshCount) : witness.Member :=
  (witness.freshMembers.orderIsoOfFin rfl j).val

/-- The right-block selector index corresponding to one fresh member. -/
def freshIndex (witness : Witness n)
    (i : witness.Member) (hi : i ∈ witness.freshMembers) :
    Fin witness.freshCount :=
  (witness.freshMembers.orderIsoOfFin rfl).symm ⟨i, hi⟩

@[simp]
theorem freshMember_freshIndex (witness : Witness n)
    (i : witness.Member) (hi : i ∈ witness.freshMembers) :
    witness.freshMember (witness.freshIndex i hi) = i := by
  simp [freshMember, freshIndex]

@[simp]
theorem freshIndex_freshMember (witness : Witness n)
    (j : Fin witness.freshCount) :
    witness.freshIndex (witness.freshMember j) (by
      exact (witness.freshMembers.orderIsoOfFin rfl j).property) = j := by
  change (witness.freshMembers.orderIsoOfFin rfl).symm
    ((witness.freshMembers.orderIsoOfFin rfl) j) = j
  exact (witness.freshMembers.orderIsoOfFin rfl).symm_apply_apply j

end Witness

end SOS1Promotion

end Instance

end OMMXProof
