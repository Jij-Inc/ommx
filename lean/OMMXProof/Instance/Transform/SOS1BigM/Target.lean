import OMMXProof.Instance.Extend
import OMMXProof.Instance.Transform.SOS1BigM.Plan
import Mathlib.Tactic

/-!
# Target construction for SOS1 Big-M lowering

This module builds the generated linear constraints and target `Instance`, then
relates target feasibility to the reused/fresh selector formulation.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

namespace Plan

/-- A virtual selector tuple indexed by every SOS1 member. At a reused member
its value is ignored by `plannedSelector`; choosing the canonical value makes
the encode lemma exact. -/
def freshSelectorState {source : Instance n} (plan : Plan source)
    (members : State n) (fresh : State plan.freshCount) :
    plan.Member → Rat :=
  fun i =>
    if hi : i ∈ plan.freshMembers then
      fresh (plan.freshIndex i hi)
    else
      canonicalSelector (plan.memberState members) i

namespace Validated

def upperLink {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (j : Fin plan.freshCount) :
    LinearConstraint (n + plan.freshCount) where
  expr := Affine.sub
    (Affine.coordinate
      (Fin.castAdd plan.freshCount (plan.freshMember j).val))
    (Affine.scale (validated.bounds.upper (plan.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
  sense := .lessEqual

def lowerLink {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (j : Fin plan.freshCount) :
    LinearConstraint (n + plan.freshCount) where
  expr := Affine.sub
    (Affine.scale (validated.bounds.lower (plan.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
    (Affine.coordinate
      (Fin.castAdd plan.freshCount (plan.freshMember j).val))
  sense := .lessEqual

@[simp]
theorem upperLink_holds {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (j : Fin plan.freshCount) (state : State n)
    (selectors : State plan.freshCount) :
    (validated.upperLink j).Holds (State.append state selectors) ↔
      state (plan.freshMember j).val ≤
        validated.bounds.upper (plan.freshMember j) * selectors j := by
  simp [upperLink, LinearConstraint.Holds, State.append]

@[simp]
theorem lowerLink_holds {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (j : Fin plan.freshCount) (state : State n)
    (selectors : State plan.freshCount) :
    (validated.lowerLink j).Holds (State.append state selectors) ↔
      validated.bounds.lower (plan.freshMember j) * selectors j ≤
        state (plan.freshMember j).val := by
  simp [lowerLink, LinearConstraint.Holds, State.append]

def linksFor {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (j : Fin plan.freshCount) :
    List (LinearConstraint (n + plan.freshCount)) :=
  (if 0 < validated.bounds.upper (plan.freshMember j) then
      [validated.upperLink j]
    else []) ++
    if validated.bounds.lower (plan.freshMember j) < 0 then
      [validated.lowerLink j]
    else []

def linkConstraints {source : Instance n} {plan : Plan source} (validated : plan.Validated) :
    List (LinearConstraint (n + plan.freshCount)) :=
  (List.ofFn fun j => validated.linksFor j).flatten

theorem linkConstraints_hold_iff {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (state : State n) (selectors : State plan.freshCount) :
    (∀ constraint ∈ validated.linkConstraints,
      constraint.Holds (State.append state selectors)) ↔
      ∀ j,
        OptionalUpperLink (validated.bounds.upper (plan.freshMember j))
            (state (plan.freshMember j).val) (selectors j) ∧
          OptionalLowerLink (validated.bounds.lower (plan.freshMember j))
            (state (plan.freshMember j).val) (selectors j) := by
  simp only [linkConstraints, List.forall_mem_flatten,
    List.forall_mem_ofFn_iff]
  constructor
  · intro h j
    have hj := h j
    by_cases hu : 0 < validated.bounds.upper (plan.freshMember j)
    <;> by_cases hl : validated.bounds.lower (plan.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj
  · intro h j
    have hj := h j
    by_cases hu : 0 < validated.bounds.upper (plan.freshMember j)
    <;> by_cases hl : validated.bounds.lower (plan.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj

end Validated

/-- Coefficients of the mixed cardinality row: reused binary members stay in
the source block and every fresh selector contributes in the right block. -/
def cardinalityExpr {source : Instance n} (plan : Plan source) :
    Affine (n + plan.freshCount) where
  coeff := Fin.append
    (fun i =>
      if i ∈ plan.constraint.members ∧ source.domains i = .binary then 1 else 0)
    (fun _ => 1)
  constant := -1

def cardinalityConstraint {source : Instance n} (plan : Plan source) :
    LinearConstraint (n + plan.freshCount) where
  expr := plan.cardinalityExpr
  sense := .lessEqual

def selectorSum {source : Instance n} (plan : Plan source)
    (state : State n) (selectors : State plan.freshCount) : Rat :=
  (∑ i ∈ plan.constraint.members,
      if source.domains i = .binary then state i else 0) +
    ∑ j, selectors j

def reusedContribution {source : Instance n} (plan : Plan source)
    (state : State n) (i : plan.Member) : Rat :=
  if i ∈ plan.reusedMembers then plan.memberState state i else 0

def freshContribution {source : Instance n} (plan : Plan source)
    (state : State n) (selectors : State plan.freshCount)
    (i : plan.Member) : Rat :=
  if i ∈ plan.freshMembers then plan.freshSelectorState state selectors i else 0

theorem plannedSelector_eq_contributions {source : Instance n}
    (plan : Plan source) (state : State n)
    (selectors : State plan.freshCount) (i : plan.Member) :
    plannedSelector plan.reusedMembers (plan.memberState state)
        (plan.freshSelectorState state selectors) i =
      plan.reusedContribution state i +
        plan.freshContribution state selectors i := by
  by_cases hr : i ∈ plan.reusedMembers
  · have hf : i ∉ plan.freshMembers := by
      simp [freshMembers, hr]
    simp [plannedSelector, reusedContribution, freshContribution, hr, hf]
  · have hf : i ∈ plan.freshMembers := by
      simp [freshMembers, hr]
    simp [plannedSelector, reusedContribution, freshContribution, hr, hf]

theorem sourceReusedSum_eq {source : Instance n} (plan : Plan source)
    (state : State n) :
    (∑ i ∈ plan.constraint.members,
      if source.domains i = .binary then state i else 0) =
      ∑ i : plan.Member, plan.reusedContribution state i := by
  symm
  calc
    (∑ i : plan.Member, plan.reusedContribution state i) =
        ∑ i : plan.Member,
          if source.domains i = .binary then state i else 0 := by
      apply Finset.sum_congr rfl
      intro i _
      simp [reusedContribution, reusedMembers, memberState]
    _ = ∑ i ∈ plan.constraint.members,
          if source.domains i = .binary then state i else 0 := by
      exact Finset.sum_coe_sort plan.constraint.members
        (fun i => if source.domains i = .binary then state i else 0)

theorem freshSelectorSum_eq {source : Instance n} (plan : Plan source)
    (state : State n) (selectors : State plan.freshCount) :
    (∑ j, selectors j) =
      ∑ i : plan.Member, plan.freshContribution state selectors i := by
  calc
    (∑ j, selectors j) =
        ∑ i : {i // i ∈ plan.freshMembers},
          plan.freshSelectorState state selectors i.val := by
      apply Fintype.sum_equiv
        (plan.freshMembers.orderIsoOfFin rfl).toEquiv
      intro j
      unfold freshSelectorState
      have hi :
          ((plan.freshMembers.orderIsoOfFin rfl).toEquiv j).val ∈
            plan.freshMembers :=
        ((plan.freshMembers.orderIsoOfFin rfl).toEquiv j).property
      rw [dif_pos hi]
      change selectors j =
        selectors ((plan.freshMembers.orderIsoOfFin rfl).symm
          ((plan.freshMembers.orderIsoOfFin rfl) j))
      exact congrArg selectors
        ((plan.freshMembers.orderIsoOfFin rfl).symm_apply_apply j).symm
    _ = ∑ i ∈ plan.freshMembers,
          plan.freshSelectorState state selectors i := by
      exact Finset.sum_coe_sort plan.freshMembers
        (plan.freshSelectorState state selectors)
    _ = ∑ i : plan.Member, plan.freshContribution state selectors i := by
      symm
      simpa [freshContribution] using
        (Finset.sum_ite_mem_eq plan.freshMembers
          (plan.freshSelectorState state selectors))

theorem selectorSum_eq_plannedSelector {source : Instance n}
    (plan : Plan source) (state : State n)
    (selectors : State plan.freshCount) :
    plan.selectorSum state selectors =
      ∑ i, plannedSelector plan.reusedMembers (plan.memberState state)
        (plan.freshSelectorState state selectors) i := by
  rw [selectorSum, plan.sourceReusedSum_eq state,
    plan.freshSelectorSum_eq state selectors]
  rw [← Finset.sum_add_distrib]
  apply Finset.sum_congr rfl
  intro i _
  exact (plan.plannedSelector_eq_contributions state selectors i).symm

@[simp]
theorem cardinalityConstraint_holds {source : Instance n} (plan : Plan source)
    (state : State n) (selectors : State plan.freshCount) :
    plan.cardinalityConstraint.Holds (State.append state selectors) ↔
      plan.selectorSum state selectors ≤ 1 := by
  have hsum :
      (∑ i : Fin n,
        if i ∈ plan.constraint.members ∧ source.domains i = .binary then
          state i
        else 0) =
        ∑ i ∈ plan.constraint.members,
          if source.domains i = .binary then state i else 0 := by
    convert Finset.sum_ite_mem_eq plan.constraint.members
      (fun i => if source.domains i = .binary then state i else 0) using 1
    simp [ite_and]
  simp [cardinalityConstraint, cardinalityExpr, selectorSum,
    LinearConstraint.Holds, Affine.eval, State.append, Fin.sum_univ_add]
  rw [hsum]

namespace Validated

def generatedConstraints {source : Instance n} {plan : Plan source} (validated : plan.Validated) :
    List (LinearConstraint (n + plan.freshCount)) :=
  validated.linkConstraints ++ [plan.cardinalityConstraint]

end Validated

theorem plannedSelector_binary_of_domains {source : Instance n}
    (plan : Plan source) (state : State n)
    (selectors : State plan.freshCount)
    (hsourceDomains : ∀ i, state i ∈ source.domains i)
    (hselectorDomains : ∀ j, selectors j ∈ Domain.binary) :
    GenericBinaryOn Finset.univ
      (plannedSelector plan.reusedMembers (plan.memberState state)
        (plan.freshSelectorState state selectors)) := by
  intro i _
  by_cases hr : i ∈ plan.reusedMembers
  · have hdomain : source.domains i = .binary :=
      (Finset.mem_filter.mp hr).2
    simpa [plannedSelector, hr, memberState, hdomain] using
      hsourceDomains i
  · have hfresh : i ∈ plan.freshMembers := by
      simp [freshMembers, hr]
    simpa [plannedSelector, hr, freshSelectorState, hfresh] using
      hselectorDomains (plan.freshIndex i hfresh)

namespace Validated

theorem linksForFresh_iff_linksForMembers {source : Instance n}
    {plan : Plan source} (validated : plan.Validated) (state : State n)
    (selectors : State plan.freshCount) :
    (∀ j,
      OptionalUpperLink (validated.bounds.upper (plan.freshMember j))
          (state (plan.freshMember j).val) (selectors j) ∧
        OptionalLowerLink (validated.bounds.lower (plan.freshMember j))
          (state (plan.freshMember j).val) (selectors j)) ↔
      ∀ i, i ∉ plan.reusedMembers →
        OptionalUpperLink (validated.bounds.upper i)
            (plan.memberState state i)
            (plan.freshSelectorState state selectors i) ∧
          OptionalLowerLink (validated.bounds.lower i)
            (plan.memberState state i)
            (plan.freshSelectorState state selectors i) := by
  constructor
  · intro h i hr
    have hfresh : i ∈ plan.freshMembers := by
      simp [freshMembers, hr]
    have hi := h (plan.freshIndex i hfresh)
    simpa [memberState, freshSelectorState, hfresh] using hi
  · intro h j
    have hfresh : plan.freshMember j ∈ plan.freshMembers :=
      (plan.freshMembers.orderIsoOfFin rfl j).property
    have hr : plan.freshMember j ∉ plan.reusedMembers := by
      simpa [freshMembers] using hfresh
    have hj := h (plan.freshMember j) hr
    simpa [memberState, freshSelectorState, hfresh] using hj

/-- The generated linear rows denote exactly the SDK reused/fresh selector
formulation, provided the old and fresh blocks satisfy their declared domains. -/
theorem generatedConstraints_hold_iff_plannedSelectorFormulation
    {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (state : State n) (selectors : State plan.freshCount)
    (hsourceDomains : ∀ i, state i ∈ source.domains i)
    (hselectorDomains : ∀ j, selectors j ∈ Domain.binary) :
    (∀ constraint ∈ validated.generatedConstraints,
      constraint.Holds (State.append state selectors)) ↔
      PlannedSelectorFormulation plan.reusedMembers validated.bounds
        (plan.memberState state)
        (plan.freshSelectorState state selectors) := by
  rw [generatedConstraints, List.forall_mem_append,
    List.forall_mem_singleton, validated.linkConstraints_hold_iff,
    plan.cardinalityConstraint_holds,
    validated.linksForFresh_iff_linksForMembers]
  constructor
  · rintro ⟨hlinks, hcardinality⟩
    refine ⟨plan.plannedSelector_binary_of_domains state selectors
      hsourceDomains hselectorDomains, hlinks, ?_⟩
    rw [← plan.selectorSum_eq_plannedSelector state selectors]
    exact hcardinality
  · rintro ⟨_, hlinks, hcardinality⟩
    refine ⟨hlinks, ?_⟩
    rw [plan.selectorSum_eq_plannedSelector state selectors]
    exact hcardinality

theorem freshSelectors_binary_of_plannedSelectorFormulation
    {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    (state : State n) (selectors : State plan.freshCount)
    (hformulation : PlannedSelectorFormulation plan.reusedMembers validated.bounds
      (plan.memberState state)
      (plan.freshSelectorState state selectors)) :
    ∀ j, selectors j ∈ Domain.binary := by
  intro j
  have hfresh : plan.freshMember j ∈ plan.freshMembers :=
    (plan.freshMembers.orderIsoOfFin rfl j).property
  have hr : plan.freshMember j ∉ plan.reusedMembers := by
    simpa [freshMembers] using hfresh
  have hbinary := hformulation.1 (plan.freshMember j) (by simp)
  simpa [plannedSelector, hr, freshSelectorState, hfresh] using hbinary

theorem selectedHolds_of_plannedSelectorFormulation
    {source : Instance n} {plan : Plan source} (validated : plan.Validated)
    {state : State n}
    {selectors : State plan.freshCount}
    (hdomains : ∀ i, state i ∈ source.domains i)
    (hformulation : PlannedSelectorFormulation plan.reusedMembers validated.bounds
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

def target {source : Instance n} {plan : Plan source} (validated : plan.Validated) :
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
        PlannedSelectorFormulation plan.reusedMembers validated.bounds
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
        PlannedSelectorFormulation plan.reusedMembers validated.bounds
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
