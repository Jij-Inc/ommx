import OMMXProof.Instance
import OMMXProof.Instance.Transform.IndicatorBigM.Formulation
import Mathlib.Tactic

/-!
# Plan data for Indicator Big-M lowering

`Plan` identifies one source Indicator constraint. Validation checks that the
trigger and polarity are supported and that the affine image over the source
domain box has every finite side required by the body sense.
-/

namespace OMMXProof

namespace Instance

namespace IndicatorBigM

/-- A plan for lowering one occurrence in the source Indicator list. -/
structure Plan (source : Instance n) where
  constraintIndex : Fin source.indicatorConstraints.length

namespace Plan

def constraint {source : Instance n} (plan : Plan source) :
    IndicatorConstraint n :=
  source.indicatorConstraints.get plan.constraintIndex

def bodyValue {source : Instance n} (plan : Plan source)
    (state : State n) : Rat :=
  plan.constraint.body.expr.eval state

/-- The affine domain-box bound computed from the source domains. -/
def bodyBound {source : Instance n} (plan : Plan source) : Bound :=
  plan.constraint.body.expr.evaluateBoundFromDomains source.domains

theorem bodyValue_mem_bodyBound {source : Instance n} (plan : Plan source)
    {state : State n} (hdomains : ∀ i, state i ∈ source.domains i) :
    plan.bodyValue state ∈ plan.bodyBound := by
  simpa [bodyBound, bodyValue] using
    (Affine.evaluateBoundFromDomains_sound plan.constraint.body.expr
      source.domains hdomains)

/-- Exact validation performed before Big-M coefficients are extracted.

The selected Indicator must have a binary active-on-one trigger. A finite upper
body bound is required for both supported body senses; equality additionally
requires a finite lower body bound. -/
structure Valid {source : Instance n} (plan : Plan source) : Prop where
  triggerBinary : source.domains plan.constraint.trigger = .binary
  polarityActiveOnOne : plan.constraint.polarity = .activeOnOne
  upperFinite : plan.bodyBound.upper.IsFinite
  lowerFinite :
    match plan.constraint.body.sense with
    | .lessEqual => True
    | .equal => plan.bodyBound.lower.IsFinite

instance {source : Instance n} (plan : Plan source) : Decidable plan.Valid := by
  let proposition :=
    source.domains plan.constraint.trigger = .binary ∧
      plan.constraint.polarity = .activeOnOne ∧
      plan.bodyBound.upper.IsFinite ∧
      match plan.constraint.body.sense with
      | .lessEqual => True
      | .equal => plan.bodyBound.lower.IsFinite
  letI : Decidable proposition := by
    dsimp only [proposition]
    cases hsense : plan.constraint.body.sense <;> infer_instance
  exact decidable_of_iff proposition
    ⟨fun h ↦ ⟨h.1, h.2.1, h.2.2.1, h.2.2.2⟩,
      fun h ↦ ⟨h.triggerBinary, h.polarityActiveOnOne, h.upperFinite, h.lowerFinite⟩⟩

/-- A plan together with evidence that Indicator Big-M lowering is defined. -/
structure Validated {source : Instance n} (plan : Plan source) : Type where
  valid : plan.Valid

/-- Validate every precondition needed to lower the selected Indicator. -/
def validate {source : Instance n} (plan : Plan source) :
    Option plan.Validated :=
  if hvalid : plan.Valid then some ⟨hvalid⟩ else none

namespace Validated

/-- The finite upper body bound extracted from a validated plan. -/
def upper {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) : Rat :=
  plan.bodyBound.upper.finiteValue validated.valid.upperFinite

theorem upper_exact {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) :
    plan.bodyBound.upper = .finite validated.upper := by
  exact Endpoint.eq_finiteValue plan.bodyBound.upper
    validated.valid.upperFinite

/-- The finite lower body bound required by an equality Indicator. -/
def lower {source : Instance n} {plan : Plan source}
    (validated : plan.Validated)
    (hsense : plan.constraint.body.sense = .equal) : Rat :=
  plan.bodyBound.lower.finiteValue (by
    have hrequired := validated.valid.lowerFinite
    simpa [hsense] using hrequired)

theorem lower_exact {source : Instance n} {plan : Plan source}
    (validated : plan.Validated)
    (hsense : plan.constraint.body.sense = .equal) :
    plan.bodyBound.lower = .finite (validated.lower hsense) := by
  exact Endpoint.eq_finiteValue plan.bodyBound.lower (by
    have hrequired := validated.valid.lowerFinite
    simpa [hsense] using hrequired)

end Validated

end Plan

end IndicatorBigM

end Instance

end OMMXProof
