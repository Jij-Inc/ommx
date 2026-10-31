import OMMXProof.Instance.Extend
import OMMXProof.Instance.Transform.SOS1BigM.Lowering.Plan
import Mathlib.Tactic

/-!
# Target construction for SOS1 Big-M lowering

This module instantiates the shared canonical Big-M rows for a validated
lowering plan, builds the target `Instance`, and relates target feasibility to
the reused/fresh selector formulation.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

namespace Plan

/-- The shared virtual selector tuple exposed through the lowering plan API. -/
def freshSelectorState {source : Instance n} (plan : Plan source)
    (members : State n) (fresh : State plan.freshCount) :
    plan.Member → Rat :=
  plan.selectorLayout.freshSelectorState members fresh

namespace Validated

/-- The canonical link and cardinality rows instantiated with validated bounds. -/
def generatedConstraints {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) :
    List (LinearConstraint (n + plan.freshCount)) :=
  plan.selectorLayout.canonicalRows validated.bounds

/-- The generated rows denote exactly the shared selector formulation whenever
the retained reused members and fresh selectors satisfy their binary domains. -/
theorem generatedConstraints_hold_iff_plannedSelectorFormulation
    {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (state : State n) (selectors : State plan.freshCount)
    (hsourceDomains : ∀ i, state i ∈ source.domains i)
    (hselectorDomains : ∀ j, selectors j ∈ Domain.binary) :
    (∀ constraint ∈ validated.generatedConstraints,
      constraint.Holds (State.append state selectors)) ↔
      PlannedSelectorFormulationHolds plan.reusedMembers validated.bounds
        (plan.memberState state)
        (plan.freshSelectorState state selectors) := by
  exact
    plan.selectorLayout.canonicalRows_hold_iff_plannedSelectorFormulation
      validated.bounds state selectors
      (plan.reusedBinary_of_domains hsourceDomains) hselectorDomains

theorem freshSelectors_binary_of_plannedSelectorFormulation
    {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (state : State n) (selectors : State plan.freshCount)
    (hformulation :
      PlannedSelectorFormulationHolds plan.reusedMembers validated.bounds
        (plan.memberState state)
        (plan.freshSelectorState state selectors)) :
    ∀ j, selectors j ∈ Domain.binary := by
  exact
    plan.selectorLayout.freshSelectors_binary_of_plannedSelectorFormulation
      validated.bounds state selectors hformulation

theorem selectedHolds_of_plannedSelectorFormulation
    {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    {state : State n}
    {selectors : State plan.freshCount}
    (hdomains : ∀ i, state i ∈ source.domains i)
    (hformulation :
      PlannedSelectorFormulationHolds plan.reusedMembers validated.bounds
        (plan.memberState state)
        (plan.freshSelectorState state selectors)) :
    plan.constraint.Holds state := by
  apply (plan.genericSOS1_memberState_iff_holds state).mp
  exact plannedSelectorFormulation_project_sos1
    plan.reusedMembers validated.bounds
    (plan.memberState state)
    (plan.freshSelectorState state selectors)
    (validated.withinBounds_of_domains hdomains) hformulation

end Validated

def BaseFeasible {source : Instance n} (plan : Plan source)
    (state : State n) : Prop :=
  (∀ i, state i ∈ source.domains i) ∧
    (∀ constraint ∈ source.constraints, constraint.Holds state) ∧
    (∀ constraint ∈ source.oneHotConstraints, constraint.Holds state) ∧
    (∀ constraint ∈ source.sos1Constraints.eraseIdx plan.constraintIndex.val,
      constraint.Holds state) ∧
    ∀ constraint ∈ source.indicatorConstraints, constraint.Holds state

theorem allSOS1_iff_erased_and_selected {source : Instance n}
    (plan : Plan source) (state : State n) :
    (∀ constraint ∈ source.sos1Constraints, constraint.Holds state) ↔
      (∀ constraint ∈
        source.sos1Constraints.eraseIdx plan.constraintIndex.val,
        constraint.Holds state) ∧
        plan.constraint.Holds state := by
  constructor
  · intro hall
    constructor
    · intro constraint hconstraint
      exact hall constraint (List.mem_of_mem_eraseIdx hconstraint)
    · exact hall plan.constraint (List.get_mem _ plan.constraintIndex)
  · rintro ⟨herased, hselected⟩ constraint hconstraint
    have hperm := List.getElem_cons_eraseIdx_perm
      (l := source.sos1Constraints) plan.constraintIndex.isLt
    have hleft :
        constraint ∈
          plan.constraint ::
            source.sos1Constraints.eraseIdx plan.constraintIndex.val := by
      exact hperm.symm.subset hconstraint
    rcases List.mem_cons.mp hleft with hsame | herasedMem
    · simpa [hsame] using hselected
    · exact herased constraint herasedMem

theorem source_feasible_iff_base_and_selected {source : Instance n}
    (plan : Plan source) (state : State n) :
    source.Feasible state ↔
      plan.BaseFeasible state ∧ plan.constraint.Holds state := by
  unfold Instance.Feasible BaseFeasible
  rw [plan.allSOS1_iff_erased_and_selected state]
  aesop

namespace Validated

def target {source : Instance n} {plan : Plan source}
    (validated : plan.Validated) :
    Instance (n + plan.freshCount) where
  domains := Domain.append source.domains (fun _ => .binary)
  constraints :=
    (source.constraints.map fun constraint =>
      constraint.extend plan.freshCount) ++ validated.generatedConstraints
  oneHotConstraints :=
    source.oneHotConstraints.map fun constraint =>
      constraint.castAdd plan.freshCount
  sos1Constraints :=
    (source.sos1Constraints.eraseIdx plan.constraintIndex.val).map
      fun constraint => constraint.castAdd plan.freshCount
  indicatorConstraints :=
    source.indicatorConstraints.map fun constraint =>
      constraint.extend plan.freshCount
  objective := source.objective.extend plan.freshCount
  sense := source.sense

theorem target_feasible_append_iff_base_and_formulation
    {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (state : State n) (selectors : State plan.freshCount) :
    validated.target.Feasible (State.append state selectors) ↔
      plan.BaseFeasible state ∧
        PlannedSelectorFormulationHolds plan.reusedMembers validated.bounds
          (plan.memberState state)
          (plan.freshSelectorState state selectors) := by
  have hsourceAt (i : Fin n) :
      State.append state selectors (Fin.castAdd plan.freshCount i) = state i := by
    simp [State.append]
  have hfreshAt (j : Fin plan.freshCount) :
      State.append state selectors (Fin.natAdd n j) = selectors j := by
    simp [State.append]
  simp only [Instance.Feasible, target, Domain.append,
    Fin.forall_fin_add, hsourceAt, hfreshAt, Fin.append_left,
    Fin.append_right, List.forall_mem_append, List.forall_mem_map,
    LinearConstraint.holds_extend_append,
    OneHotConstraint.holds_castAdd_append, SOS1Constraint.holds_castAdd_append,
    IndicatorConstraint.holds_extend_append, BaseFeasible]
  constructor
  · rintro ⟨⟨hdomains, hselectors⟩,
      ⟨holdConstraints, hgenerated⟩, honeHot, hsos1, hindicator⟩
    have hformulation :=
      (validated.generatedConstraints_hold_iff_plannedSelectorFormulation
        state selectors hdomains hselectors).mp hgenerated
    exact ⟨⟨hdomains, holdConstraints, honeHot, hsos1, hindicator⟩,
      hformulation⟩
  · rintro ⟨⟨hdomains, holdConstraints, honeHot, hsos1, hindicator⟩,
      hformulation⟩
    have hselectors :=
      validated.freshSelectors_binary_of_plannedSelectorFormulation
        state selectors hformulation
    have hgenerated :=
      (validated.generatedConstraints_hold_iff_plannedSelectorFormulation
        state selectors hdomains hselectors).mpr hformulation
    exact ⟨⟨hdomains, hselectors⟩,
      ⟨holdConstraints, hgenerated⟩, honeHot, hsos1, hindicator⟩

theorem target_feasible_iff_base_and_formulation
    {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (state : State (n + plan.freshCount)) :
    validated.target.Feasible state ↔
      plan.BaseFeasible (State.source state) ∧
        PlannedSelectorFormulationHolds plan.reusedMembers validated.bounds
          (plan.memberState (State.source state))
          (plan.freshSelectorState (State.source state)
            (State.extendedPart state)) := by
  simpa only [State.append_source_extendedPart] using
    validated.target_feasible_append_iff_base_and_formulation
      (State.source state) (State.extendedPart state)

end Validated

end Plan

end SOS1BigM

end Instance

end OMMXProof
