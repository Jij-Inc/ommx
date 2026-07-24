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

`Witness` records the transformation-specific data; `Witness.Valid` is the
independently checkable precondition.
Once validity is supplied, the same central denotation theorem yields the
`Instance.Transform` reduction, relaxation, objective-preservation, and
round-trip properties.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

/-- A witness for lowering one occurrence in the source SOS1 list.

Bounds are indexed only by members of the selected SOS1 constraint. -/
structure Witness (source : Instance n) where
  constraintIndex : Fin source.sos1Constraints.length
  bounds : SelectorBounds
    {i // i ∈ (source.sos1Constraints.get constraintIndex).members}

namespace Witness

def constraint {source : Instance n} (witness : Witness source) : SOS1Constraint n :=
  source.sos1Constraints.get witness.constraintIndex

abbrev Member {source : Instance n} (witness : Witness source) :=
  {i // i ∈ witness.constraint.members}

def memberState {source : Instance n} (witness : Witness source)
    (state : State n) : witness.Member → Rat :=
  fun i => state i

def reusedMembers {source : Instance n} (witness : Witness source) :
    Finset witness.Member :=
  Finset.univ.filter fun i => source.domains i = .binary

def freshMembers {source : Instance n} (witness : Witness source) :
    Finset witness.Member :=
  witness.reusedMembersᶜ

/-- Exact validation performed before the target Instance is trusted.

`Rat` makes finiteness intrinsic. The independent `Domain` syntax has no
semi-continuous or semi-integer cases, so no additional kind rejection is
needed here. -/
def Valid {source : Instance n} (witness : Witness source) : Prop :=
  witness.constraint.members.Nonempty ∧
    (∀ i : witness.Member,
      (source.domains i).bound.lower =
          Endpoint.finite (witness.bounds.lower i) ∧
        (source.domains i).bound.upper =
          Endpoint.finite (witness.bounds.upper i)) ∧
    FreshBoundsContainZero witness.reusedMembers witness.bounds

instance {source : Instance n} (witness : Witness source) :
    Decidable witness.Valid := by
  unfold Valid
  infer_instance

theorem withinBounds_of_domains {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) {state : State n}
    (hdomains : ∀ i, state i ∈ source.domains i) :
    WithinSelectorBounds witness.bounds (witness.memberState state) := by
  intro i
  have hlower := (hvalid.2.1 i).1
  have hupper := (hvalid.2.1 i).2
  exact ⟨Domain.finite_lower_le (hdomains i) hlower,
    Domain.le_finite_upper (hdomains i) hupper⟩

theorem reusedBinary_of_domains {source : Instance n} (witness : Witness source)
    {state : State n} (hdomains : ∀ i, state i ∈ source.domains i) :
    GenericBinaryOn witness.reusedMembers (witness.memberState state) := by
  intro i hi
  have hdomain : source.domains i = .binary :=
    (Finset.mem_filter.mp hi).2
  simpa [memberState, hdomain] using hdomains i

theorem genericSOS1_memberState_iff_holds {source : Instance n}
    (witness : Witness source) (state : State n) :
    GenericSOS1 (witness.memberState state) ↔ witness.constraint.Holds state := by
  classical
  rw [GenericSOS1, Finset.card_le_one]
  simp only [genericSupport, Finset.mem_filter, Finset.mem_univ, true_and]
  constructor
  · intro h i hi j hj hine hjne
    have hij :
        (⟨i, hi⟩ : witness.Member) = (⟨j, hj⟩ : witness.Member) := by
      apply h
      · simpa [memberState] using hine
      · simpa [memberState] using hjne
    exact congrArg Subtype.val hij
  · intro h i hi j hj
    apply Subtype.ext
    apply h i.val i.property j.val j.property
    · simpa [memberState] using hi
    · simpa [memberState] using hj

abbrev freshCount {source : Instance n} (witness : Witness source) : Nat :=
  witness.freshMembers.card

def freshMember {source : Instance n} (witness : Witness source)
    (j : Fin witness.freshCount) : witness.Member :=
  (witness.freshMembers.orderIsoOfFin rfl j).val

def freshIndex {source : Instance n} (witness : Witness source)
    (i : witness.Member) (hi : i ∈ witness.freshMembers) :
    Fin witness.freshCount :=
  (witness.freshMembers.orderIsoOfFin rfl).symm ⟨i, hi⟩

@[simp]
theorem freshMember_freshIndex {source : Instance n} (witness : Witness source)
    (i : witness.Member) (hi : i ∈ witness.freshMembers) :
    witness.freshMember (witness.freshIndex i hi) = i := by
  simp [freshMember, freshIndex]

@[simp]
theorem freshIndex_freshMember {source : Instance n} (witness : Witness source)
    (j : Fin witness.freshCount) :
    witness.freshIndex (witness.freshMember j) (by
      exact (witness.freshMembers.orderIsoOfFin rfl j).property) = j := by
  change (witness.freshMembers.orderIsoOfFin rfl).symm
    ((witness.freshMembers.orderIsoOfFin rfl) j) = j
  exact (witness.freshMembers.orderIsoOfFin rfl).symm_apply_apply j

def decodeState {source : Instance n} (witness : Witness source)
    (state : State (n + witness.freshCount)) : State n :=
  State.source state

def encodeSelectors {source : Instance n} (witness : Witness source)
    (state : State n) : State witness.freshCount :=
  fun j => canonicalSelector (witness.memberState state) (witness.freshMember j)

def encodeState {source : Instance n} (witness : Witness source)
    (state : State n) : State (n + witness.freshCount) :=
  State.append state (witness.encodeSelectors state)

@[simp]
theorem decode_encode {source : Instance n} (witness : Witness source)
    (state : State n) :
    witness.decodeState (witness.encodeState state) = state := by
  simp [decodeState, encodeState]

/-- A virtual selector tuple indexed by every SOS1 member. At a reused member
its value is ignored by `plannedSelector`; choosing the canonical value makes
the encode lemma exact. -/
def freshSelectorState {source : Instance n} (witness : Witness source)
    (members : State n) (fresh : State witness.freshCount) :
    witness.Member → Rat :=
  fun i =>
    if hi : i ∈ witness.freshMembers then
      fresh (witness.freshIndex i hi)
    else
      canonicalSelector (witness.memberState members) i

@[simp]
theorem freshSelectorState_encode {source : Instance n} (witness : Witness source)
    (state : State n) :
    witness.freshSelectorState state (witness.encodeSelectors state) =
      canonicalSelector (witness.memberState state) := by
  funext i
  by_cases hi : i ∈ witness.freshMembers
  · simp [freshSelectorState, hi, encodeSelectors]
  · simp [freshSelectorState, hi]

def upperLink {source : Instance n} (witness : Witness source)
    (j : Fin witness.freshCount) :
    LinearConstraint (n + witness.freshCount) where
  expr := Affine.sub
    (Affine.coordinate
      (Fin.castAdd witness.freshCount (witness.freshMember j).val))
    (Affine.scale (witness.bounds.upper (witness.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
  sense := .lessEqual

def lowerLink {source : Instance n} (witness : Witness source)
    (j : Fin witness.freshCount) :
    LinearConstraint (n + witness.freshCount) where
  expr := Affine.sub
    (Affine.scale (witness.bounds.lower (witness.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
    (Affine.coordinate
      (Fin.castAdd witness.freshCount (witness.freshMember j).val))
  sense := .lessEqual

@[simp]
theorem upperLink_holds {source : Instance n} (witness : Witness source)
    (j : Fin witness.freshCount) (state : State n)
    (selectors : State witness.freshCount) :
    (witness.upperLink j).Holds (State.append state selectors) ↔
      state (witness.freshMember j).val ≤
        witness.bounds.upper (witness.freshMember j) * selectors j := by
  simp [upperLink, LinearConstraint.Holds, State.append]

@[simp]
theorem lowerLink_holds {source : Instance n} (witness : Witness source)
    (j : Fin witness.freshCount) (state : State n)
    (selectors : State witness.freshCount) :
    (witness.lowerLink j).Holds (State.append state selectors) ↔
      witness.bounds.lower (witness.freshMember j) * selectors j ≤
        state (witness.freshMember j).val := by
  simp [lowerLink, LinearConstraint.Holds, State.append]

def linksFor {source : Instance n} (witness : Witness source)
    (j : Fin witness.freshCount) :
    List (LinearConstraint (n + witness.freshCount)) :=
  (if 0 < witness.bounds.upper (witness.freshMember j) then
      [witness.upperLink j]
    else []) ++
    if witness.bounds.lower (witness.freshMember j) < 0 then
      [witness.lowerLink j]
    else []

def linkConstraints {source : Instance n} (witness : Witness source) :
    List (LinearConstraint (n + witness.freshCount)) :=
  (List.ofFn fun j => witness.linksFor j).flatten

theorem linkConstraints_hold_iff {source : Instance n} (witness : Witness source)
    (state : State n) (selectors : State witness.freshCount) :
    (∀ constraint ∈ witness.linkConstraints,
      constraint.Holds (State.append state selectors)) ↔
      ∀ j,
        OptionalUpperLink (witness.bounds.upper (witness.freshMember j))
            (state (witness.freshMember j).val) (selectors j) ∧
          OptionalLowerLink (witness.bounds.lower (witness.freshMember j))
            (state (witness.freshMember j).val) (selectors j) := by
  simp only [linkConstraints, List.forall_mem_flatten,
    List.forall_mem_ofFn_iff]
  constructor
  · intro h j
    have hj := h j
    by_cases hu : 0 < witness.bounds.upper (witness.freshMember j)
    <;> by_cases hl : witness.bounds.lower (witness.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj
  · intro h j
    have hj := h j
    by_cases hu : 0 < witness.bounds.upper (witness.freshMember j)
    <;> by_cases hl : witness.bounds.lower (witness.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj

/-- Coefficients of the mixed cardinality row: reused binary members stay in
the source block and every fresh selector contributes in the right block. -/
def cardinalityExpr {source : Instance n} (witness : Witness source) :
    Affine (n + witness.freshCount) where
  coeff := Fin.append
    (fun i =>
      if i ∈ witness.constraint.members ∧ source.domains i = .binary then 1 else 0)
    (fun _ => 1)
  constant := -1

def cardinalityConstraint {source : Instance n} (witness : Witness source) :
    LinearConstraint (n + witness.freshCount) where
  expr := witness.cardinalityExpr
  sense := .lessEqual

def selectorSum {source : Instance n} (witness : Witness source)
    (state : State n) (selectors : State witness.freshCount) : Rat :=
  (∑ i ∈ witness.constraint.members,
      if source.domains i = .binary then state i else 0) +
    ∑ j, selectors j

def reusedContribution {source : Instance n} (witness : Witness source)
    (state : State n) (i : witness.Member) : Rat :=
  if i ∈ witness.reusedMembers then witness.memberState state i else 0

def freshContribution {source : Instance n} (witness : Witness source)
    (state : State n) (selectors : State witness.freshCount)
    (i : witness.Member) : Rat :=
  if i ∈ witness.freshMembers then witness.freshSelectorState state selectors i else 0

theorem plannedSelector_eq_contributions {source : Instance n}
    (witness : Witness source) (state : State n)
    (selectors : State witness.freshCount) (i : witness.Member) :
    plannedSelector witness.reusedMembers (witness.memberState state)
        (witness.freshSelectorState state selectors) i =
      witness.reusedContribution state i +
        witness.freshContribution state selectors i := by
  by_cases hr : i ∈ witness.reusedMembers
  · have hf : i ∉ witness.freshMembers := by
      simp [freshMembers, hr]
    simp [plannedSelector, reusedContribution, freshContribution, hr, hf]
  · have hf : i ∈ witness.freshMembers := by
      simp [freshMembers, hr]
    simp [plannedSelector, reusedContribution, freshContribution, hr, hf]

theorem sourceReusedSum_eq {source : Instance n} (witness : Witness source)
    (state : State n) :
    (∑ i ∈ witness.constraint.members,
      if source.domains i = .binary then state i else 0) =
      ∑ i : witness.Member, witness.reusedContribution state i := by
  symm
  calc
    (∑ i : witness.Member, witness.reusedContribution state i) =
        ∑ i : witness.Member,
          if source.domains i = .binary then state i else 0 := by
      apply Finset.sum_congr rfl
      intro i _
      simp [reusedContribution, reusedMembers, memberState]
    _ = ∑ i ∈ witness.constraint.members,
          if source.domains i = .binary then state i else 0 := by
      exact Finset.sum_coe_sort witness.constraint.members
        (fun i => if source.domains i = .binary then state i else 0)

theorem freshSelectorSum_eq {source : Instance n} (witness : Witness source)
    (state : State n) (selectors : State witness.freshCount) :
    (∑ j, selectors j) =
      ∑ i : witness.Member, witness.freshContribution state selectors i := by
  calc
    (∑ j, selectors j) =
        ∑ i : {i // i ∈ witness.freshMembers},
          witness.freshSelectorState state selectors i.val := by
      apply Fintype.sum_equiv
        (witness.freshMembers.orderIsoOfFin rfl).toEquiv
      intro j
      unfold freshSelectorState
      have hi :
          ((witness.freshMembers.orderIsoOfFin rfl).toEquiv j).val ∈
            witness.freshMembers :=
        ((witness.freshMembers.orderIsoOfFin rfl).toEquiv j).property
      rw [dif_pos hi]
      change selectors j =
        selectors ((witness.freshMembers.orderIsoOfFin rfl).symm
          ((witness.freshMembers.orderIsoOfFin rfl) j))
      exact congrArg selectors
        ((witness.freshMembers.orderIsoOfFin rfl).symm_apply_apply j).symm
    _ = ∑ i ∈ witness.freshMembers,
          witness.freshSelectorState state selectors i := by
      exact Finset.sum_coe_sort witness.freshMembers
        (witness.freshSelectorState state selectors)
    _ = ∑ i : witness.Member, witness.freshContribution state selectors i := by
      symm
      simpa [freshContribution] using
        (Finset.sum_ite_mem_eq witness.freshMembers
          (witness.freshSelectorState state selectors))

theorem selectorSum_eq_plannedSelector {source : Instance n}
    (witness : Witness source) (state : State n)
    (selectors : State witness.freshCount) :
    witness.selectorSum state selectors =
      ∑ i, plannedSelector witness.reusedMembers (witness.memberState state)
        (witness.freshSelectorState state selectors) i := by
  rw [selectorSum, witness.sourceReusedSum_eq state,
    witness.freshSelectorSum_eq state selectors]
  rw [← Finset.sum_add_distrib]
  apply Finset.sum_congr rfl
  intro i _
  exact (witness.plannedSelector_eq_contributions state selectors i).symm

@[simp]
theorem cardinalityConstraint_holds {source : Instance n} (witness : Witness source)
    (state : State n) (selectors : State witness.freshCount) :
    witness.cardinalityConstraint.Holds (State.append state selectors) ↔
      witness.selectorSum state selectors ≤ 1 := by
  have hsum :
      (∑ i : Fin n,
        if i ∈ witness.constraint.members ∧ source.domains i = .binary then
          state i
        else 0) =
        ∑ i ∈ witness.constraint.members,
          if source.domains i = .binary then state i else 0 := by
    convert Finset.sum_ite_mem_eq witness.constraint.members
      (fun i => if source.domains i = .binary then state i else 0) using 1
    simp [ite_and]
  simp [cardinalityConstraint, cardinalityExpr, selectorSum,
    LinearConstraint.Holds, Affine.eval, State.append, Fin.sum_univ_add]
  rw [hsum]

def generatedConstraints {source : Instance n} (witness : Witness source) :
    List (LinearConstraint (n + witness.freshCount)) :=
  witness.linkConstraints ++ [witness.cardinalityConstraint]

theorem plannedSelector_binary_of_domains {source : Instance n}
    (witness : Witness source) (state : State n)
    (selectors : State witness.freshCount)
    (hsourceDomains : ∀ i, state i ∈ source.domains i)
    (hselectorDomains : ∀ j, selectors j ∈ Domain.binary) :
    GenericBinaryOn Finset.univ
      (plannedSelector witness.reusedMembers (witness.memberState state)
        (witness.freshSelectorState state selectors)) := by
  intro i _
  by_cases hr : i ∈ witness.reusedMembers
  · have hdomain : source.domains i = .binary :=
      (Finset.mem_filter.mp hr).2
    simpa [plannedSelector, hr, memberState, hdomain] using
      hsourceDomains i
  · have hfresh : i ∈ witness.freshMembers := by
      simp [freshMembers, hr]
    simpa [plannedSelector, hr, freshSelectorState, hfresh] using
      hselectorDomains (witness.freshIndex i hfresh)

theorem linksForFresh_iff_linksForMembers {source : Instance n}
    (witness : Witness source) (state : State n)
    (selectors : State witness.freshCount) :
    (∀ j,
      OptionalUpperLink (witness.bounds.upper (witness.freshMember j))
          (state (witness.freshMember j).val) (selectors j) ∧
        OptionalLowerLink (witness.bounds.lower (witness.freshMember j))
          (state (witness.freshMember j).val) (selectors j)) ↔
      ∀ i, i ∉ witness.reusedMembers →
        OptionalUpperLink (witness.bounds.upper i)
            (witness.memberState state i)
            (witness.freshSelectorState state selectors i) ∧
          OptionalLowerLink (witness.bounds.lower i)
            (witness.memberState state i)
            (witness.freshSelectorState state selectors i) := by
  constructor
  · intro h i hr
    have hfresh : i ∈ witness.freshMembers := by
      simp [freshMembers, hr]
    have hi := h (witness.freshIndex i hfresh)
    simpa [memberState, freshSelectorState, hfresh] using hi
  · intro h j
    have hfresh : witness.freshMember j ∈ witness.freshMembers :=
      (witness.freshMembers.orderIsoOfFin rfl j).property
    have hr : witness.freshMember j ∉ witness.reusedMembers := by
      simpa [freshMembers] using hfresh
    have hj := h (witness.freshMember j) hr
    simpa [memberState, freshSelectorState, hfresh] using hj

/-- The generated linear rows denote exactly the SDK planned-selector gadget,
provided the old and fresh blocks satisfy their declared domains. -/
theorem generatedConstraints_hold_iff_plannedSelectorGadget
    {source : Instance n} (witness : Witness source)
    (state : State n) (selectors : State witness.freshCount)
    (hsourceDomains : ∀ i, state i ∈ source.domains i)
    (hselectorDomains : ∀ j, selectors j ∈ Domain.binary) :
    (∀ constraint ∈ witness.generatedConstraints,
      constraint.Holds (State.append state selectors)) ↔
      PlannedSelectorGadget witness.reusedMembers witness.bounds
        (witness.memberState state)
        (witness.freshSelectorState state selectors) := by
  rw [generatedConstraints, List.forall_mem_append,
    List.forall_mem_singleton, witness.linkConstraints_hold_iff,
    witness.cardinalityConstraint_holds,
    witness.linksForFresh_iff_linksForMembers]
  constructor
  · rintro ⟨hlinks, hcardinality⟩
    refine ⟨witness.plannedSelector_binary_of_domains state selectors
      hsourceDomains hselectorDomains, hlinks, ?_⟩
    rw [← witness.selectorSum_eq_plannedSelector state selectors]
    exact hcardinality
  · rintro ⟨_, hlinks, hcardinality⟩
    refine ⟨hlinks, ?_⟩
    rw [witness.selectorSum_eq_plannedSelector state selectors]
    exact hcardinality

theorem freshSelectors_binary_of_plannedSelectorGadget
    {source : Instance n} (witness : Witness source)
    (state : State n) (selectors : State witness.freshCount)
    (hgadget : PlannedSelectorGadget witness.reusedMembers witness.bounds
      (witness.memberState state)
      (witness.freshSelectorState state selectors)) :
    ∀ j, selectors j ∈ Domain.binary := by
  intro j
  have hfresh : witness.freshMember j ∈ witness.freshMembers :=
    (witness.freshMembers.orderIsoOfFin rfl j).property
  have hr : witness.freshMember j ∉ witness.reusedMembers := by
    simpa [freshMembers] using hfresh
  have hbinary := hgadget.1 (witness.freshMember j) (by simp)
  simpa [plannedSelector, hr, freshSelectorState, hfresh] using hbinary

theorem selectedHolds_of_plannedSelectorGadget
    {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) {state : State n}
    {selectors : State witness.freshCount}
    (hdomains : ∀ i, state i ∈ source.domains i)
    (hgadget : PlannedSelectorGadget witness.reusedMembers witness.bounds
      (witness.memberState state)
      (witness.freshSelectorState state selectors)) :
    witness.constraint.Holds state := by
  apply (witness.genericSOS1_memberState_iff_holds state).mp
  exact plannedSelectorGadget_project_sos1
    witness.reusedMembers witness.bounds
    (witness.memberState state)
    (witness.freshSelectorState state selectors)
    (witness.withinBounds_of_domains hvalid hdomains) hgadget

theorem plannedSelectorGadget_encode_of_selectedHolds
    {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) {state : State n}
    (hdomains : ∀ i, state i ∈ source.domains i)
    (hselected : witness.constraint.Holds state) :
    PlannedSelectorGadget witness.reusedMembers witness.bounds
      (witness.memberState state)
      (witness.freshSelectorState state (witness.encodeSelectors state)) := by
  rw [witness.freshSelectorState_encode state]
  exact canonicalSelector_plannedGadget
    witness.reusedMembers witness.bounds (witness.memberState state)
    (witness.withinBounds_of_domains hvalid hdomains)
    (witness.reusedBinary_of_domains hdomains)
    ((witness.genericSOS1_memberState_iff_holds state).mpr hselected)

def BaseFeasible {source : Instance n} (witness : Witness source)
    (state : State n) : Prop :=
  (∀ i, state i ∈ source.domains i) ∧
    (∀ constraint ∈ source.constraints, constraint.Holds state) ∧
    (∀ constraint ∈ source.oneHotConstraints, constraint.Holds state) ∧
    (∀ constraint ∈ source.sos1Constraints.eraseIdx witness.constraintIndex.val,
      constraint.Holds state) ∧
    ∀ constraint ∈ source.indicatorConstraints, constraint.Holds state

theorem allSOS1_iff_erased_and_selected {source : Instance n}
    (witness : Witness source) (state : State n) :
    (∀ constraint ∈ source.sos1Constraints, constraint.Holds state) ↔
      (∀ constraint ∈
        source.sos1Constraints.eraseIdx witness.constraintIndex.val,
        constraint.Holds state) ∧
        witness.constraint.Holds state := by
  constructor
  · intro hall
    constructor
    · intro constraint hconstraint
      exact hall constraint (List.mem_of_mem_eraseIdx hconstraint)
    · exact hall witness.constraint (List.get_mem _ witness.constraintIndex)
  · rintro ⟨herased, hselected⟩ constraint hconstraint
    have hperm := List.getElem_cons_eraseIdx_perm
      (l := source.sos1Constraints) witness.constraintIndex.isLt
    have hleft :
        constraint ∈
          witness.constraint ::
            source.sos1Constraints.eraseIdx witness.constraintIndex.val := by
      exact hperm.symm.subset hconstraint
    rcases List.mem_cons.mp hleft with hsame | herasedMem
    · simpa [hsame] using hselected
    · exact herased constraint herasedMem

theorem source_feasible_iff_base_and_selected {source : Instance n}
    (witness : Witness source) (state : State n) :
    source.Feasible state ↔
      witness.BaseFeasible state ∧ witness.constraint.Holds state := by
  unfold Instance.Feasible BaseFeasible
  rw [witness.allSOS1_iff_erased_and_selected state]
  aesop

def target {source : Instance n} (witness : Witness source) :
    Instance (n + witness.freshCount) where
  domains := Domain.append source.domains (fun _ => .binary)
  constraints :=
    (source.constraints.map fun constraint =>
      constraint.extend witness.freshCount) ++ witness.generatedConstraints
  oneHotConstraints :=
    source.oneHotConstraints.map fun constraint =>
      constraint.extend witness.freshCount
  sos1Constraints :=
    (source.sos1Constraints.eraseIdx witness.constraintIndex.val).map
      fun constraint => constraint.extend witness.freshCount
  indicatorConstraints :=
    source.indicatorConstraints.map fun constraint =>
      constraint.extend witness.freshCount
  objective := source.objective.extend witness.freshCount
  sense := source.sense

theorem target_feasible_append_iff_base_and_gadget
    {source : Instance n} (witness : Witness source)
    (state : State n) (selectors : State witness.freshCount) :
    witness.target.Feasible (State.append state selectors) ↔
      witness.BaseFeasible state ∧
        PlannedSelectorGadget witness.reusedMembers witness.bounds
          (witness.memberState state)
          (witness.freshSelectorState state selectors) := by
  have hsourceAt (i : Fin n) :
      State.append state selectors (Fin.castAdd witness.freshCount i) = state i := by
    simp [State.append]
  have hfreshAt (j : Fin witness.freshCount) :
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
      (witness.generatedConstraints_hold_iff_plannedSelectorGadget
        state selectors hdomains hselectors).mp hgenerated
    exact ⟨⟨hdomains, holdConstraints, honeHot, hsos1, hindicator⟩,
      hgadget⟩
  · rintro ⟨⟨hdomains, holdConstraints, honeHot, hsos1, hindicator⟩,
      hgadget⟩
    have hselectors :=
      witness.freshSelectors_binary_of_plannedSelectorGadget
        state selectors hgadget
    have hgenerated :=
      (witness.generatedConstraints_hold_iff_plannedSelectorGadget
        state selectors hdomains hselectors).mpr hgadget
    exact ⟨⟨hdomains, hselectors⟩,
      ⟨holdConstraints, hgenerated⟩, honeHot, hsos1, hindicator⟩

theorem target_feasible_iff_base_and_gadget
    {source : Instance n} (witness : Witness source)
    (state : State (n + witness.freshCount)) :
    witness.target.Feasible state ↔
      witness.BaseFeasible (State.source state) ∧
        PlannedSelectorGadget witness.reusedMembers witness.bounds
          (witness.memberState (State.source state))
          (witness.freshSelectorState (State.source state)
            (State.fresh state)) := by
  simpa only [State.append_source_fresh] using
    witness.target_feasible_append_iff_base_and_gadget
      (State.source state) (State.fresh state)

theorem decode_feasible {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) {state : State (n + witness.freshCount)}
    (hfeasible : witness.target.Feasible state) :
    source.Feasible (witness.decodeState state) := by
  rcases (witness.target_feasible_iff_base_and_gadget state).mp hfeasible with
    ⟨hbase, hgadget⟩
  apply (witness.source_feasible_iff_base_and_selected
    (witness.decodeState state)).mpr
  refine ⟨hbase, ?_⟩
  exact witness.selectedHolds_of_plannedSelectorGadget hvalid hbase.1 hgadget

theorem encode_feasible {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) {state : State n}
    (hfeasible : source.Feasible state) :
    witness.target.Feasible (witness.encodeState state) := by
  rcases (witness.source_feasible_iff_base_and_selected state).mp hfeasible with
    ⟨hbase, hselected⟩
  rw [encodeState]
  apply (witness.target_feasible_append_iff_base_and_gadget
    state (witness.encodeSelectors state)).mpr
  exact ⟨hbase,
    witness.plannedSelectorGadget_encode_of_selectedHolds
      hvalid hbase.1 hselected⟩

/-- The SOS1 Big-M lowering packaged in the general Instance transformation
shape. The state maps are total; `Option` is introduced by `Transform`. -/
def lowering {source : Instance n} (witness : Witness source) :
    Instance.Transform source where
  targetDimension := n + witness.freshCount
  target := witness.target
  encode := fun state => some (witness.encodeState state)
  decode := fun state => some (witness.decodeState state)

theorem lowering_isReduction {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) :
    witness.lowering.IsReduction := by
  intro targetState htarget
  exact ⟨witness.decodeState targetState, rfl,
    witness.decode_feasible hvalid htarget⟩

theorem lowering_isRelaxation {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) :
    witness.lowering.IsRelaxation := by
  intro sourceState hsource
  exact ⟨witness.encodeState sourceState, rfl,
    witness.encode_feasible hvalid hsource⟩

theorem lowering_sensePreserving {source : Instance n}
    (witness : Witness source) :
    witness.lowering.SensePreserving :=
  rfl

theorem lowering_sourceObjectiveValuePreserving {source : Instance n}
    (witness : Witness source) :
    witness.lowering.SourceObjectiveValuePreserving := by
  intro sourceState _
  simp [lowering, Instance.ObjectiveValue, target, encodeState]

theorem lowering_targetObjectiveValuePreserving {source : Instance n}
    (witness : Witness source) :
    witness.lowering.TargetObjectiveValuePreserving := by
  intro targetState _
  simp [lowering, Instance.ObjectiveValue, target, decodeState]

theorem lowering_sourceObjectivePreserving {source : Instance n}
    (witness : Witness source) :
    witness.lowering.SourceObjectivePreserving :=
  ⟨witness.lowering_sensePreserving,
    witness.lowering_sourceObjectiveValuePreserving⟩

theorem lowering_targetObjectivePreserving {source : Instance n}
    (witness : Witness source) :
    witness.lowering.TargetObjectivePreserving :=
  ⟨witness.lowering_sensePreserving,
    witness.lowering_targetObjectiveValuePreserving⟩

theorem lowering_sourceRoundTrip {source : Instance n}
    (witness : Witness source) :
    witness.lowering.SourceRoundTrip := by
  intro sourceState _
  simp [lowering, witness.decode_encode]

end Witness

end SOS1BigM

end Instance

end OMMXProof
