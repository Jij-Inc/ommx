import OMMXProof.Instance.Transform.SOS1BigM.SelectorFormulation
import Mathlib.Tactic

/-!
# Witness data for SOS1 Big-M promotion

An `SOS1BigM.Witness n` describes one standard Big-M selector formulation whose
retained decision variables occupy the left `n` components of a flat Instance.
Fresh selectors occupy the right block, and the regular constraints retained by
the promotion precede the selector-formulation constraints. The witness records
the bounds used by the link rows; validation checks that these bounds contain the
declared domains of the promoted members.

The witness is intentionally untrusted.  It determines a promoted target and
state maps unconditionally; `StandardBigMForm` in the validation module
records a conservative sufficient condition under which those maps preserve
feasibility and objective values.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

/-- Untrusted layout data for promoting one regular selector formulation to SOS1.

`freshMembers` records exactly those SOS1 members which the witness claims use
fresh selectors.  Their order determines the order of the right selector block.
Members outside `freshMembers` are claimed to be reused selectors.

For fresh members, `linkBounds` records the lower and upper coefficients used
by the Big-M link rows. They may be looser than the promoted members' declared
bounds. Reused members have no link rows; their entries provide the semantic
envelope required by the initial conservative checker.

`retainedConstraintCount` splits the regular-constraint list into a retained
prefix and a selector-formulation suffix. -/
structure Witness (n : Nat) where
  members : Finset (Fin n)
  freshMembers : Finset {i // i ∈ members}
  linkBounds : SelectorBounds {i // i ∈ members}
  retainedConstraintCount : Nat

namespace Witness

/-- The direction-independent selector layout described by this witness. -/
def selectorLayout (witness : Witness n) : SelectorLayout n where
  members := witness.members
  freshMembers := witness.freshMembers

/-- An SOS1 member selected by the witness. -/
abbrev Member (witness : Witness n) :=
  witness.selectorLayout.Member

/-- Restrict a retained state to the witnessed SOS1 members. -/
def memberState (witness : Witness n)
    (state : State n) : witness.Member → Rat :=
  witness.selectorLayout.memberState state

/-- Members which the witness claims are reused directly as binary selectors. -/
def reusedMembers (witness : Witness n) : Finset witness.Member :=
  witness.selectorLayout.reusedMembers

/-- Number of fresh selector variables in the flat source Instance. -/
abbrev freshCount (witness : Witness n) : Nat :=
  witness.selectorLayout.freshCount

/-- The member corresponding to one fresh selector in the right block. -/
def freshMember (witness : Witness n)
    (j : Fin witness.freshCount) : witness.Member :=
  witness.selectorLayout.freshMember j

/-- The right-block selector index corresponding to one fresh member. -/
def freshIndex (witness : Witness n)
    (i : witness.Member) (hi : i ∈ witness.freshMembers) :
    Fin witness.freshCount :=
  witness.selectorLayout.freshIndex i hi

/-- A virtual selector tuple indexed by every witnessed SOS1 member. -/
def freshSelectorState (witness : Witness n)
    (members : State n) (fresh : State witness.freshCount) :
    witness.Member → Rat :=
  witness.selectorLayout.freshSelectorState members fresh

@[simp]
theorem freshMember_freshIndex (witness : Witness n)
    (i : witness.Member) (hi : i ∈ witness.freshMembers) :
    witness.freshMember (witness.freshIndex i hi) = i := by
  exact witness.selectorLayout.freshMember_freshIndex i hi

@[simp]
theorem freshIndex_freshMember (witness : Witness n)
    (j : Fin witness.freshCount) :
    witness.freshIndex (witness.freshMember j) (by
      exact (witness.selectorLayout.freshMembers.orderIsoOfFin rfl j).property) = j := by
  exact witness.selectorLayout.freshIndex_freshMember j

end Witness

end SOS1BigM

end Instance

end OMMXProof
