import OMMXProof.Instance.Transform
import Mathlib.Tactic

/-!
# Indicator Big-M lowering as an Instance transformation

This module models the SDK lowering of one active-on-one Indicator constraint.
The selected Indicator is removed, its nonredundant Big-M sides are appended to
the regular constraints, and every other part of the source Instance is
preserved. No decision variable is introduced, so both state maps are the
identity.

`Plan.Valid` certifies that the trigger is binary and that the affine bound
computed from the source domains has every finite side required by the
lowering. `Plan.create` computes that bound and returns a valid plan when those
sides are available. A finite lower endpoint is required only for an equality
Indicator.
-/

namespace OMMXProof

namespace Instance

namespace IndicatorBigM

/-! ## Pointwise Big-M semantics -/

/-- The upper Big-M side `f(x) + u y - u ≤ 0`, omitted when `u ≤ 0`. -/
def UpperSide (body : State n → Rat) (trigger : Fin n) (upper : Rat)
    (state : State n) : Prop :=
  if 0 < upper then
    body state + upper * state trigger - upper ≤ 0
  else
    True

/-- The lower Big-M side `-f(x) - l y + l ≤ 0`, omitted when `l ≥ 0`. -/
def LowerSide (body : State n → Rat) (trigger : Fin n) (lower : Rat)
    (state : State n) : Prop :=
  if lower < 0 then
    -body state - lower * state trigger + lower ≤ 0
  else
    True

theorem upperSide_iff_indicator {body : State n → Rat} {trigger : Fin n}
    {upper : Rat} {state : State n}
    (hbinary : state trigger ∈ Domain.binary)
    (hbound : body state ≤ upper) :
    UpperSide body trigger upper state ↔
      IndicatorPredicate trigger .activeOnOne
        (fun x => body x ≤ 0) state := by
  rcases hbinary with hzero | hone
  · by_cases hupper : 0 < upper
    · simp [UpperSide, hupper, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hzero]
      linarith
    · simp [UpperSide, hupper, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hzero]
  · by_cases hupper : 0 < upper
    · simp [UpperSide, hupper, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hone]
    · have hnonpos : upper ≤ 0 := le_of_not_gt hupper
      have hbody : body state ≤ 0 := le_trans hbound hnonpos
      simp [UpperSide, hupper, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hone, hbody]

theorem lowerSide_iff_indicator {body : State n → Rat} {trigger : Fin n}
    {lower : Rat} {state : State n}
    (hbinary : state trigger ∈ Domain.binary)
    (hbound : lower ≤ body state) :
    LowerSide body trigger lower state ↔
      IndicatorPredicate trigger .activeOnOne
        (fun x => 0 ≤ body x) state := by
  rcases hbinary with hzero | hone
  · by_cases hlower : lower < 0
    · simp [LowerSide, hlower, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hzero]
      linarith
    · simp [LowerSide, hlower, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hzero]
  · by_cases hlower : lower < 0
    · simp [LowerSide, hlower, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hone]
    · have hnonneg : 0 ≤ lower := le_of_not_gt hlower
      have hbody : 0 ≤ body state := le_trans hnonneg hbound
      simp [LowerSide, hlower, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hone, hbody]

theorem equalitySides_iff_indicator {body : State n → Rat}
    {trigger : Fin n} {lower upper : Rat} {state : State n}
    (hbinary : state trigger ∈ Domain.binary)
    (hlower : lower ≤ body state) (hupper : body state ≤ upper) :
    UpperSide body trigger upper state ∧
        LowerSide body trigger lower state ↔
      IndicatorPredicate trigger .activeOnOne
        (fun x => body x = 0) state := by
  rw [upperSide_iff_indicator hbinary hupper,
    lowerSide_iff_indicator hbinary hlower]
  unfold IndicatorPredicate
  constructor
  · rintro ⟨hupperSide, hlowerSide⟩ hactive
    exact le_antisymm (hupperSide hactive) (hlowerSide hactive)
  · intro hequal
    constructor
    · intro hactive
      exact le_of_eq (hequal hactive)
    · intro hactive
      exact le_of_eq (hequal hactive).symm

/-! ## Instance-level lowering plan -/

/-- One planned conversion of one occurrence in the source Indicator list.

`bodyBound` is the affine image bound computed from the source domains. It
uses explicit infinite endpoints through `Bound`, rather than optional
rational endpoints. -/
structure Plan (source : Instance n) where
  constraintIndex : Fin source.indicatorConstraints.length
  bodyBound : Bound
  deriving DecidableEq

namespace Plan

def constraint {source : Instance n} (plan : Plan source) :
    IndicatorConstraint n :=
  source.indicatorConstraints.get plan.constraintIndex

def bodyValue {source : Instance n} (plan : Plan source)
    (state : State n) : Rat :=
  plan.constraint.body.expr.eval state

/-- The affine image bound determined by the source domains. -/
def computedBodyBound {source : Instance n} (plan : Plan source) : Bound :=
  plan.constraint.body.expr.evaluateBound source.domains

/-- Executable validation required before the target Instance is trusted.

The stored bound must be exactly the one computed from the source domain box.
Both body senses need a finite upper endpoint. Equality additionally needs a
finite lower endpoint. -/
def Valid {source : Instance n} (plan : Plan source) : Prop :=
  source.domains plan.constraint.trigger = .binary ∧
    plan.constraint.polarity = .activeOnOne ∧
    plan.bodyBound = plan.computedBodyBound ∧
    match plan.bodyBound.upper with
    | .finite _ =>
        match plan.constraint.body.sense with
        | .lessEqual => True
        | .equal =>
            match plan.bodyBound.lower with
            | .finite _ => True
            | _ => False
    | _ => False

instance {source : Instance n} (plan : Plan source) :
    Decidable plan.Valid := by
  unfold Valid
  cases hupper : plan.bodyBound.upper <;>
    cases hsense : plan.constraint.body.sense <;>
    cases hlower : plan.bodyBound.lower <;>
    infer_instance

/-- Compute an Indicator Big-M plan from exact affine bounds over the source
domain box.

The selected Indicator must be active on one with a binary trigger. A finite
upper bound is required for both supported body senses. A finite lower bound is
required only for equality. -/
def create (source : Instance n)
    (constraintIndex : Fin source.indicatorConstraints.length) :
    Option (Plan source) :=
  let constraint := source.indicatorConstraints.get constraintIndex
  let plan : Plan source :=
    { constraintIndex
      bodyBound := constraint.body.expr.evaluateBound source.domains }
  if plan.Valid then some plan else none

/-- Every plan returned by `create` satisfies the semantic plan certificate. -/
theorem create_valid {source : Instance n}
    {constraintIndex : Fin source.indicatorConstraints.length}
    {plan : Plan source}
    (hcreate : create source constraintIndex = some plan) :
    plan.Valid := by
  unfold create at hcreate
  dsimp only at hcreate
  split at hcreate
  · rename_i hvalid
    simp only [Option.some.injEq] at hcreate
    subst plan
    exact hvalid
  · contradiction

/-- A constant affine expression used to spell the generated rows. -/
def constantExpr (value : Rat) : Affine n where
  coeff := fun _ => 0
  constant := value

@[simp]
theorem eval_constantExpr (value : Rat) (state : State n) :
    (constantExpr value).eval state = value := by
  simp [constantExpr, Affine.eval]

/-- The generated row `f(x) + u y - u ≤ 0`. -/
def upperConstraint {source : Instance n} (plan : Plan source)
    (upper : Rat) :
    LinearConstraint n where
  expr := Affine.sub
    (Affine.add plan.constraint.body.expr
      (Affine.scale upper
        (Affine.coordinate plan.constraint.trigger)))
    (constantExpr upper)
  sense := .lessEqual

/-- The generated row `-f(x) - l y + l ≤ 0`. -/
def lowerConstraint {source : Instance n} (plan : Plan source)
    (lower : Rat) : LinearConstraint n where
  expr := Affine.add
    (Affine.sub (Affine.neg plan.constraint.body.expr)
      (Affine.scale lower
        (Affine.coordinate plan.constraint.trigger)))
    (constantExpr lower)
  sense := .lessEqual

@[simp]
theorem upperConstraint_holds {source : Instance n} (plan : Plan source)
    (upper : Rat) (state : State n) :
    (plan.upperConstraint upper).Holds state ↔
      plan.bodyValue state +
          upper * state plan.constraint.trigger - upper ≤ 0 := by
  simp [upperConstraint, bodyValue, LinearConstraint.Holds]

@[simp]
theorem lowerConstraint_holds {source : Instance n} (plan : Plan source)
    (lower : Rat) (state : State n) :
    (plan.lowerConstraint lower).Holds state ↔
      -plan.bodyValue state -
          lower * state plan.constraint.trigger + lower ≤ 0 := by
  simp [lowerConstraint, bodyValue, LinearConstraint.Holds]

def upperConstraints {source : Instance n} (plan : Plan source) :
    List (LinearConstraint n) :=
  match plan.bodyBound.upper with
  | .finite upper =>
      if 0 < upper then [plan.upperConstraint upper] else []
  | _ => []

def lowerConstraints {source : Instance n} (plan : Plan source) :
    List (LinearConstraint n) :=
  match plan.constraint.body.sense, plan.bodyBound.lower with
  | .equal, .finite lower =>
      if lower < 0 then [plan.lowerConstraint lower] else []
  | _, _ => []

/-- Generated regular constraints, in SDK insertion order. -/
def generatedConstraints {source : Instance n} (plan : Plan source) :
    List (LinearConstraint n) :=
  plan.upperConstraints ++ plan.lowerConstraints

theorem upperConstraints_hold_iff {source : Instance n} (plan : Plan source)
    (state : State n) {upper : Rat}
    (hupper : plan.bodyBound.upper = .finite upper) :
    (∀ constraint ∈ plan.upperConstraints, constraint.Holds state) ↔
      UpperSide plan.bodyValue plan.constraint.trigger upper state := by
  by_cases hemitted : 0 < upper <;>
    simp [upperConstraints, UpperSide, hupper, hemitted]

theorem lowerConstraints_hold_iff {source : Instance n} (plan : Plan source)
    (state : State n) {lower : Rat}
    (hsense : plan.constraint.body.sense = .equal)
    (hlower : plan.bodyBound.lower = .finite lower) :
    (∀ constraint ∈ plan.lowerConstraints, constraint.Holds state) ↔
      LowerSide plan.bodyValue plan.constraint.trigger lower state := by
  by_cases hemitted : lower < 0 <;>
    simp [lowerConstraints, LowerSide, hsense, hlower, hemitted]

/-- The generated rows have exactly the selected Indicator's denotation on
states satisfying the source domains. -/
theorem generatedConstraints_hold_iff {source : Instance n}
    (plan : Plan source) (hvalid : plan.Valid) {state : State n}
    (hdomains : ∀ i, state i ∈ source.domains i) :
    (∀ constraint ∈ plan.generatedConstraints,
      constraint.Holds state) ↔
        plan.constraint.Holds state := by
  rcases hvalid with
    ⟨htriggerDomain, hpolarity, hboundExact, hfiniteSides⟩
  have hbinary : state plan.constraint.trigger ∈ Domain.binary := by
    have htrigger := hdomains plan.constraint.trigger
    rw [htriggerDomain] at htrigger
    exact htrigger
  have hbodyInBound :
      plan.bodyValue state ∈ plan.bodyBound := by
    rw [hboundExact]
    simpa [computedBodyBound, bodyValue] using
      (Affine.evaluateBound_sound plan.constraint.body.expr
        source.domains hdomains)
  cases hupper : plan.bodyBound.upper with
  | negInf =>
      simp [hupper] at hfiniteSides
  | posInf =>
      simp [hupper] at hfiniteSides
  | finite upper =>
      have hupperBound : plan.bodyValue state ≤ upper :=
        Bound.le_finite_upper hbodyInBound hupper
      cases hsense : plan.constraint.body.sense with
      | lessEqual =>
          have hgenerated :
              (∀ constraint ∈ plan.generatedConstraints,
                constraint.Holds state) ↔
                UpperSide plan.bodyValue plan.constraint.trigger
                  upper state := by
            rw [generatedConstraints, List.forall_mem_append,
              plan.upperConstraints_hold_iff state hupper]
            simp [lowerConstraints, hsense]
          exact hgenerated.trans (by
            simpa [IndicatorConstraint.Holds, IndicatorPredicate, hpolarity,
              LinearConstraint.Holds, hsense, bodyValue] using
                (upperSide_iff_indicator hbinary hupperBound))
      | equal =>
          cases hlower : plan.bodyBound.lower with
          | negInf =>
              simp [hupper, hsense, hlower] at hfiniteSides
          | posInf =>
              simp [hupper, hsense, hlower] at hfiniteSides
          | finite lower =>
              have hlowerBound : lower ≤ plan.bodyValue state :=
                Bound.finite_lower_le hbodyInBound hlower
              have hgenerated :
                  (∀ constraint ∈ plan.generatedConstraints,
                    constraint.Holds state) ↔
                    UpperSide plan.bodyValue plan.constraint.trigger
                        upper state ∧
                      LowerSide plan.bodyValue plan.constraint.trigger
                        lower state := by
                rw [generatedConstraints, List.forall_mem_append,
                  plan.upperConstraints_hold_iff state hupper,
                  plan.lowerConstraints_hold_iff state hsense hlower]
              exact hgenerated.trans (by
                simpa [IndicatorConstraint.Holds, IndicatorPredicate,
                  hpolarity, LinearConstraint.Holds, hsense, bodyValue] using
                    (equalitySides_iff_indicator hbinary
                      hlowerBound hupperBound))

/-! ## Target Instance and transformation contracts -/

def BaseFeasible {source : Instance n} (plan : Plan source)
    (state : State n) : Prop :=
  (∀ i, state i ∈ source.domains i) ∧
    (∀ constraint ∈ source.constraints, constraint.Holds state) ∧
    (∀ constraint ∈ source.oneHotConstraints, constraint.Holds state) ∧
    (∀ constraint ∈ source.sos1Constraints, constraint.Holds state) ∧
    ∀ constraint ∈
      source.indicatorConstraints.eraseIdx plan.constraintIndex.val,
      constraint.Holds state

theorem allIndicators_iff_erased_and_selected {source : Instance n}
    (plan : Plan source) (state : State n) :
    (∀ constraint ∈ source.indicatorConstraints,
      constraint.Holds state) ↔
      (∀ constraint ∈
        source.indicatorConstraints.eraseIdx plan.constraintIndex.val,
        constraint.Holds state) ∧
        plan.constraint.Holds state := by
  constructor
  · intro hall
    constructor
    · intro constraint hconstraint
      exact hall constraint (List.mem_of_mem_eraseIdx hconstraint)
    · exact hall plan.constraint
        (List.get_mem _ plan.constraintIndex)
  · rintro ⟨herased, hselected⟩ constraint hconstraint
    have hperm := List.getElem_cons_eraseIdx_perm
      (l := source.indicatorConstraints) plan.constraintIndex.isLt
    have hleft :
        constraint ∈
          plan.constraint ::
            source.indicatorConstraints.eraseIdx
              plan.constraintIndex.val := by
      exact hperm.symm.subset hconstraint
    rcases List.mem_cons.mp hleft with hsame | herasedMem
    · simpa [hsame] using hselected
    · exact herased constraint herasedMem

theorem source_feasible_iff_base_and_selected {source : Instance n}
    (plan : Plan source) (state : State n) :
    source.Feasible state ↔
      plan.BaseFeasible state ∧ plan.constraint.Holds state := by
  unfold Instance.Feasible BaseFeasible
  rw [plan.allIndicators_iff_erased_and_selected state]
  aesop

def target {source : Instance n} (plan : Plan source) : Instance n where
  domains := source.domains
  constraints := source.constraints ++ plan.generatedConstraints
  oneHotConstraints := source.oneHotConstraints
  sos1Constraints := source.sos1Constraints
  indicatorConstraints :=
    source.indicatorConstraints.eraseIdx plan.constraintIndex.val
  objective := source.objective
  sense := source.sense

theorem target_feasible_iff_base_and_generated {source : Instance n}
    (plan : Plan source) (state : State n) :
    plan.target.Feasible state ↔
      plan.BaseFeasible state ∧
        ∀ constraint ∈ plan.generatedConstraints,
          constraint.Holds state := by
  simp only [Instance.Feasible, target, BaseFeasible,
    List.forall_mem_append]
  aesop

/-- The target and source have the same feasible states. -/
theorem target_feasible_iff_source_feasible {source : Instance n}
    (plan : Plan source) (hvalid : plan.Valid) (state : State n) :
    plan.target.Feasible state ↔ source.Feasible state := by
  rw [plan.target_feasible_iff_base_and_generated,
    plan.source_feasible_iff_base_and_selected]
  apply and_congr_right
  intro hbase
  exact plan.generatedConstraints_hold_iff hvalid hbase.1

/-- Indicator Big-M lowering packaged as an `Instance.Transform`.

No state component is added or removed, so encoding and decoding are both
total identity maps. -/
def lowering {source : Instance n} (plan : Plan source) :
    Instance.Transform source where
  targetDimension := n
  target := plan.target
  encode := some
  decode := some

theorem lowering_isReduction {source : Instance n} (plan : Plan source)
    (hvalid : plan.Valid) :
    plan.lowering.IsReduction := by
  intro targetState htarget
  exact ⟨targetState, rfl,
    (plan.target_feasible_iff_source_feasible hvalid targetState).mp htarget⟩

theorem lowering_isRelaxation {source : Instance n} (plan : Plan source)
    (hvalid : plan.Valid) :
    plan.lowering.IsRelaxation := by
  intro sourceState hsource
  exact ⟨sourceState, rfl,
    (plan.target_feasible_iff_source_feasible hvalid sourceState).mpr hsource⟩

theorem lowering_sensePreserving {source : Instance n}
    (plan : Plan source) :
    plan.lowering.SensePreserving :=
  rfl

theorem lowering_sourceObjectiveValuePreserving {source : Instance n}
    (plan : Plan source) :
    plan.lowering.SourceObjectiveValuePreserving := by
  intro sourceState _
  rfl

theorem lowering_targetObjectiveValuePreserving {source : Instance n}
    (plan : Plan source) :
    plan.lowering.TargetObjectiveValuePreserving := by
  intro targetState _
  rfl

theorem lowering_sourceObjectivePreserving {source : Instance n}
    (plan : Plan source) :
    plan.lowering.SourceObjectivePreserving :=
  ⟨plan.lowering_sensePreserving,
    plan.lowering_sourceObjectiveValuePreserving⟩

theorem lowering_targetObjectivePreserving {source : Instance n}
    (plan : Plan source) :
    plan.lowering.TargetObjectivePreserving :=
  ⟨plan.lowering_sensePreserving,
    plan.lowering_targetObjectiveValuePreserving⟩

theorem lowering_sourceRoundTrip {source : Instance n}
    (plan : Plan source) :
    plan.lowering.SourceRoundTrip := by
  intro sourceState _
  rfl

theorem lowering_targetRoundTrip {source : Instance n}
    (plan : Plan source) :
    plan.lowering.TargetRoundTrip := by
  intro targetState _
  rfl

end Plan

end IndicatorBigM

end Instance

end OMMXProof
