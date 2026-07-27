import OMMXProof.Instance.Transform
import Mathlib.Tactic

/-!
# Indicator promotion

This module replaces one ordinary linear-constraint occurrence with an
Indicator constraint over the same state space. The Indicator body is obtained
by substituting the trigger's active value into the selected ordinary
constraint.

Exact replacement additionally requires the selected constraint to follow from
the surviving Instance when the trigger is inactive. `Witness.Valid` receives
that implication as an external proof; discovering it belongs to the Presolve
or Branch-and-Bound process which constructs the witness.
-/

namespace OMMXProof

namespace Instance

namespace IndicatorPromotion

private theorem replace
    (base source consequent : State n → Prop)
    (trigger : Fin n) (polarity : IndicatorPolarity)
    (activeExact : ∀ {state},
      base state →
      state trigger ∈ Domain.binary →
      polarity.Active (state trigger) →
      (source state ↔ consequent state))
    (inactiveSource : ∀ {state},
      base state →
      state trigger ∈ Domain.binary →
      state trigger = polarity.inactiveValue →
      source state) :
    ∀ state,
      base state →
      state trigger ∈ Domain.binary →
      (source state ↔
        IndicatorPredicate trigger polarity consequent state) := by
  intro state hbase hbinary
  rcases IndicatorPolarity.active_or_inactive_of_binary hbinary with
    hactive | hinactive
  · constructor
    · intro hsource _
      exact (activeExact hbase hbinary hactive).mp hsource
    · intro hindicator
      exact (activeExact hbase hbinary hactive).mpr (hindicator hactive)
  · have hnotActive : ¬polarity.Active (state trigger) := by
      cases polarity <;> simp_all [IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, IndicatorPolarity.inactiveValue]
    constructor
    · intro _ hactive
      exact False.elim (hnotActive hactive)
    · intro _
      exact inactiveSource hbase hbinary hinactive

/-- Data selecting one ordinary constraint occurrence and its Indicator
trigger.

The promoted Indicator body is derived from the selected constraint rather
than stored independently. -/
structure Witness (source : Instance n) where
  constraintIndex : Fin source.constraints.length
  trigger : Fin n
  polarity : IndicatorPolarity
  deriving DecidableEq

namespace Witness

def constraint {source : Instance n} (witness : Witness source) :
    LinearConstraint n :=
  source.constraints.get witness.constraintIndex

def indicator {source : Instance n} (witness : Witness source) :
    IndicatorConstraint n where
  trigger := witness.trigger
  polarity := witness.polarity
  body := witness.constraint.substitute witness.trigger
    witness.polarity.activeValue

/-- Feasibility of every source component except the selected ordinary
constraint. This is the context allowed when proving that the selected
constraint is redundant on the inactive branch. -/
def BaseFeasible {source : Instance n} (witness : Witness source)
    (state : State n) : Prop :=
  (∀ i, state i ∈ source.domains i) ∧
    (∀ constraint ∈
      source.constraints.eraseIdx witness.constraintIndex.val,
      constraint.Holds state) ∧
    (∀ constraint ∈ source.oneHotConstraints, constraint.Holds state) ∧
    (∀ constraint ∈ source.sos1Constraints, constraint.Holds state) ∧
    ∀ constraint ∈ source.indicatorConstraints, constraint.Holds state

/-- Exactness conditions for Indicator promotion.

The trigger must be binary. The second conjunct is supplied by the process
which discovered the promotion: after removing the selected constraint, the
surviving Instance must still imply it whenever the trigger is inactive. -/
def Valid {source : Instance n} (witness : Witness source) : Prop :=
  source.domains witness.trigger = .binary ∧
    ∀ {state},
      witness.BaseFeasible state →
      state witness.trigger = witness.polarity.inactiveValue →
      witness.constraint.Holds state

theorem constraint_holds_iff_indicator_holds {source : Instance n}
    (witness : Witness source) (hvalid : witness.Valid)
    (state : State n) (hbase : witness.BaseFeasible state) :
    witness.constraint.Holds state ↔ witness.indicator.Holds state := by
  have hbinary : state witness.trigger ∈ Domain.binary := by
    have htrigger := hbase.1 witness.trigger
    rw [hvalid.1] at htrigger
    exact htrigger
  have hreplacement :
      witness.constraint.Holds state ↔
        IndicatorPredicate witness.trigger witness.polarity
          witness.indicator.body.Holds state := by
    apply replace witness.BaseFeasible
      witness.constraint.Holds witness.indicator.body.Holds
      witness.trigger witness.polarity
    · intro currentState _ _ hactive
      exact (LinearConstraint.substitute_holds_iff
        (show currentState witness.trigger =
          witness.polarity.activeValue from hactive)).symm
    · intro currentState hcurrentBase _ hinactive
      exact hvalid.2 hcurrentBase hinactive
    · exact hbase
    · exact hbinary
  simpa [indicator, IndicatorConstraint.Holds, IndicatorPredicate] using
    hreplacement

theorem allConstraints_iff_erased_and_selected {source : Instance n}
    (witness : Witness source) (state : State n) :
    (∀ constraint ∈ source.constraints, constraint.Holds state) ↔
      (∀ constraint ∈
        source.constraints.eraseIdx witness.constraintIndex.val,
        constraint.Holds state) ∧
        witness.constraint.Holds state := by
  constructor
  · intro hall
    constructor
    · intro constraint hconstraint
      exact hall constraint (List.mem_of_mem_eraseIdx hconstraint)
    · exact hall witness.constraint
        (List.get_mem _ witness.constraintIndex)
  · rintro ⟨herased, hselected⟩ constraint hconstraint
    have hperm := List.getElem_cons_eraseIdx_perm
      (l := source.constraints) witness.constraintIndex.isLt
    have hleft :
        constraint ∈
          witness.constraint ::
            source.constraints.eraseIdx witness.constraintIndex.val := by
      exact hperm.symm.subset hconstraint
    rcases List.mem_cons.mp hleft with hsame | herasedMem
    · simpa [hsame] using hselected
    · exact herased constraint herasedMem

theorem source_feasible_iff_base_and_constraint {source : Instance n}
    (witness : Witness source) (state : State n) :
    source.Feasible state ↔
      witness.BaseFeasible state ∧ witness.constraint.Holds state := by
  unfold Instance.Feasible BaseFeasible
  rw [witness.allConstraints_iff_erased_and_selected state]
  aesop

def target {source : Instance n} (witness : Witness source) : Instance n where
  domains := source.domains
  constraints :=
    source.constraints.eraseIdx witness.constraintIndex.val
  oneHotConstraints := source.oneHotConstraints
  sos1Constraints := source.sos1Constraints
  indicatorConstraints :=
    source.indicatorConstraints ++ [witness.indicator]
  objective := source.objective
  sense := source.sense

theorem target_feasible_iff_base_and_indicator {source : Instance n}
    (witness : Witness source) (state : State n) :
    witness.target.Feasible state ↔
      witness.BaseFeasible state ∧ witness.indicator.Holds state := by
  simp only [Instance.Feasible, target, BaseFeasible,
    List.forall_mem_append]
  aesop

/-- A valid promotion preserves the feasible region exactly. -/
theorem target_feasible_iff_source_feasible {source : Instance n}
    (witness : Witness source) (hvalid : witness.Valid)
    (state : State n) :
    witness.target.Feasible state ↔ source.Feasible state := by
  rw [witness.target_feasible_iff_base_and_indicator,
    witness.source_feasible_iff_base_and_constraint]
  apply and_congr_right
  intro hbase
  exact (witness.constraint_holds_iff_indicator_holds
    hvalid state hbase).symm

/-- Indicator promotion packaged as a same-state `Instance.Transform`. -/
def promotion {source : Instance n} (witness : Witness source) :
    Instance.Transform source where
  targetDimension := n
  target := witness.target
  encode := some
  decode := some

theorem promotion_isReduction {source : Instance n}
    (witness : Witness source) (hvalid : witness.Valid) :
    witness.promotion.IsReduction := by
  intro targetState htarget
  exact ⟨targetState, rfl,
    (witness.target_feasible_iff_source_feasible hvalid targetState).mp
      htarget⟩

theorem promotion_isRelaxation {source : Instance n}
    (witness : Witness source) (hvalid : witness.Valid) :
    witness.promotion.IsRelaxation := by
  intro sourceState hsource
  exact ⟨sourceState, rfl,
    (witness.target_feasible_iff_source_feasible hvalid sourceState).mpr
      hsource⟩

theorem promotion_sensePreserving {source : Instance n}
    (witness : Witness source) :
    witness.promotion.SensePreserving :=
  rfl

theorem promotion_sourceObjectiveValuePreserving {source : Instance n}
    (witness : Witness source) :
    witness.promotion.SourceObjectiveValuePreserving := by
  intro sourceState _
  rfl

theorem promotion_targetObjectiveValuePreserving {source : Instance n}
    (witness : Witness source) :
    witness.promotion.TargetObjectiveValuePreserving := by
  intro targetState _
  rfl

theorem promotion_sourceObjectivePreserving {source : Instance n}
    (witness : Witness source) :
    witness.promotion.SourceObjectivePreserving :=
  ⟨witness.promotion_sensePreserving,
    witness.promotion_sourceObjectiveValuePreserving⟩

theorem promotion_targetObjectivePreserving {source : Instance n}
    (witness : Witness source) :
    witness.promotion.TargetObjectivePreserving :=
  ⟨witness.promotion_sensePreserving,
    witness.promotion_targetObjectiveValuePreserving⟩

theorem promotion_sourceRoundTrip {source : Instance n}
    (witness : Witness source) :
    witness.promotion.SourceRoundTrip := by
  intro sourceState _
  rfl

theorem promotion_targetRoundTrip {source : Instance n}
    (witness : Witness source) :
    witness.promotion.TargetRoundTrip := by
  intro targetState _
  rfl

end Witness

end IndicatorPromotion

end Instance

end OMMXProof
