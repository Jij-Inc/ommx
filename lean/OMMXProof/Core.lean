import OMMXProof.Constraint.Indicator
import OMMXProof.Constraint.OneHot
import OMMXProof.Constraint.SOS1
import OMMXProof.SemanticProblem

/-!
# Exact semantic core

This module assembles the independent constraint semantics into a finite
optimization Instance. It deliberately has no OMMX Rust, protobuf,
floating-point, lifecycle, or identifier semantics.
-/

namespace OMMXProof

/--
# Simplified semantic model for OMMX Instance.

- IDs are packed into a finite index space `Fin n`, while Rust SDK uses stable parse IDs.

## Temporal Limitations

- Only Affine expressions are supported, while Rust SDK supports arbitrary polynomial expressions.
-/
structure Instance (n : Nat) where
  domains : Fin n → Domain
  constraints : List (LinearConstraint n)
  oneHotConstraints : List (OneHotConstraint n) := []
  sos1Constraints : List (SOS1Constraint n) := []
  indicatorConstraints : List (IndicatorConstraint n) := []
  objective : Affine n
  sense : OptimizationSense

namespace Instance

def Feasible (inst : Instance n) (state : State n) : Prop :=
  (∀ i, state i ∈ inst.domains i) ∧
    (∀ constraint ∈ inst.constraints, constraint.Holds state) ∧
    (∀ constraint ∈ inst.oneHotConstraints, constraint.Holds state) ∧
    (∀ constraint ∈ inst.sos1Constraints, constraint.Holds state) ∧
    ∀ constraint ∈ inst.indicatorConstraints, constraint.Holds state

def ObjectiveValue (inst : Instance n) (state : State n) : Rat :=
  inst.objective.eval state

def asSemanticProblem (inst : Instance n) : SemanticProblem (State n) where
  feasible := inst.Feasible
  objective := inst.ObjectiveValue
  sense := inst.sense

theorem constraintsFeasible_of_feasible {inst : Instance n} {state : State n}
    (h : inst.Feasible state) :
    ∀ constraint ∈ inst.constraints, constraint.Holds state := h.2.1

end Instance

end OMMXProof
