import OMMXProof.Constraint.SOS1
import OMMXProof.Domain
import OMMXProof.Instance.Extend
import Mathlib.Tactic

/-!
# SOS1 Big-M selector formulation

This module defines the complete selector formulation shared by SOS1 Big-M
lowering and promotion. It includes the member-indexed semantics, the concrete
retained-prefix/fresh-selector-suffix layout, the canonical link and cardinality
rows, and the equivalence between those rows and the semantic formulation.

The formulation may reuse binary SOS1 members as selectors, introduce fresh
selectors for the remaining members, and omit a link side whose bound is zero.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

def GenericBinaryOn (members : Finset ι) (state : ι → Rat) : Prop :=
  ∀ i ∈ members, state i ∈ Domain.binary

def genericSupport [DecidableEq ι]
    (members : Finset ι) (state : ι → Rat) : Finset ι :=
  members.filter fun i => state i ≠ 0

def GenericSOS1 [Fintype ι] [DecidableEq ι] (members : ι → Rat) : Prop :=
  (genericSupport Finset.univ members).card ≤ 1

/-- The sum of binary member values equals the cardinality of their nonzero support. -/
theorem generic_binary_sum_eq_support_card [DecidableEq ι]
    (members : Finset ι) (state : ι → Rat)
    (hbinary : GenericBinaryOn members state) :
    ∑ i ∈ members, state i = ((genericSupport members state).card : Rat) := by
  classical
  induction members using Finset.induction_on with
  | empty => simp [genericSupport]
  | @insert index rest hnotmem ih =>
    have htail : GenericBinaryOn rest state := by
      intro i hi
      exact hbinary i (Finset.mem_insert_of_mem hi)
    rcases hbinary index (Finset.mem_insert_self index rest) with hzero | hone
    · have hsupport :
          genericSupport (insert index rest) state =
            genericSupport rest state := by
        ext i
        simp only [genericSupport, Finset.mem_filter, Finset.mem_insert]
        constructor
        · rintro ⟨hi | hi, hne⟩
          · exact False.elim (hne (hi ▸ hzero))
          · exact ⟨hi, hne⟩
        · rintro ⟨hi, hne⟩
          exact ⟨Or.inr hi, hne⟩
      rw [Finset.sum_insert hnotmem, hzero, zero_add, ih htail, hsupport]
    · have hsupport :
          genericSupport (insert index rest) state =
            insert index (genericSupport rest state) := by
        ext i
        simp only [genericSupport, Finset.mem_filter, Finset.mem_insert]
        constructor
        · rintro ⟨hi | hi, hne⟩
          · exact Or.inl hi
          · exact Or.inr ⟨hi, hne⟩
        · rintro (hi | ⟨hi, hne⟩)
          · exact ⟨Or.inl hi, by subst i; simp [hone]⟩
          · exact ⟨Or.inr hi, hne⟩
      have hnotSupport : index ∉ genericSupport rest state := by
        intro hmem
        exact hnotmem (Finset.mem_filter.mp hmem).1
      rw [Finset.sum_insert hnotmem, hone, ih htail, hsupport,
        Finset.card_insert_of_notMem hnotSupport]
      push_cast
      ring

structure SelectorBounds (ι : Type*) where
  lower : ι → Rat
  upper : ι → Rat

def WithinSelectorBounds (bounds : SelectorBounds ι) (members : ι → Rat) : Prop :=
  ∀ i, bounds.lower i ≤ members i ∧ members i ≤ bounds.upper i

def canonicalSelector (members : ι → Rat) : ι → Rat :=
  fun i => if members i = 0 then 0 else 1

/-- Every canonical selector is binary. -/
theorem canonicalSelector_binary (members : ι → Rat) (i : ι) :
    canonicalSelector members i ∈ Domain.binary := by
  by_cases hzero : members i = 0 <;>
    simp [canonicalSelector, Membership.mem, Domain.Holds, hzero]

/-- Canonical selectors have the same nonzero support as their members. -/
theorem canonicalSelector_support [Fintype ι] [DecidableEq ι]
    (members : ι → Rat) :
    genericSupport Finset.univ (canonicalSelector members) =
      genericSupport Finset.univ members := by
  ext i
  simp [genericSupport, canonicalSelector]

def plannedSelector [DecidableEq ι] (reused : Finset ι)
    (members freshSelectors : ι → Rat) : ι → Rat :=
  fun i => if i ∈ reused then members i else freshSelectors i

def OptionalUpperLink (upper member selector : Rat) : Prop :=
  0 < upper → member ≤ upper * selector

def OptionalLowerLink (lower member selector : Rat) : Prop :=
  lower < 0 → lower * selector ≤ member

/-- Semantic satisfaction predicate for the mixed reused/fresh selector
formulation shared by SOS1 Big-M lowering and promotion.

With valid member bounds, this formulation projects to `GenericSOS1`.
Conversely, every bounded SOS1 state whose reused members are binary satisfies
it under the canonical selector assignment. The concrete layout and canonical
linear rows defined below are equivalent to this predicate. -/
structure PlannedSelectorFormulationHolds [Fintype ι] [DecidableEq ι]
    (reused : Finset ι) (bounds : SelectorBounds ι)
    (members freshSelectors : ι → Rat) : Prop where
  allSelectorsAreBinary :
    GenericBinaryOn Finset.univ (plannedSelector reused members freshSelectors)
  nonReusedHaveLinkedSelectors :
    ∀ i, i ∉ reused →
      OptionalUpperLink (bounds.upper i) (members i) (freshSelectors i) ∧
        OptionalLowerLink (bounds.lower i) (members i) (freshSelectors i)
  selectorsSumLe1 :
    ∑ i, plannedSelector reused members freshSelectors i ≤ 1

instance [Fintype ι] [DecidableEq ι] (reused : Finset ι)
    (bounds : SelectorBounds ι) (members freshSelectors : ι → Rat) :
    Decidable
      (PlannedSelectorFormulationHolds reused bounds members freshSelectors) := by
  let proposition :=
    GenericBinaryOn Finset.univ (plannedSelector reused members freshSelectors) ∧
      (∀ i, i ∉ reused →
        OptionalUpperLink (bounds.upper i) (members i) (freshSelectors i) ∧
          OptionalLowerLink (bounds.lower i) (members i) (freshSelectors i)) ∧
      ∑ i, plannedSelector reused members freshSelectors i ≤ 1
  letI : Decidable proposition := by
    dsimp only [proposition]
    unfold GenericBinaryOn OptionalUpperLink OptionalLowerLink
    infer_instance
  exact decidable_of_iff proposition
    ⟨fun h => ⟨h.1, h.2.1, h.2.2⟩,
      fun h => ⟨h.allSelectorsAreBinary, h.nonReusedHaveLinkedSelectors,
        h.selectorsSumLe1⟩⟩

/-- A linked member is zero whenever its fresh selector is zero. -/
theorem member_eq_zero_of_fresh_selector_eq_zero [DecidableEq ι]
    {bounds : SelectorBounds ι}
    {members freshSelectors : ι → Rat} {i : ι}
    (hbound : WithinSelectorBounds bounds members)
    (hlinks :
      OptionalUpperLink (bounds.upper i) (members i) (freshSelectors i) ∧
        OptionalLowerLink (bounds.lower i) (members i) (freshSelectors i))
    (hselector : freshSelectors i = 0) :
    members i = 0 := by
  have hupper : members i ≤ 0 := by
    by_cases hemitted : 0 < bounds.upper i
    · have h := hlinks.1
      simpa [hselector] using h hemitted
    · exact le_trans (hbound i).2 (le_of_not_gt hemitted)
  have hlower : 0 ≤ members i := by
    by_cases hemitted : bounds.lower i < 0
    · have h := hlinks.2
      simpa [hselector] using h hemitted
    · exact le_trans (le_of_not_gt hemitted) (hbound i).1
  exact le_antisymm hupper hlower

/-- Projecting a valid mixed selector formulation recovers the original SOS1 condition. -/
theorem plannedSelectorFormulation_project_sos1
    [Fintype ι] [DecidableEq ι]
    (reused : Finset ι) (bounds : SelectorBounds ι)
    (members freshSelectors : ι → Rat)
    (hbound : WithinSelectorBounds bounds members)
    (hformulation :
      PlannedSelectorFormulationHolds reused bounds members freshSelectors) :
    GenericSOS1 members := by
  have hselectorSOS1 : GenericSOS1 (plannedSelector reused members freshSelectors) := by
    unfold GenericSOS1
    have hcardRat :
        ((genericSupport Finset.univ
          (plannedSelector reused members freshSelectors)).card : Rat) ≤ 1 := by
      rw [← generic_binary_sum_eq_support_card Finset.univ
        (plannedSelector reused members freshSelectors)
        hformulation.allSelectorsAreBinary]
      exact hformulation.selectorsSumLe1
    exact_mod_cast hcardRat
  have hsubset :
      genericSupport Finset.univ members ⊆
        genericSupport Finset.univ (plannedSelector reused members freshSelectors) := by
    intro i hi
    simp only [genericSupport, Finset.mem_filter, Finset.mem_univ, true_and] at hi ⊢
    by_cases hreused : i ∈ reused
    · simpa [plannedSelector, hreused] using hi
    · simp only [plannedSelector, hreused, ↓reduceIte]
      intro hselector
      exact hi (member_eq_zero_of_fresh_selector_eq_zero hbound
        (hformulation.nonReusedHaveLinkedSelectors i hreused) hselector)
  exact le_trans (Finset.card_le_card hsubset) hselectorSOS1

/-- Reusing binary members agrees with the canonical selector on every member. -/
theorem plannedSelector_canonical [Fintype ι] [DecidableEq ι]
    (reused : Finset ι) (members : ι → Rat)
    (hreusedBinary : GenericBinaryOn reused members) :
    plannedSelector reused members (canonicalSelector members) =
      canonicalSelector members := by
  funext i
  by_cases hreused : i ∈ reused
  · rcases hreusedBinary i hreused with hzero | hone
    · simp [plannedSelector, canonicalSelector, hreused, hzero]
    · simp [plannedSelector, canonicalSelector, hreused, hone]
  · simp [plannedSelector, hreused]

/-- Canonical selectors realize the planned formulation of a bounded SOS1 state. -/
theorem canonicalSelector_plannedFormulation [Fintype ι] [DecidableEq ι]
    (reused : Finset ι) (bounds : SelectorBounds ι) (members : ι → Rat)
    (hbound : WithinSelectorBounds bounds members)
    (hreusedBinary : GenericBinaryOn reused members)
    (hsos1 : GenericSOS1 members) :
    PlannedSelectorFormulationHolds reused bounds members
      (canonicalSelector members) := by
  have hplanned := plannedSelector_canonical reused members hreusedBinary
  refine {
    allSelectorsAreBinary := ?_
    nonReusedHaveLinkedSelectors := ?_
    selectorsSumLe1 := ?_
  }
  · rw [hplanned]
    intro i _
    exact canonicalSelector_binary members i
  · intro i hfresh
    by_cases hmember : members i = 0
    · simp [OptionalUpperLink, OptionalLowerLink, canonicalSelector, hmember]
    · have hb := hbound i
      simp [OptionalUpperLink, OptionalLowerLink, canonicalSelector, hmember,
        hb.1, hb.2]
  · rw [hplanned]
    rw [generic_binary_sum_eq_support_card Finset.univ
      (canonicalSelector members) (fun i _ => canonicalSelector_binary members i)]
    rw [canonicalSelector_support]
    exact_mod_cast hsos1

/-! ## Concrete selector layout and canonical rows -/

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
