import OMMXProof.Instance.Transform.SOS1BigM.Formulation
import OMMXProof.Instance.Transform.SOS1BigM.Promotion.Target
import Mathlib.Tactic

/-!
# Canonical selector formulation for SOS1 Big-M promotion

This module defines the regular Big-M link and cardinality constraints generated
by a promotion witness and relates them to the abstract selector formulation.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

namespace Witness

section Source

variable (witness : Witness n)
variable (source : Instance (n + witness.freshCount))

def memberState (state : State n) : witness.Member → Rat :=
  fun i => state i

/-- A virtual selector tuple indexed by every promoted SOS1 member. -/
def freshSelectorState (members : State n)
    (fresh : State witness.freshCount) :
    witness.Member → Rat :=
  fun i =>
    if hi : i ∈ witness.freshMembers then
      fresh (witness.freshIndex i hi)
    else
      canonicalSelector (witness.memberState members) i

/-- Finite declared bounds for every promoted SOS1 member. -/
def FiniteMemberBounds : Prop :=
  ∀ i : witness.Member, ((witness.target source).domains i).bound.IsFinite

instance : Decidable (witness.FiniteMemberBounds source) := by
  unfold FiniteMemberBounds
  infer_instance

/-- Bounds extracted from the retained member domains.

The fallback zero is used only to keep construction total for untrusted
witnesses. `StandardBigMForm.finiteMemberBounds` proves that the fallback is
unreachable in every accepted promotion. -/
def selectorBounds : SelectorBounds witness.Member where
  lower := fun i =>
    if h : ((witness.target source).domains i).bound.IsFinite then
      ((witness.target source).domains i).bound.finiteLower h
    else 0
  upper := fun i =>
    if h : ((witness.target source).domains i).bound.IsFinite then
      ((witness.target source).domains i).bound.finiteUpper h
    else 0

theorem selectorBounds_exact
    (hfinite : witness.FiniteMemberBounds source)
    (i : witness.Member) :
    ((witness.target source).domains i).bound.lower =
        .finite ((witness.selectorBounds source).lower i) ∧
      ((witness.target source).domains i).bound.upper =
        .finite ((witness.selectorBounds source).upper i) := by
  have hi := hfinite i
  simp only [selectorBounds, hi, ↓reduceDIte]
  exact ⟨Bound.lower_eq_finiteLower _ hi,
    Bound.upper_eq_finiteUpper _ hi⟩

theorem withinSelectorBounds_of_domains
    (hfinite : witness.FiniteMemberBounds source)
    {state : State n}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i) :
    WithinSelectorBounds (witness.selectorBounds source)
      (witness.memberState state) := by
  intro i
  exact
    ⟨Domain.finite_lower_le (hdomains i)
        (witness.selectorBounds_exact source hfinite i).1,
      Domain.le_finite_upper (hdomains i)
        (witness.selectorBounds_exact source hfinite i).2⟩

/-- The witness classifies reused members exactly as retained binary members. -/
def ReusedMembersMatchDomains : Prop :=
  ∀ i : witness.Member,
    i ∈ witness.reusedMembers ↔ (witness.target source).domains i = .binary

instance : Decidable (witness.ReusedMembersMatchDomains source) := by
  unfold ReusedMembersMatchDomains
  infer_instance

theorem reusedBinary_of_domains
    (hlayout : witness.ReusedMembersMatchDomains source)
    {state : State n}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i) :
    GenericBinaryOn witness.reusedMembers (witness.memberState state) := by
  intro i hi
  have hdomain : (witness.target source).domains i = .binary :=
    (hlayout i).mp hi
  simpa [memberState, hdomain] using hdomains i

theorem genericSOS1_memberState_iff_holds (state : State n) :
    GenericSOS1 (witness.memberState state) ↔
      witness.promotedConstraint.Holds state := by
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

/-! ## The canonical regular selector formulation -/

def upperLink (j : Fin witness.freshCount) :
    LinearConstraint (n + witness.freshCount) where
  expr := Affine.sub
    (Affine.coordinate
      (Fin.castAdd witness.freshCount (witness.freshMember j).val))
    (Affine.scale
      ((witness.selectorBounds source).upper (witness.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
  sense := .lessEqual

def lowerLink (j : Fin witness.freshCount) :
    LinearConstraint (n + witness.freshCount) where
  expr := Affine.sub
    (Affine.scale
      ((witness.selectorBounds source).lower (witness.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
    (Affine.coordinate
      (Fin.castAdd witness.freshCount (witness.freshMember j).val))
  sense := .lessEqual

@[simp]
theorem upperLink_holds
    (j : Fin witness.freshCount) (state : State n)
    (selectors : State witness.freshCount) :
    (witness.upperLink source j).Holds (State.append state selectors) ↔
      state (witness.freshMember j).val ≤
        (witness.selectorBounds source).upper (witness.freshMember j) *
          selectors j := by
  simp [upperLink, LinearConstraint.Holds, State.append]

@[simp]
theorem lowerLink_holds
    (j : Fin witness.freshCount) (state : State n)
    (selectors : State witness.freshCount) :
    (witness.lowerLink source j).Holds (State.append state selectors) ↔
      (witness.selectorBounds source).lower (witness.freshMember j) *
          selectors j ≤
        state (witness.freshMember j).val := by
  simp [lowerLink, LinearConstraint.Holds, State.append]

def linksFor (j : Fin witness.freshCount) :
    List (LinearConstraint (n + witness.freshCount)) :=
  (if 0 <
      (witness.selectorBounds source).upper (witness.freshMember j) then
      [witness.upperLink source j]
    else []) ++
    if (witness.selectorBounds source).lower (witness.freshMember j) < 0 then
      [witness.lowerLink source j]
    else []

def linkConstraints :
    List (LinearConstraint (n + witness.freshCount)) :=
  (List.ofFn fun j => witness.linksFor source j).flatten

theorem linkConstraints_hold_iff
    (state : State n) (selectors : State witness.freshCount) :
    (∀ constraint ∈ witness.linkConstraints source,
      constraint.Holds (State.append state selectors)) ↔
      ∀ j,
        OptionalUpperLink
            ((witness.selectorBounds source).upper (witness.freshMember j))
            (state (witness.freshMember j).val) (selectors j) ∧
          OptionalLowerLink
            ((witness.selectorBounds source).lower (witness.freshMember j))
            (state (witness.freshMember j).val) (selectors j) := by
  simp only [linkConstraints, List.forall_mem_flatten,
    List.forall_mem_ofFn_iff]
  constructor
  · intro h j
    have hj := h j
    by_cases hu :
      0 < (witness.selectorBounds source).upper (witness.freshMember j)
    <;> by_cases hl :
      (witness.selectorBounds source).lower (witness.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj
  · intro h j
    have hj := h j
    by_cases hu :
      0 < (witness.selectorBounds source).upper (witness.freshMember j)
    <;> by_cases hl :
      (witness.selectorBounds source).lower (witness.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj

/-- Coefficients of the mixed cardinality row. Reused binary members remain in
the retained block, and every fresh selector contributes in the suffix block. -/
def cardinalityExpr : Affine (n + witness.freshCount) where
  coeff := Fin.append
    (fun i =>
      if i ∈ witness.members ∧
          (witness.target source).domains i = .binary then 1 else 0)
    (fun _ => 1)
  constant := -1

def cardinalityConstraint :
    LinearConstraint (n + witness.freshCount) where
  expr := witness.cardinalityExpr source
  sense := .lessEqual

def selectorSum (state : State n)
    (selectors : State witness.freshCount) : Rat :=
  (∑ i ∈ witness.members,
      if (witness.target source).domains i = .binary then state i else 0) +
    ∑ j, selectors j

def reusedContribution (state : State n) (i : witness.Member) : Rat :=
  if i ∈ witness.reusedMembers then witness.memberState state i else 0

def freshContribution (state : State n)
    (selectors : State witness.freshCount) (i : witness.Member) : Rat :=
  if i ∈ witness.freshMembers then
    witness.freshSelectorState state selectors i
  else 0

theorem plannedSelector_eq_contributions
    (state : State n) (selectors : State witness.freshCount)
    (i : witness.Member) :
    plannedSelector witness.reusedMembers (witness.memberState state)
        (witness.freshSelectorState state selectors) i =
      witness.reusedContribution state i +
        witness.freshContribution state selectors i := by
  by_cases hr : i ∈ witness.reusedMembers
  · have hf : i ∉ witness.freshMembers := by
      simpa [reusedMembers] using hr
    simp [plannedSelector, reusedContribution, freshContribution, hr, hf]
  · have hf : i ∈ witness.freshMembers := by
      simp [reusedMembers] at hr
      exact hr
    simp [plannedSelector, reusedContribution, freshContribution, hr, hf]

theorem retainedReusedSum_eq
    (hlayout : witness.ReusedMembersMatchDomains source)
    (state : State n) :
    (∑ i ∈ witness.members,
      if (witness.target source).domains i = .binary then state i else 0) =
      ∑ i : witness.Member, witness.reusedContribution state i := by
  symm
  calc
    (∑ i : witness.Member, witness.reusedContribution state i) =
        ∑ i : witness.Member,
          if (witness.target source).domains i = .binary then state i else 0 := by
      apply Finset.sum_congr rfl
      intro i _
      rw [reusedContribution]
      by_cases hr : i ∈ witness.reusedMembers
      · simp only [hr, ↓reduceIte]
        rw [(hlayout i).mp hr]
        rfl
      · simp only [hr, ↓reduceIte]
        have hne : (witness.target source).domains i ≠ .binary := by
          exact fun hbinary => hr ((hlayout i).mpr hbinary)
        simp [hne]
    _ = ∑ i ∈ witness.members,
          if (witness.target source).domains i = .binary then state i else 0 := by
      exact Finset.sum_coe_sort witness.members
        (fun i =>
          if (witness.target source).domains i = .binary then state i else 0)

theorem freshSelectorSum_eq
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
    _ = ∑ i : witness.Member,
          witness.freshContribution state selectors i := by
      symm
      simpa [freshContribution] using
        (Finset.sum_ite_mem_eq witness.freshMembers
          (witness.freshSelectorState state selectors))

theorem selectorSum_eq_plannedSelector
    (hlayout : witness.ReusedMembersMatchDomains source)
    (state : State n) (selectors : State witness.freshCount) :
    witness.selectorSum source state selectors =
      ∑ i, plannedSelector witness.reusedMembers (witness.memberState state)
        (witness.freshSelectorState state selectors) i := by
  rw [selectorSum, witness.retainedReusedSum_eq source hlayout state,
    witness.freshSelectorSum_eq state selectors]
  rw [← Finset.sum_add_distrib]
  apply Finset.sum_congr rfl
  intro i _
  exact (witness.plannedSelector_eq_contributions state selectors i).symm

@[simp]
theorem cardinalityConstraint_holds
    (state : State n) (selectors : State witness.freshCount) :
    (witness.cardinalityConstraint source).Holds
        (State.append state selectors) ↔
      witness.selectorSum source state selectors ≤ 1 := by
  have hsum :
      (∑ i : Fin n,
        if i ∈ witness.members ∧
            (witness.target source).domains i = .binary then state i else 0) =
        ∑ i ∈ witness.members,
          if (witness.target source).domains i = .binary then state i else 0 := by
    convert Finset.sum_ite_mem_eq witness.members
      (fun i =>
        if (witness.target source).domains i = .binary then state i else 0) using 1
    simp [ite_and]
  simp [cardinalityConstraint, cardinalityExpr, selectorSum,
    LinearConstraint.Holds, Affine.eval, State.append, Fin.sum_univ_add]
  rw [hsum]

def generatedConstraints :
    List (LinearConstraint (n + witness.freshCount)) :=
  witness.linkConstraints source ++ [witness.cardinalityConstraint source]

theorem plannedSelector_binary_of_domains
    (hlayout : witness.ReusedMembersMatchDomains source)
    (state : State n) (selectors : State witness.freshCount)
    (hretainedDomains :
      ∀ i, state i ∈ (witness.target source).domains i)
    (hselectorDomains : ∀ j, selectors j ∈ Domain.binary) :
    GenericBinaryOn Finset.univ
      (plannedSelector witness.reusedMembers (witness.memberState state)
        (witness.freshSelectorState state selectors)) := by
  intro i _
  by_cases hr : i ∈ witness.reusedMembers
  · have hdomain : (witness.target source).domains i = .binary :=
      (hlayout i).mp hr
    simpa [plannedSelector, hr, memberState, hdomain] using
      hretainedDomains i
  · have hfresh : i ∈ witness.freshMembers := by
      simp [reusedMembers] at hr
      exact hr
    simpa [plannedSelector, hr, freshSelectorState, hfresh] using
      hselectorDomains (witness.freshIndex i hfresh)

theorem linksForFresh_iff_linksForMembers
    (state : State n) (selectors : State witness.freshCount) :
    (∀ j,
      OptionalUpperLink
          ((witness.selectorBounds source).upper (witness.freshMember j))
          (state (witness.freshMember j).val) (selectors j) ∧
        OptionalLowerLink
          ((witness.selectorBounds source).lower (witness.freshMember j))
          (state (witness.freshMember j).val) (selectors j)) ↔
      ∀ i, i ∉ witness.reusedMembers →
        OptionalUpperLink ((witness.selectorBounds source).upper i)
            (witness.memberState state i)
            (witness.freshSelectorState state selectors i) ∧
          OptionalLowerLink ((witness.selectorBounds source).lower i)
            (witness.memberState state i)
            (witness.freshSelectorState state selectors i) := by
  constructor
  · intro h i hr
    have hfresh : i ∈ witness.freshMembers := by
      simp [reusedMembers] at hr
      exact hr
    have hi := h (witness.freshIndex i hfresh)
    simpa [memberState, freshSelectorState, hfresh] using hi
  · intro h j
    have hfresh : witness.freshMember j ∈ witness.freshMembers :=
      (witness.freshMembers.orderIsoOfFin rfl j).property
    have hr : witness.freshMember j ∉ witness.reusedMembers := by
      simpa [reusedMembers] using hfresh
    have hj := h (witness.freshMember j) hr
    simpa [memberState, freshSelectorState, hfresh] using hj

theorem generatedConstraints_hold_iff_plannedSelectorFormulation
    (hlayout : witness.ReusedMembersMatchDomains source)
    (state : State n) (selectors : State witness.freshCount)
    (hretainedDomains :
      ∀ i, state i ∈ (witness.target source).domains i)
    (hselectorDomains : ∀ j, selectors j ∈ Domain.binary) :
    (∀ constraint ∈ witness.generatedConstraints source,
      constraint.Holds (State.append state selectors)) ↔
      PlannedSelectorFormulation witness.reusedMembers
        (witness.selectorBounds source)
        (witness.memberState state)
        (witness.freshSelectorState state selectors) := by
  rw [generatedConstraints, List.forall_mem_append,
    List.forall_mem_singleton, witness.linkConstraints_hold_iff,
    witness.cardinalityConstraint_holds,
    witness.linksForFresh_iff_linksForMembers]
  constructor
  · rintro ⟨hlinks, hcardinality⟩
    refine ⟨witness.plannedSelector_binary_of_domains source hlayout
      state selectors hretainedDomains hselectorDomains, hlinks, ?_⟩
    rw [← witness.selectorSum_eq_plannedSelector source hlayout state selectors]
    exact hcardinality
  · rintro ⟨_, hlinks, hcardinality⟩
    refine ⟨hlinks, ?_⟩
    rw [witness.selectorSum_eq_plannedSelector source hlayout state selectors]
    exact hcardinality

theorem freshSelectors_binary_of_plannedSelectorFormulation
    (state : State n) (selectors : State witness.freshCount)
    (hformulation :
      PlannedSelectorFormulation witness.reusedMembers
        (witness.selectorBounds source)
        (witness.memberState state)
        (witness.freshSelectorState state selectors)) :
    ∀ j, selectors j ∈ Domain.binary := by
  intro j
  have hfresh : witness.freshMember j ∈ witness.freshMembers :=
    (witness.freshMembers.orderIsoOfFin rfl j).property
  have hr : witness.freshMember j ∉ witness.reusedMembers := by
    simpa [reusedMembers] using hfresh
  have hbinary := hformulation.1 (witness.freshMember j) (by simp)
  simpa [plannedSelector, hr, freshSelectorState, hfresh] using hbinary

theorem selectedHolds_of_plannedSelectorFormulation
    (hfinite : witness.FiniteMemberBounds source)
    {state : State n} {selectors : State witness.freshCount}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i)
    (hformulation :
      PlannedSelectorFormulation witness.reusedMembers
        (witness.selectorBounds source)
        (witness.memberState state)
        (witness.freshSelectorState state selectors)) :
    witness.promotedConstraint.Holds state := by
  apply (witness.genericSOS1_memberState_iff_holds state).mp
  exact plannedSelectorFormulation_project_sos1
    witness.reusedMembers (witness.selectorBounds source)
    (witness.memberState state)
    (witness.freshSelectorState state selectors)
    (witness.withinSelectorBounds_of_domains source hfinite hdomains)
    hformulation

theorem canonicalSelectorFormulation_of_selected
    (hfinite : witness.FiniteMemberBounds source)
    (hlayout : witness.ReusedMembersMatchDomains source)
    {state : State n}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i)
    (hselected : witness.promotedConstraint.Holds state) :
    PlannedSelectorFormulation witness.reusedMembers
      (witness.selectorBounds source)
      (witness.memberState state)
      (canonicalSelector (witness.memberState state)) := by
  apply canonicalSelector_plannedFormulation
  · exact witness.withinSelectorBounds_of_domains source hfinite hdomains
  · exact witness.reusedBinary_of_domains source hlayout hdomains
  · exact (witness.genericSOS1_memberState_iff_holds state).mpr hselected

end Source

end Witness

end SOS1BigM

end Instance

end OMMXProof
