import OMMXProof.Constraint.Linear

/-!
# Indicator constraint semantics

An Indicator constraint requires its linear body only when its binary trigger
has the selected active value.
-/

namespace OMMXProof

inductive IndicatorPolarity where
  | activeOnZero
  | activeOnOne
  deriving DecidableEq, Repr

namespace IndicatorPolarity

def activeValue : IndicatorPolarity → Rat
  | .activeOnZero => 0
  | .activeOnOne => 1

def Active (polarity : IndicatorPolarity) (value : Rat) : Prop :=
  value = polarity.activeValue

instance (polarity : IndicatorPolarity) (value : Rat) :
    Decidable (Active polarity value) := by
  unfold Active
  infer_instance

end IndicatorPolarity

structure IndicatorConstraint (n : Nat) where
  trigger : Fin n
  polarity : IndicatorPolarity
  body : LinearConstraint n

namespace IndicatorConstraint

def Holds (constraint : IndicatorConstraint n) (state : State n) : Prop :=
  constraint.polarity.Active (state constraint.trigger) →
    constraint.body.Holds state

instance (constraint : IndicatorConstraint n) (state : State n) :
    Decidable (constraint.Holds state) := by
  unfold Holds
  infer_instance

end IndicatorConstraint

def IndicatorPredicate (trigger : Fin n) (polarity : IndicatorPolarity)
    (body : State n → Prop) (state : State n) : Prop :=
  polarity.Active (state trigger) → body state

end OMMXProof
