import OMMXProof.Instance.Extend
import OMMXProof.Instance.Transform
import Mathlib.Tactic

/-!
# SOS1 Big-M lowering as an Instance transformation

This module writes the SDK SOS1 conversion as an actual target `Instance`
together with its state maps. One SOS1 occurrence is selected by list index,
binary members are reused as their own selectors, and one fresh binary
component is appended for every other member. Trivial zero-bound link sides
are omitted.

`Plan` is syntax; `Plan.Valid` is the independently checkable precondition.
Once validity is supplied, the same central denotation theorem yields the
existing `ProjectionPreserves` view and the new `Instance.Transform`
reduction/relaxation/round-trip properties.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

/-- One planned conversion of one occurrence in the source SOS1 list.

Bounds are indexed only by members of the selected SOS1 constraint. -/
structure Plan (source : Instance n) where
  constraintIndex : Fin source.sos1Constraints.length
  bounds : SelectorBounds
    {i // i ∈ (source.sos1Constraints.get constraintIndex).members}

namespace Plan

def constraint {source : Instance n} (plan : Plan source) : SOS1Constraint n :=
  source.sos1Constraints.get plan.constraintIndex

abbrev Member {source : Instance n} (plan : Plan source) :=
  {i // i ∈ plan.constraint.members}

def memberState {source : Instance n} (plan : Plan source)
    (state : State n) : plan.Member → Rat :=
  fun i => state i

def reusedMembers {source : Instance n} (plan : Plan source) :
    Finset plan.Member :=
  Finset.univ.filter fun i => source.domains i = .binary

def freshMembers {source : Instance n} (plan : Plan source) :
    Finset plan.Member :=
  plan.reusedMembersᶜ

/-- Exact validation performed before the target Instance is trusted.

`Rat` makes finiteness intrinsic. The independent `Domain` syntax has no
semi-continuous or semi-integer cases, so no additional kind rejection is
needed here. -/
def Valid {source : Instance n} (plan : Plan source) : Prop :=
  plan.constraint.members.Nonempty ∧
    (∀ i : plan.Member,
      (source.domains i).lowerBound = some (plan.bounds.lower i) ∧
        (source.domains i).upperBound = some (plan.bounds.upper i)) ∧
    FreshBoundsContainZero plan.reusedMembers plan.bounds

instance {source : Instance n} (plan : Plan source) :
    Decidable plan.Valid := by
  unfold Valid
  infer_instance

theorem withinBounds_of_domains {source : Instance n} (plan : Plan source)
    (hvalid : plan.Valid) {state : State n}
    (hdomains : ∀ i, state i ∈ source.domains i) :
    WithinSelectorBounds plan.bounds (plan.memberState state) := by
  intro i
  have hlower := (hvalid.2.1 i).1
  have hupper := (hvalid.2.1 i).2
  exact ⟨Domain.lowerBound_le (hdomains i) hlower,
    Domain.le_upperBound (hdomains i) hupper⟩

theorem reusedBinary_of_domains {source : Instance n} (plan : Plan source)
    {state : State n} (hdomains : ∀ i, state i ∈ source.domains i) :
    GenericBinaryOn plan.reusedMembers (plan.memberState state) := by
  intro i hi
  have hdomain : source.domains i = .binary :=
    (Finset.mem_filter.mp hi).2
  simpa [memberState, hdomain] using hdomains i

theorem genericSOS1_memberState_iff_holds {source : Instance n}
    (plan : Plan source) (state : State n) :
    GenericSOS1 (plan.memberState state) ↔ plan.constraint.Holds state := by
  classical
  rw [GenericSOS1, Finset.card_le_one]
  simp only [genericSupport, Finset.mem_filter, Finset.mem_univ, true_and]
  constructor
  · intro h i hi j hj hine hjne
    have hij :
        (⟨i, hi⟩ : plan.Member) = (⟨j, hj⟩ : plan.Member) := by
      apply h
      · simpa [memberState] using hine
      · simpa [memberState] using hjne
    exact congrArg Subtype.val hij
  · intro h i hi j hj
    apply Subtype.ext
    apply h i.val i.property j.val j.property
    · simpa [memberState] using hi
    · simpa [memberState] using hj

abbrev freshCount {source : Instance n} (plan : Plan source) : Nat :=
  plan.freshMembers.card

def freshMember {source : Instance n} (plan : Plan source)
    (j : Fin plan.freshCount) : plan.Member :=
  (plan.freshMembers.orderIsoOfFin rfl j).val

def freshIndex {source : Instance n} (plan : Plan source)
    (i : plan.Member) (hi : i ∈ plan.freshMembers) :
    Fin plan.freshCount :=
  (plan.freshMembers.orderIsoOfFin rfl).symm ⟨i, hi⟩

@[simp]
theorem freshMember_freshIndex {source : Instance n} (plan : Plan source)
    (i : plan.Member) (hi : i ∈ plan.freshMembers) :
    plan.freshMember (plan.freshIndex i hi) = i := by
  simp [freshMember, freshIndex]

@[simp]
theorem freshIndex_freshMember {source : Instance n} (plan : Plan source)
    (j : Fin plan.freshCount) :
    plan.freshIndex (plan.freshMember j) (by
      exact (plan.freshMembers.orderIsoOfFin rfl j).property) = j := by
  change (plan.freshMembers.orderIsoOfFin rfl).symm
    ((plan.freshMembers.orderIsoOfFin rfl) j) = j
  exact (plan.freshMembers.orderIsoOfFin rfl).symm_apply_apply j

def decodeState {source : Instance n} (plan : Plan source)
    (state : State (n + plan.freshCount)) : State n :=
  State.source state

def encodeSelectors {source : Instance n} (plan : Plan source)
    (state : State n) : State plan.freshCount :=
  fun j => canonicalSelector (plan.memberState state) (plan.freshMember j)

def encodeState {source : Instance n} (plan : Plan source)
    (state : State n) : State (n + plan.freshCount) :=
  State.append state (plan.encodeSelectors state)

@[simp]
theorem decode_encode {source : Instance n} (plan : Plan source)
    (state : State n) :
    plan.decodeState (plan.encodeState state) = state := by
  simp [decodeState, encodeState]

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

@[simp]
theorem freshSelectorState_encode {source : Instance n} (plan : Plan source)
    (state : State n) :
    plan.freshSelectorState state (plan.encodeSelectors state) =
      canonicalSelector (plan.memberState state) := by
  funext i
  by_cases hi : i ∈ plan.freshMembers
  · simp [freshSelectorState, hi, encodeSelectors]
  · simp [freshSelectorState, hi]

def upperLink {source : Instance n} (plan : Plan source)
    (j : Fin plan.freshCount) :
    LinearConstraint (n + plan.freshCount) where
  expr := Affine.sub
    (Affine.coordinate
      (Fin.castAdd plan.freshCount (plan.freshMember j).val))
    (Affine.scale (plan.bounds.upper (plan.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
  sense := .lessEqual

def lowerLink {source : Instance n} (plan : Plan source)
    (j : Fin plan.freshCount) :
    LinearConstraint (n + plan.freshCount) where
  expr := Affine.sub
    (Affine.scale (plan.bounds.lower (plan.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
    (Affine.coordinate
      (Fin.castAdd plan.freshCount (plan.freshMember j).val))
  sense := .lessEqual

@[simp]
theorem upperLink_holds {source : Instance n} (plan : Plan source)
    (j : Fin plan.freshCount) (state : State n)
    (selectors : State plan.freshCount) :
    (plan.upperLink j).Holds (State.append state selectors) ↔
      state (plan.freshMember j).val ≤
        plan.bounds.upper (plan.freshMember j) * selectors j := by
  simp [upperLink, LinearConstraint.Holds, State.append]

@[simp]
theorem lowerLink_holds {source : Instance n} (plan : Plan source)
    (j : Fin plan.freshCount) (state : State n)
    (selectors : State plan.freshCount) :
    (plan.lowerLink j).Holds (State.append state selectors) ↔
      plan.bounds.lower (plan.freshMember j) * selectors j ≤
        state (plan.freshMember j).val := by
  simp [lowerLink, LinearConstraint.Holds, State.append]

def linksFor {source : Instance n} (plan : Plan source)
    (j : Fin plan.freshCount) :
    List (LinearConstraint (n + plan.freshCount)) :=
  (if 0 < plan.bounds.upper (plan.freshMember j) then
      [plan.upperLink j]
    else []) ++
    if plan.bounds.lower (plan.freshMember j) < 0 then
      [plan.lowerLink j]
    else []

def linkConstraints {source : Instance n} (plan : Plan source) :
    List (LinearConstraint (n + plan.freshCount)) :=
  (List.ofFn fun j => plan.linksFor j).flatten

theorem linkConstraints_hold_iff {source : Instance n} (plan : Plan source)
    (state : State n) (selectors : State plan.freshCount) :
    (∀ constraint ∈ plan.linkConstraints,
      constraint.Holds (State.append state selectors)) ↔
      ∀ j,
        OptionalUpperLink (plan.bounds.upper (plan.freshMember j))
            (state (plan.freshMember j).val) (selectors j) ∧
          OptionalLowerLink (plan.bounds.lower (plan.freshMember j))
            (state (plan.freshMember j).val) (selectors j) := by
  simp only [linkConstraints, List.forall_mem_flatten,
    List.forall_mem_ofFn_iff]
  constructor
  · intro h j
    have hj := h j
    by_cases hu : 0 < plan.bounds.upper (plan.freshMember j)
    <;> by_cases hl : plan.bounds.lower (plan.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj
  · intro h j
    have hj := h j
    by_cases hu : 0 < plan.bounds.upper (plan.freshMember j)
    <;> by_cases hl : plan.bounds.lower (plan.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj

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

def generatedConstraints {source : Instance n} (plan : Plan source) :
    List (LinearConstraint (n + plan.freshCount)) :=
  plan.linkConstraints ++ [plan.cardinalityConstraint]

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

theorem linksForFresh_iff_linksForMembers {source : Instance n}
    (plan : Plan source) (state : State n)
    (selectors : State plan.freshCount) :
    (∀ j,
      OptionalUpperLink (plan.bounds.upper (plan.freshMember j))
          (state (plan.freshMember j).val) (selectors j) ∧
        OptionalLowerLink (plan.bounds.lower (plan.freshMember j))
          (state (plan.freshMember j).val) (selectors j)) ↔
      ∀ i, i ∉ plan.reusedMembers →
        OptionalUpperLink (plan.bounds.upper i)
            (plan.memberState state i)
            (plan.freshSelectorState state selectors i) ∧
          OptionalLowerLink (plan.bounds.lower i)
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

/-- The generated linear rows denote exactly the SDK planned-selector gadget,
provided the old and fresh blocks satisfy their declared domains. -/
theorem generatedConstraints_hold_iff_plannedSelectorGadget
    {source : Instance n} (plan : Plan source)
    (state : State n) (selectors : State plan.freshCount)
    (hsourceDomains : ∀ i, state i ∈ source.domains i)
    (hselectorDomains : ∀ j, selectors j ∈ Domain.binary) :
    (∀ constraint ∈ plan.generatedConstraints,
      constraint.Holds (State.append state selectors)) ↔
      PlannedSelectorGadget plan.reusedMembers plan.bounds
        (plan.memberState state)
        (plan.freshSelectorState state selectors) := by
  rw [generatedConstraints, List.forall_mem_append,
    List.forall_mem_singleton, plan.linkConstraints_hold_iff,
    plan.cardinalityConstraint_holds,
    plan.linksForFresh_iff_linksForMembers]
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

theorem freshSelectors_binary_of_plannedSelectorGadget
    {source : Instance n} (plan : Plan source)
    (state : State n) (selectors : State plan.freshCount)
    (hgadget : PlannedSelectorGadget plan.reusedMembers plan.bounds
      (plan.memberState state)
      (plan.freshSelectorState state selectors)) :
    ∀ j, selectors j ∈ Domain.binary := by
  intro j
  have hfresh : plan.freshMember j ∈ plan.freshMembers :=
    (plan.freshMembers.orderIsoOfFin rfl j).property
  have hr : plan.freshMember j ∉ plan.reusedMembers := by
    simpa [freshMembers] using hfresh
  have hbinary := hgadget.1 (plan.freshMember j) (by simp)
  simpa [plannedSelector, hr, freshSelectorState, hfresh] using hbinary

theorem selectedHolds_of_plannedSelectorGadget
    {source : Instance n} (plan : Plan source)
    (hvalid : plan.Valid) {state : State n}
    {selectors : State plan.freshCount}
    (hdomains : ∀ i, state i ∈ source.domains i)
    (hgadget : PlannedSelectorGadget plan.reusedMembers plan.bounds
      (plan.memberState state)
      (plan.freshSelectorState state selectors)) :
    plan.constraint.Holds state := by
  apply (plan.genericSOS1_memberState_iff_holds state).mp
  exact plannedSelectorGadget_project_sos1
    plan.reusedMembers plan.bounds
    (plan.memberState state)
    (plan.freshSelectorState state selectors)
    (plan.withinBounds_of_domains hvalid hdomains) hgadget

theorem plannedSelectorGadget_encode_of_selectedHolds
    {source : Instance n} (plan : Plan source)
    (hvalid : plan.Valid) {state : State n}
    (hdomains : ∀ i, state i ∈ source.domains i)
    (hselected : plan.constraint.Holds state) :
    PlannedSelectorGadget plan.reusedMembers plan.bounds
      (plan.memberState state)
      (plan.freshSelectorState state (plan.encodeSelectors state)) := by
  rw [plan.freshSelectorState_encode state]
  exact canonicalSelector_plannedGadget
    plan.reusedMembers plan.bounds (plan.memberState state)
    (plan.withinBounds_of_domains hvalid hdomains)
    (plan.reusedBinary_of_domains hdomains)
    ((plan.genericSOS1_memberState_iff_holds state).mpr hselected)

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

def target {source : Instance n} (plan : Plan source) :
    Instance (n + plan.freshCount) where
  domains := Domain.append source.domains (fun _ => .binary)
  constraints :=
    (source.constraints.map fun constraint =>
      constraint.extend plan.freshCount) ++ plan.generatedConstraints
  oneHotConstraints :=
    source.oneHotConstraints.map fun constraint =>
      constraint.extend plan.freshCount
  sos1Constraints :=
    (source.sos1Constraints.eraseIdx plan.constraintIndex.val).map
      fun constraint => constraint.extend plan.freshCount
  indicatorConstraints :=
    source.indicatorConstraints.map fun constraint =>
      constraint.extend plan.freshCount
  objective := source.objective.extend plan.freshCount
  sense := source.sense

theorem target_feasible_append_iff_base_and_gadget
    {source : Instance n} (plan : Plan source)
    (state : State n) (selectors : State plan.freshCount) :
    plan.target.Feasible (State.append state selectors) ↔
      plan.BaseFeasible state ∧
        PlannedSelectorGadget plan.reusedMembers plan.bounds
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
    OneHotConstraint.holds_extend_append, SOS1Constraint.holds_extend_append,
    IndicatorConstraint.holds_extend_append, BaseFeasible]
  constructor
  · rintro ⟨⟨hdomains, hselectors⟩,
      ⟨holdConstraints, hgenerated⟩, honeHot, hsos1, hindicator⟩
    have hgadget :=
      (plan.generatedConstraints_hold_iff_plannedSelectorGadget
        state selectors hdomains hselectors).mp hgenerated
    exact ⟨⟨hdomains, holdConstraints, honeHot, hsos1, hindicator⟩,
      hgadget⟩
  · rintro ⟨⟨hdomains, holdConstraints, honeHot, hsos1, hindicator⟩,
      hgadget⟩
    have hselectors :=
      plan.freshSelectors_binary_of_plannedSelectorGadget
        state selectors hgadget
    have hgenerated :=
      (plan.generatedConstraints_hold_iff_plannedSelectorGadget
        state selectors hdomains hselectors).mpr hgadget
    exact ⟨⟨hdomains, hselectors⟩,
      ⟨holdConstraints, hgenerated⟩, honeHot, hsos1, hindicator⟩

theorem target_feasible_iff_base_and_gadget
    {source : Instance n} (plan : Plan source)
    (state : State (n + plan.freshCount)) :
    plan.target.Feasible state ↔
      plan.BaseFeasible (State.source state) ∧
        PlannedSelectorGadget plan.reusedMembers plan.bounds
          (plan.memberState (State.source state))
          (plan.freshSelectorState (State.source state)
            (State.fresh state)) := by
  simpa only [State.append_source_fresh] using
    plan.target_feasible_append_iff_base_and_gadget
      (State.source state) (State.fresh state)

theorem decode_feasible {source : Instance n} (plan : Plan source)
    (hvalid : plan.Valid) {state : State (n + plan.freshCount)}
    (hfeasible : plan.target.Feasible state) :
    source.Feasible (plan.decodeState state) := by
  rcases (plan.target_feasible_iff_base_and_gadget state).mp hfeasible with
    ⟨hbase, hgadget⟩
  apply (plan.source_feasible_iff_base_and_selected
    (plan.decodeState state)).mpr
  refine ⟨hbase, ?_⟩
  exact plan.selectedHolds_of_plannedSelectorGadget hvalid hbase.1 hgadget

theorem encode_feasible {source : Instance n} (plan : Plan source)
    (hvalid : plan.Valid) {state : State n}
    (hfeasible : source.Feasible state) :
    plan.target.Feasible (plan.encodeState state) := by
  rcases (plan.source_feasible_iff_base_and_selected state).mp hfeasible with
    ⟨hbase, hselected⟩
  rw [encodeState]
  apply (plan.target_feasible_append_iff_base_and_gadget
    state (plan.encodeSelectors state)).mpr
  exact ⟨hbase,
    plan.plannedSelectorGadget_encode_of_selectedHolds
      hvalid hbase.1 hselected⟩

/-- The legacy projection view of the same lowering. Its orientation is from
the generated target back to the source Instance. -/
def projectionPreserves {source : Instance n} (plan : Plan source)
    (hvalid : plan.Valid) :
    ProjectionPreserves plan.target.asSemanticProblem
      source.asSemanticProblem where
  project := plan.decodeState
  lift := plan.encodeState
  project_feasible h := plan.decode_feasible hvalid h
  lift_feasible h := plan.encode_feasible hvalid h
  project_lift _ := plan.decode_encode _
  objective_project := by
    intro state _
    simp [Instance.asSemanticProblem, Instance.ObjectiveValue,
      target, decodeState]
  objective_lift := by
    intro state _
    simp [Instance.asSemanticProblem, Instance.ObjectiveValue,
      target, encodeState]
  sense_eq := rfl

/-- The SOS1 Big-M lowering packaged in the general Instance transformation
shape. The state maps are total; `Option` is introduced by `Transform`. -/
def lowering {source : Instance n} (plan : Plan source) :
    Instance.Transform source where
  targetDimension := n + plan.freshCount
  target := plan.target
  encode := fun state => some (plan.encodeState state)
  decode := fun state => some (plan.decodeState state)

theorem lowering_isReduction {source : Instance n} (plan : Plan source)
    (hvalid : plan.Valid) :
    plan.lowering.IsReduction := by
  intro targetState htarget
  exact ⟨plan.decodeState targetState, rfl,
    plan.decode_feasible hvalid htarget⟩

theorem lowering_isRelaxation {source : Instance n} (plan : Plan source)
    (hvalid : plan.Valid) :
    plan.lowering.IsRelaxation := by
  intro sourceState hsource
  exact ⟨plan.encodeState sourceState, rfl,
    plan.encode_feasible hvalid hsource⟩

theorem lowering_sensePreserving {source : Instance n}
    (plan : Plan source) :
    plan.lowering.SensePreserving :=
  rfl

theorem lowering_sourceObjectiveValuePreserving {source : Instance n}
    (plan : Plan source) :
    plan.lowering.SourceObjectiveValuePreserving := by
  intro sourceState _
  simp [lowering, Instance.ObjectiveValue, target, encodeState]

theorem lowering_targetObjectiveValuePreserving {source : Instance n}
    (plan : Plan source) :
    plan.lowering.TargetObjectiveValuePreserving := by
  intro targetState _
  simp [lowering, Instance.ObjectiveValue, target, decodeState]

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
  simp [lowering, plan.decode_encode]

end Plan

end SOS1BigM

end Instance

end OMMXProof
