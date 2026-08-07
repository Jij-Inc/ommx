import OMMXProof.Instance.Transform.IndicatorBigM.Plan
import Mathlib.Tactic

/-!
# Target construction for Indicator Big-M lowering

This module builds the generated linear constraints and target `Instance`, then
proves that they have exactly the selected Indicator's denotation on states
satisfying the source domains.
-/

namespace OMMXProof

namespace Instance

namespace IndicatorBigM

/-- A constant affine expression used to spell the generated rows. -/
private def constantExpr (value : Rat) : Affine n where
  coeff := fun _ => 0
  constant := value

@[simp]
private theorem eval_constantExpr (value : Rat) (state : State n) :
    (constantExpr value).eval state = value := by
  simp [constantExpr, Affine.eval]

namespace Plan

namespace Validated

/-- The generated upper row `f(x) + u y - u ≤ 0`. -/
def upperConstraint {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) : LinearConstraint n where
  expr := Affine.sub
    (Affine.add plan.constraint.body.expr
      (Affine.scale validated.upper
        (Affine.coordinate plan.constraint.trigger)))
    (constantExpr validated.upper)
  sense := .lessEqual

/-- The generated lower row `-f(x) - l y + l ≤ 0`. -/
def lowerConstraint {source : Instance n} {plan : Plan source}
    (validated : plan.Validated)
    (hsense : plan.constraint.body.sense = .equal) : LinearConstraint n where
  expr := Affine.add
    (Affine.sub (Affine.neg plan.constraint.body.expr)
      (Affine.scale (validated.lower hsense)
        (Affine.coordinate plan.constraint.trigger)))
    (constantExpr (validated.lower hsense))
  sense := .lessEqual

@[simp]
theorem upperConstraint_holds {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) (state : State n) :
    validated.upperConstraint.Holds state ↔
      plan.bodyValue state +
          validated.upper * state plan.constraint.trigger -
            validated.upper ≤ 0 := by
  simp [upperConstraint, Plan.bodyValue, LinearConstraint.Holds]

@[simp]
theorem lowerConstraint_holds {source : Instance n} {plan : Plan source}
    (validated : plan.Validated)
    (hsense : plan.constraint.body.sense = .equal) (state : State n) :
    (validated.lowerConstraint hsense).Holds state ↔
      -plan.bodyValue state -
          validated.lower hsense * state plan.constraint.trigger +
            validated.lower hsense ≤ 0 := by
  simp [lowerConstraint, Plan.bodyValue, LinearConstraint.Holds]

def upperConstraints {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) : List (LinearConstraint n) :=
  if 0 < validated.upper then [validated.upperConstraint] else []

def lowerConstraints {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) : List (LinearConstraint n) :=
  if hsense : plan.constraint.body.sense = .equal then
    if validated.lower hsense < 0 then
      [validated.lowerConstraint hsense]
    else
      []
  else
    []

/-- Generated regular constraints, in SDK insertion order. -/
def generatedConstraints {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) : List (LinearConstraint n) :=
  validated.upperConstraints ++ validated.lowerConstraints

theorem upperConstraints_hold_iff {source : Instance n}
    {plan : Plan source} (validated : plan.Validated) (state : State n) :
    (∀ constraint ∈ validated.upperConstraints, constraint.Holds state) ↔
      UpperSide plan.bodyValue plan.constraint.trigger
        validated.upper state := by
  by_cases hemitted : 0 < validated.upper <;>
    simp [upperConstraints, UpperSide, hemitted]

theorem lowerConstraints_hold_iff {source : Instance n}
    {plan : Plan source} (validated : plan.Validated) (state : State n)
    (hsense : plan.constraint.body.sense = .equal) :
    (∀ constraint ∈ validated.lowerConstraints, constraint.Holds state) ↔
      LowerSide plan.bodyValue plan.constraint.trigger
        (validated.lower hsense) state := by
  by_cases hemitted : validated.lower hsense < 0 <;>
    simp [lowerConstraints, LowerSide, hsense, hemitted]

/-- The generated rows have exactly the selected Indicator's denotation on
states satisfying the source domains. -/
theorem generatedConstraints_hold_iff {source : Instance n}
    {plan : Plan source} (validated : plan.Validated) {state : State n}
    (hdomains : ∀ i, state i ∈ source.domains i) :
    (∀ constraint ∈ validated.generatedConstraints,
      constraint.Holds state) ↔
        plan.constraint.Holds state := by
  have htriggerDomain := validated.valid.triggerBinary
  have hpolarity := validated.valid.polarityActiveOnOne
  have hbinary : state plan.constraint.trigger ∈ Domain.binary := by
    have htrigger := hdomains plan.constraint.trigger
    rw [htriggerDomain] at htrigger
    exact htrigger
  have hbodyInBound : plan.bodyValue state ∈ plan.bodyBound :=
    plan.bodyValue_mem_bodyBound hdomains
  have hupperBound : plan.bodyValue state ≤ validated.upper :=
    Bound.le_finite_upper hbodyInBound validated.upper_exact
  cases hsense : plan.constraint.body.sense with
  | lessEqual =>
      have hgenerated :
          (∀ constraint ∈ validated.generatedConstraints,
            constraint.Holds state) ↔
            UpperSide plan.bodyValue plan.constraint.trigger
              validated.upper state := by
        rw [generatedConstraints, List.forall_mem_append,
          validated.upperConstraints_hold_iff state]
        simp [lowerConstraints, hsense]
      exact hgenerated.trans (by
        simpa [IndicatorConstraint.Holds, IndicatorPredicate, hpolarity,
          LinearConstraint.Holds, hsense, Plan.bodyValue] using
            (upperSide_iff_indicator hbinary hupperBound))
  | equal =>
      have hlowerBound :
          validated.lower hsense ≤ plan.bodyValue state :=
        Bound.finite_lower_le hbodyInBound
          (validated.lower_exact hsense)
      have hgenerated :
          (∀ constraint ∈ validated.generatedConstraints,
            constraint.Holds state) ↔
            UpperSide plan.bodyValue plan.constraint.trigger
                validated.upper state ∧
              LowerSide plan.bodyValue plan.constraint.trigger
                (validated.lower hsense) state := by
        rw [generatedConstraints, List.forall_mem_append,
          validated.upperConstraints_hold_iff state,
          validated.lowerConstraints_hold_iff state hsense]
      exact hgenerated.trans (by
        simpa [IndicatorConstraint.Holds, IndicatorPredicate, hpolarity,
          LinearConstraint.Holds, hsense, Plan.bodyValue] using
            (equalitySides_iff_indicator hbinary
              hlowerBound hupperBound))

end Validated

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

namespace Validated

def target {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) : Instance n where
  domains := source.domains
  constraints := source.constraints ++ validated.generatedConstraints
  oneHotConstraints := source.oneHotConstraints
  sos1Constraints := source.sos1Constraints
  indicatorConstraints :=
    source.indicatorConstraints.eraseIdx plan.constraintIndex.val
  objective := source.objective
  sense := source.sense

theorem target_feasible_iff_base_and_generated {source : Instance n}
    {plan : Plan source} (validated : plan.Validated) (state : State n) :
    validated.target.Feasible state ↔
      plan.BaseFeasible state ∧
        ∀ constraint ∈ validated.generatedConstraints,
          constraint.Holds state := by
  simp only [Instance.Feasible, target, BaseFeasible,
    List.forall_mem_append]
  aesop

/-- The target and source have the same feasible states. -/
theorem target_feasible_iff_source_feasible {source : Instance n}
    {plan : Plan source} (validated : plan.Validated) (state : State n) :
    validated.target.Feasible state ↔ source.Feasible state := by
  rw [validated.target_feasible_iff_base_and_generated,
    plan.source_feasible_iff_base_and_selected]
  apply and_congr_right
  intro hbase
  exact validated.generatedConstraints_hold_iff hbase.1

end Validated

end Plan

end IndicatorBigM

end Instance

end OMMXProof
