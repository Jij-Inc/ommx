import OMMXProof.Instance.Extend
import OMMXProof.Instance.Transform.SOS1BigM.Formulation
import Mathlib.Tactic

/-!
# Canonical linear rows for the SOS1 Big-M selector formulation

This module defines the selector layout and canonical regular constraints shared
by SOS1 Big-M lowering and promotion.  It is independent of either transform:
lowering appends these rows, while promotion checks an existing constraint block
against the same rows.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

/-- A direction-independent layout of reused members and fresh selectors.

`freshMembers` is ordered as a `Finset`; that order determines the right-hand
selector block. Every member outside `freshMembers` is reused as its own
selector. -/
structure SelectorLayout (n : Nat) where
  members : Finset (Fin n)
  freshMembers : Finset {i // i ∈ members}

namespace SelectorLayout

/-- An SOS1 member in this layout. -/
abbrev Member (layout : SelectorLayout n) :=
  {i // i ∈ layout.members}

/-- Members reused directly as binary selectors. -/
def reusedMembers (layout : SelectorLayout n) : Finset layout.Member :=
  layout.freshMembersᶜ

/-- Number of fresh selector variables in the right block. -/
abbrev freshCount (layout : SelectorLayout n) : Nat :=
  layout.freshMembers.card

/-- The member corresponding to one fresh selector in the right block. -/
def freshMember (layout : SelectorLayout n)
    (j : Fin layout.freshCount) : layout.Member :=
  (layout.freshMembers.orderIsoOfFin rfl j).val

/-- The right-block selector index corresponding to one fresh member. -/
def freshIndex (layout : SelectorLayout n)
    (i : layout.Member) (hi : i ∈ layout.freshMembers) :
    Fin layout.freshCount :=
  (layout.freshMembers.orderIsoOfFin rfl).symm ⟨i, hi⟩

@[simp]
theorem freshMember_freshIndex (layout : SelectorLayout n)
    (i : layout.Member) (hi : i ∈ layout.freshMembers) :
    layout.freshMember (layout.freshIndex i hi) = i := by
  simp [freshMember, freshIndex]

@[simp]
theorem freshIndex_freshMember (layout : SelectorLayout n)
    (j : Fin layout.freshCount) :
    layout.freshIndex (layout.freshMember j) (by
      exact (layout.freshMembers.orderIsoOfFin rfl j).property) = j := by
  change (layout.freshMembers.orderIsoOfFin rfl).symm
    ((layout.freshMembers.orderIsoOfFin rfl) j) = j
  exact (layout.freshMembers.orderIsoOfFin rfl).symm_apply_apply j

/-- Restrict a retained state to the SOS1 members. -/
def memberState (layout : SelectorLayout n)
    (state : State n) : layout.Member → Rat :=
  fun i => state i

/-- A virtual selector tuple indexed by every SOS1 member.

At a reused member its value is ignored by `plannedSelector`; choosing the
canonical value makes the lowering encode lemma exact. -/
def freshSelectorState (layout : SelectorLayout n)
    (members : State n) (fresh : State layout.freshCount) :
    layout.Member → Rat :=
  fun i =>
    if hi : i ∈ layout.freshMembers then
      fresh (layout.freshIndex i hi)
    else
      canonicalSelector (layout.memberState members) i

/-- Filling the fresh block with canonical member indicators gives the
canonical selector tuple on the entire layout. -/
theorem freshSelectorState_canonical (layout : SelectorLayout n)
    (members : State n) :
    layout.freshSelectorState members
        (fun j =>
          canonicalSelector (layout.memberState members)
            (layout.freshMember j)) =
      canonicalSelector (layout.memberState members) := by
  funext i
  by_cases hi : i ∈ layout.freshMembers
  · simp [freshSelectorState, hi]
  · simp [freshSelectorState, hi]

/-- Retained coordinates which are reused directly as selectors. -/
def reusedIndices (layout : SelectorLayout n) : Finset (Fin n) :=
  layout.reusedMembers.map ⟨Subtype.val, Subtype.val_injective⟩

/-! ## Canonical link rows -/

def upperLink (layout : SelectorLayout n)
    (bounds : SelectorBounds layout.Member)
    (j : Fin layout.freshCount) :
    LinearConstraint (n + layout.freshCount) where
  expr := Affine.sub
    (Affine.coordinate
      (Fin.castAdd layout.freshCount (layout.freshMember j).val))
    (Affine.scale (bounds.upper (layout.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
  sense := .lessEqual

def lowerLink (layout : SelectorLayout n)
    (bounds : SelectorBounds layout.Member)
    (j : Fin layout.freshCount) :
    LinearConstraint (n + layout.freshCount) where
  expr := Affine.sub
    (Affine.scale (bounds.lower (layout.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
    (Affine.coordinate
      (Fin.castAdd layout.freshCount (layout.freshMember j).val))
  sense := .lessEqual

@[simp]
theorem upperLink_holds (layout : SelectorLayout n)
    (bounds : SelectorBounds layout.Member)
    (j : Fin layout.freshCount) (state : State n)
    (selectors : State layout.freshCount) :
    (layout.upperLink bounds j).Holds (State.append state selectors) ↔
      state (layout.freshMember j).val ≤
        bounds.upper (layout.freshMember j) * selectors j := by
  simp [upperLink, LinearConstraint.Holds, State.append]

@[simp]
theorem lowerLink_holds (layout : SelectorLayout n)
    (bounds : SelectorBounds layout.Member)
    (j : Fin layout.freshCount) (state : State n)
    (selectors : State layout.freshCount) :
    (layout.lowerLink bounds j).Holds (State.append state selectors) ↔
      bounds.lower (layout.freshMember j) * selectors j ≤
        state (layout.freshMember j).val := by
  simp [lowerLink, LinearConstraint.Holds, State.append]

def linksFor (layout : SelectorLayout n)
    (bounds : SelectorBounds layout.Member)
    (j : Fin layout.freshCount) :
    List (LinearConstraint (n + layout.freshCount)) :=
  (if 0 < bounds.upper (layout.freshMember j) then
      [layout.upperLink bounds j]
    else []) ++
    if bounds.lower (layout.freshMember j) < 0 then
      [layout.lowerLink bounds j]
    else []

def linkConstraints (layout : SelectorLayout n)
    (bounds : SelectorBounds layout.Member) :
    List (LinearConstraint (n + layout.freshCount)) :=
  (List.ofFn fun j => layout.linksFor bounds j).flatten

theorem linkConstraints_hold_iff (layout : SelectorLayout n)
    (bounds : SelectorBounds layout.Member)
    (state : State n) (selectors : State layout.freshCount) :
    (∀ constraint ∈ layout.linkConstraints bounds,
      constraint.Holds (State.append state selectors)) ↔
      ∀ j,
        OptionalUpperLink (bounds.upper (layout.freshMember j))
            (state (layout.freshMember j).val) (selectors j) ∧
          OptionalLowerLink (bounds.lower (layout.freshMember j))
            (state (layout.freshMember j).val) (selectors j) := by
  simp only [linkConstraints, List.forall_mem_flatten,
    List.forall_mem_ofFn_iff]
  constructor
  · intro h j
    have hj := h j
    by_cases hu : 0 < bounds.upper (layout.freshMember j)
    <;> by_cases hl : bounds.lower (layout.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj
  · intro h j
    have hj := h j
    by_cases hu : 0 < bounds.upper (layout.freshMember j)
    <;> by_cases hl : bounds.lower (layout.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj

/-! ## Canonical cardinality row -/

/-- Coefficients of the mixed cardinality row. Reused members remain in the
retained block, and every fresh selector contributes in the suffix block. -/
def cardinalityExpr (layout : SelectorLayout n) :
    Affine (n + layout.freshCount) where
  coeff := Fin.append
    (fun i => if i ∈ layout.reusedIndices then 1 else 0)
    (fun _ => 1)
  constant := -1

def cardinalityConstraint (layout : SelectorLayout n) :
    LinearConstraint (n + layout.freshCount) where
  expr := layout.cardinalityExpr
  sense := .lessEqual

def selectorSum (layout : SelectorLayout n)
    (state : State n) (selectors : State layout.freshCount) : Rat :=
  (∑ i ∈ layout.reusedIndices, state i) + ∑ j, selectors j

def reusedContribution (layout : SelectorLayout n)
    (state : State n) (i : layout.Member) : Rat :=
  if i ∈ layout.reusedMembers then layout.memberState state i else 0

def freshContribution (layout : SelectorLayout n)
    (state : State n) (selectors : State layout.freshCount)
    (i : layout.Member) : Rat :=
  if i ∈ layout.freshMembers then
    layout.freshSelectorState state selectors i
  else 0

theorem plannedSelector_eq_contributions (layout : SelectorLayout n)
    (state : State n) (selectors : State layout.freshCount)
    (i : layout.Member) :
    plannedSelector layout.reusedMembers (layout.memberState state)
        (layout.freshSelectorState state selectors) i =
      layout.reusedContribution state i +
        layout.freshContribution state selectors i := by
  by_cases hr : i ∈ layout.reusedMembers
  · have hf : i ∉ layout.freshMembers := by
      simpa [reusedMembers] using hr
    simp [plannedSelector, reusedContribution, freshContribution, hr, hf]
  · have hf : i ∈ layout.freshMembers := by
      simp [reusedMembers] at hr
      exact hr
    simp [plannedSelector, reusedContribution, freshContribution, hr, hf]

theorem reusedSelectorSum_eq (layout : SelectorLayout n)
    (state : State n) :
    (∑ i ∈ layout.reusedIndices, state i) =
      ∑ i : layout.Member, layout.reusedContribution state i := by
  classical
  calc
    (∑ i ∈ layout.reusedIndices, state i) =
        ∑ i ∈ layout.reusedMembers, layout.memberState state i := by
      simp [reusedIndices, memberState]
    _ = ∑ i : layout.Member, layout.reusedContribution state i := by
      simpa [reusedContribution] using
        (Finset.sum_ite_mem_eq layout.reusedMembers
          (layout.memberState state)).symm

theorem freshSelectorSum_eq (layout : SelectorLayout n)
    (state : State n) (selectors : State layout.freshCount) :
    (∑ j, selectors j) =
      ∑ i : layout.Member, layout.freshContribution state selectors i := by
  calc
    (∑ j, selectors j) =
        ∑ i : {i // i ∈ layout.freshMembers},
          layout.freshSelectorState state selectors i.val := by
      apply Fintype.sum_equiv
        (layout.freshMembers.orderIsoOfFin rfl).toEquiv
      intro j
      unfold freshSelectorState
      have hi :
          ((layout.freshMembers.orderIsoOfFin rfl).toEquiv j).val ∈
            layout.freshMembers :=
        ((layout.freshMembers.orderIsoOfFin rfl).toEquiv j).property
      rw [dif_pos hi]
      change selectors j =
        selectors ((layout.freshMembers.orderIsoOfFin rfl).symm
          ((layout.freshMembers.orderIsoOfFin rfl) j))
      exact congrArg selectors
        ((layout.freshMembers.orderIsoOfFin rfl).symm_apply_apply j).symm
    _ = ∑ i ∈ layout.freshMembers,
          layout.freshSelectorState state selectors i := by
      exact Finset.sum_coe_sort layout.freshMembers
        (layout.freshSelectorState state selectors)
    _ = ∑ i : layout.Member,
          layout.freshContribution state selectors i := by
      symm
      simpa [freshContribution] using
        (Finset.sum_ite_mem_eq layout.freshMembers
          (layout.freshSelectorState state selectors))

theorem selectorSum_eq_plannedSelector (layout : SelectorLayout n)
    (state : State n) (selectors : State layout.freshCount) :
    layout.selectorSum state selectors =
      ∑ i, plannedSelector layout.reusedMembers (layout.memberState state)
        (layout.freshSelectorState state selectors) i := by
  rw [selectorSum, layout.reusedSelectorSum_eq state,
    layout.freshSelectorSum_eq state selectors]
  rw [← Finset.sum_add_distrib]
  apply Finset.sum_congr rfl
  intro i _
  exact (layout.plannedSelector_eq_contributions state selectors i).symm

@[simp]
theorem cardinalityConstraint_holds (layout : SelectorLayout n)
    (state : State n) (selectors : State layout.freshCount) :
    layout.cardinalityConstraint.Holds (State.append state selectors) ↔
      layout.selectorSum state selectors ≤ 1 := by
  simp [cardinalityConstraint, cardinalityExpr, selectorSum,
    LinearConstraint.Holds, Affine.eval, State.append, Fin.sum_univ_add]

/-! ## Complete canonical row block -/

/-- Upper/lower links for every fresh selector, followed by cardinality. -/
def canonicalRows (layout : SelectorLayout n)
    (bounds : SelectorBounds layout.Member) :
    List (LinearConstraint (n + layout.freshCount)) :=
  layout.linkConstraints bounds ++ [layout.cardinalityConstraint]

theorem plannedSelector_binary (layout : SelectorLayout n)
    (state : State n) (selectors : State layout.freshCount)
    (hreusedBinary :
      GenericBinaryOn layout.reusedMembers (layout.memberState state))
    (hfreshBinary : ∀ j, selectors j ∈ Domain.binary) :
    GenericBinaryOn Finset.univ
      (plannedSelector layout.reusedMembers (layout.memberState state)
        (layout.freshSelectorState state selectors)) := by
  intro i _
  by_cases hr : i ∈ layout.reusedMembers
  · simpa [plannedSelector, hr] using hreusedBinary i hr
  · have hfresh : i ∈ layout.freshMembers := by
      simp [reusedMembers] at hr
      exact hr
    simpa [plannedSelector, hr, freshSelectorState, hfresh] using
      hfreshBinary (layout.freshIndex i hfresh)

theorem linksForFresh_iff_linksForMembers (layout : SelectorLayout n)
    (bounds : SelectorBounds layout.Member)
    (state : State n) (selectors : State layout.freshCount) :
    (∀ j,
      OptionalUpperLink (bounds.upper (layout.freshMember j))
          (state (layout.freshMember j).val) (selectors j) ∧
        OptionalLowerLink (bounds.lower (layout.freshMember j))
          (state (layout.freshMember j).val) (selectors j)) ↔
      ∀ i, i ∉ layout.reusedMembers →
        OptionalUpperLink (bounds.upper i)
            (layout.memberState state i)
            (layout.freshSelectorState state selectors i) ∧
          OptionalLowerLink (bounds.lower i)
            (layout.memberState state i)
            (layout.freshSelectorState state selectors i) := by
  constructor
  · intro h i hr
    have hfresh : i ∈ layout.freshMembers := by
      simp [reusedMembers] at hr
      exact hr
    have hi := h (layout.freshIndex i hfresh)
    simpa [memberState, freshSelectorState, hfresh] using hi
  · intro h j
    have hfresh : layout.freshMember j ∈ layout.freshMembers :=
      (layout.freshMembers.orderIsoOfFin rfl j).property
    have hr : layout.freshMember j ∉ layout.reusedMembers := by
      simpa [reusedMembers] using hfresh
    have hj := h (layout.freshMember j) hr
    simpa [memberState, freshSelectorState, hfresh] using hj

/-- The canonical linear rows denote exactly the abstract reused/fresh
selector formulation once reused and fresh selectors are known to be binary. -/
theorem canonicalRows_hold_iff_plannedSelectorFormulation
    (layout : SelectorLayout n) (bounds : SelectorBounds layout.Member)
    (state : State n) (selectors : State layout.freshCount)
    (hreusedBinary :
      GenericBinaryOn layout.reusedMembers (layout.memberState state))
    (hfreshBinary : ∀ j, selectors j ∈ Domain.binary) :
    (∀ constraint ∈ layout.canonicalRows bounds,
      constraint.Holds (State.append state selectors)) ↔
      PlannedSelectorFormulationHolds layout.reusedMembers bounds
        (layout.memberState state)
        (layout.freshSelectorState state selectors) := by
  rw [canonicalRows, List.forall_mem_append,
    List.forall_mem_singleton, layout.linkConstraints_hold_iff,
    layout.cardinalityConstraint_holds,
    layout.linksForFresh_iff_linksForMembers]
  constructor
  · rintro ⟨hlinks, hcardinality⟩
    refine
      ⟨layout.plannedSelector_binary state selectors hreusedBinary hfreshBinary,
        hlinks, ?_⟩
    rw [← layout.selectorSum_eq_plannedSelector state selectors]
    exact hcardinality
  · intro hformulation
    refine ⟨hformulation.nonReusedHaveLinkedSelectors, ?_⟩
    rw [layout.selectorSum_eq_plannedSelector state selectors]
    exact hformulation.selectorsSumLe1

/-- The abstract formulation forces every fresh selector to be binary. -/
theorem freshSelectors_binary_of_plannedSelectorFormulation
    (layout : SelectorLayout n) (bounds : SelectorBounds layout.Member)
    (state : State n) (selectors : State layout.freshCount)
    (hformulation :
      PlannedSelectorFormulationHolds layout.reusedMembers bounds
        (layout.memberState state)
        (layout.freshSelectorState state selectors)) :
    ∀ j, selectors j ∈ Domain.binary := by
  intro j
  have hfresh : layout.freshMember j ∈ layout.freshMembers :=
    (layout.freshMembers.orderIsoOfFin rfl j).property
  have hr : layout.freshMember j ∉ layout.reusedMembers := by
    simpa [reusedMembers] using hfresh
  have hbinary :=
    hformulation.allSelectorsAreBinary (layout.freshMember j) (by simp)
  simpa [plannedSelector, hr, freshSelectorState, hfresh] using hbinary

end SelectorLayout

end SOS1BigM

end Instance

end OMMXProof
