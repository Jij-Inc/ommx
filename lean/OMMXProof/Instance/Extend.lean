import OMMXProof.Instance
import Mathlib.Algebra.BigOperators.Fin
import Mathlib.Data.Fin.Tuple.Basic

/-!
# Extending an Instance with fresh components

An Instance transformation may change the dimension of its state space.  This
module embeds an `n`-component Instance into the left block of an
`(n + k)`-component Instance and supplies the corresponding state operations.
The fresh right block is absent from every lifted expression and constraint.
-/

namespace OMMXProof

namespace State

def append (source : State n) (fresh : State k) : State (n + k) :=
  Fin.append source fresh

def source (state : State (n + k)) : State n :=
  fun i => state (Fin.castAdd k i)

def fresh (state : State (n + k)) : State k :=
  fun i => state (Fin.natAdd n i)

@[simp]
theorem source_append (sourceState : State n) (freshState : State k) :
    source (append sourceState freshState) = sourceState := by
  funext i
  simp [source, append]

@[simp]
theorem fresh_append (sourceState : State n) (freshState : State k) :
    fresh (append sourceState freshState) = freshState := by
  funext i
  simp [fresh, append]

@[simp]
theorem append_source_fresh (state : State (n + k)) :
    append (source state) (fresh state) = state := by
  exact Fin.append_castAdd_natAdd

end State

namespace Affine

/-- Embed an affine function in the left block of a larger state space. -/
def extend (expr : Affine n) (k : Nat) : Affine (n + k) where
  coeff := Fin.append expr.coeff (fun _ => 0)
  constant := expr.constant

@[simp]
theorem extend_coeff_source (expr : Affine n) (k : Nat) (i : Fin n) :
    (extend expr k).coeff (Fin.castAdd k i) = expr.coeff i := by
  simp [extend]

@[simp]
theorem extend_coeff_fresh (expr : Affine n) (k : Nat) (i : Fin k) :
    (extend expr k).coeff (Fin.natAdd n i) = 0 := by
  simp [extend]

@[simp]
theorem eval_extend_append (expr : Affine n)
    (sourceState : State n) (freshState : State k) :
    (extend expr k).eval (State.append sourceState freshState) =
      expr.eval sourceState := by
  simp [Affine.eval, extend, State.append, Fin.sum_univ_add]

@[simp]
theorem eval_extend (expr : Affine n) (state : State (n + k)) :
    (extend expr k).eval state = expr.eval (State.source state) := by
  simpa only [State.append_source_fresh] using
    eval_extend_append expr (State.source state) (State.fresh state)

end Affine

namespace LinearConstraint

def extend (constraint : LinearConstraint n) (k : Nat) :
    LinearConstraint (n + k) where
  expr := constraint.expr.extend k
  sense := constraint.sense

@[simp]
theorem holds_extend_append (constraint : LinearConstraint n)
    (sourceState : State n) (freshState : State k) :
    (extend constraint k).Holds (State.append sourceState freshState) ↔
      constraint.Holds sourceState := by
  rcases constraint with ⟨expr, sense⟩
  cases sense <;> simp [extend, Holds]

@[simp]
theorem holds_extend (constraint : LinearConstraint n)
    (state : State (n + k)) :
    (extend constraint k).Holds state ↔
      constraint.Holds (State.source state) := by
  simpa only [State.append_source_fresh] using
    holds_extend_append constraint (State.source state) (State.fresh state)

end LinearConstraint

def extendMembers (members : Finset (Fin n)) (k : Nat) :
    Finset (Fin (n + k)) :=
  members.map (Fin.castAddEmb k)

@[simp]
theorem mem_extendMembers (members : Finset (Fin n)) (k : Nat) (i : Fin n) :
    Fin.castAdd k i ∈ extendMembers members k ↔ i ∈ members := by
  simp [extendMembers]

namespace OneHotConstraint

def extend (constraint : OneHotConstraint n) (k : Nat) :
    OneHotConstraint (n + k) where
  members := extendMembers constraint.members k

@[simp]
theorem holds_extend_append (constraint : OneHotConstraint n)
    (sourceState : State n) (freshState : State k) :
    (extend constraint k).Holds (State.append sourceState freshState) ↔
      constraint.Holds sourceState := by
  simp [extend, Holds, extendMembers, State.append]

@[simp]
theorem holds_extend (constraint : OneHotConstraint n)
    (state : State (n + k)) :
    (extend constraint k).Holds state ↔
      constraint.Holds (State.source state) := by
  simpa only [State.append_source_fresh] using
    holds_extend_append constraint (State.source state) (State.fresh state)

end OneHotConstraint

namespace SOS1Constraint

def extend (constraint : SOS1Constraint n) (k : Nat) :
    SOS1Constraint (n + k) where
  members := extendMembers constraint.members k

@[simp]
theorem holds_extend_append (constraint : SOS1Constraint n)
    (sourceState : State n) (freshState : State k) :
    (extend constraint k).Holds (State.append sourceState freshState) ↔
      constraint.Holds sourceState := by
  simp [extend, Holds, extendMembers, State.append]

@[simp]
theorem holds_extend (constraint : SOS1Constraint n)
    (state : State (n + k)) :
    (extend constraint k).Holds state ↔
      constraint.Holds (State.source state) := by
  simpa only [State.append_source_fresh] using
    holds_extend_append constraint (State.source state) (State.fresh state)

end SOS1Constraint

namespace IndicatorConstraint

def extend (constraint : IndicatorConstraint n) (k : Nat) :
    IndicatorConstraint (n + k) where
  trigger := Fin.castAdd k constraint.trigger
  polarity := constraint.polarity
  body := constraint.body.extend k

@[simp]
theorem holds_extend_append (constraint : IndicatorConstraint n)
    (sourceState : State n) (freshState : State k) :
    (extend constraint k).Holds (State.append sourceState freshState) ↔
      constraint.Holds sourceState := by
  constructor
  · intro hextended hactive
    apply (LinearConstraint.holds_extend_append constraint.body
      sourceState freshState).mp
    apply hextended
    simpa [extend, State.append] using hactive
  · intro hsource hactive
    apply (LinearConstraint.holds_extend_append constraint.body
      sourceState freshState).mpr
    apply hsource
    simpa [extend, State.append] using hactive

@[simp]
theorem holds_extend (constraint : IndicatorConstraint n)
    (state : State (n + k)) :
    (extend constraint k).Holds state ↔
      constraint.Holds (State.source state) := by
  simpa only [State.append_source_fresh] using
    holds_extend_append constraint (State.source state) (State.fresh state)

end IndicatorConstraint

namespace Domain

def append (source : Fin n → Domain) (fresh : Fin k → Domain) :
    Fin (n + k) → Domain :=
  Fin.append source fresh

@[simp]
theorem append_source (source : Fin n → Domain) (fresh : Fin k → Domain)
    (i : Fin n) :
    append source fresh (Fin.castAdd k i) = source i := by
  simp [append]

@[simp]
theorem append_fresh (source : Fin n → Domain) (fresh : Fin k → Domain)
    (i : Fin k) :
    append source fresh (Fin.natAdd n i) = fresh i := by
  simp [append]

end Domain

namespace Instance

/-- Extend an Instance with fresh domains that are otherwise unused. -/
def extend (inst : Instance n) (freshDomains : Fin k → Domain) :
    Instance (n + k) where
  domains := Domain.append inst.domains freshDomains
  constraints := inst.constraints.map fun constraint =>
    constraint.extend k
  oneHotConstraints := inst.oneHotConstraints.map fun constraint =>
    constraint.extend k
  sos1Constraints := inst.sos1Constraints.map fun constraint =>
    constraint.extend k
  indicatorConstraints := inst.indicatorConstraints.map fun constraint =>
    constraint.extend k
  objective := inst.objective.extend k
  sense := inst.sense

@[simp]
theorem feasible_extend (inst : Instance n) (freshDomains : Fin k → Domain)
    (state : State (n + k)) :
    (extend inst freshDomains).Feasible state ↔
      inst.Feasible (State.source state) ∧
        ∀ j, state (Fin.natAdd n j) ∈ freshDomains j := by
  simp only [Feasible, extend, Domain.append, Fin.forall_fin_add,
    Fin.append_left, Fin.append_right, LinearConstraint.holds_extend,
    OneHotConstraint.holds_extend, SOS1Constraint.holds_extend,
    IndicatorConstraint.holds_extend, List.forall_mem_map]
  aesop

@[simp]
theorem objectiveValue_extend (inst : Instance n)
    (freshDomains : Fin k → Domain) (state : State (n + k)) :
    (extend inst freshDomains).ObjectiveValue state =
      inst.ObjectiveValue (State.source state) := by
  simp [ObjectiveValue, extend]

end Instance

end OMMXProof
