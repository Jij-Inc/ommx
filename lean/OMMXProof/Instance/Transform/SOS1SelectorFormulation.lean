import OMMXProof.Constraint.SOS1
import OMMXProof.Domain
import Mathlib.Tactic

/-!
# SOS1 selector formulation

This module gives the direction-independent selector semantics shared by SOS1
lowering and promotion. The formulation may reuse binary SOS1 members as
selectors, introduce fresh selectors for the remaining members, and omit a
link side whose bound is zero.
-/

namespace OMMXProof

namespace Instance

namespace SOS1SelectorFormulation

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
  if 0 < upper then member ≤ upper * selector else True

def OptionalLowerLink (lower member selector : Rat) : Prop :=
  if lower < 0 then lower * selector ≤ member else True

/-- Exact formulation of a mixed reused/fresh selector layout. -/
def PlannedSelectorFormulation [Fintype ι] [DecidableEq ι]
    (reused : Finset ι) (bounds : SelectorBounds ι)
    (members freshSelectors : ι → Rat) : Prop :=
  GenericBinaryOn Finset.univ (plannedSelector reused members freshSelectors) ∧
    (∀ i, i ∉ reused →
      OptionalUpperLink (bounds.upper i) (members i) (freshSelectors i) ∧
        OptionalLowerLink (bounds.lower i) (members i) (freshSelectors i)) ∧
    ∑ i, plannedSelector reused members freshSelectors i ≤ 1

instance [Fintype ι] [DecidableEq ι] (reused : Finset ι)
    (bounds : SelectorBounds ι) (members freshSelectors : ι → Rat) :
    Decidable
      (PlannedSelectorFormulation reused bounds members freshSelectors) := by
  unfold PlannedSelectorFormulation GenericBinaryOn OptionalUpperLink
    OptionalLowerLink
  infer_instance

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
      simp [OptionalUpperLink, hemitted, hselector] at h
      exact h
    · exact le_trans (hbound i).2 (le_of_not_gt hemitted)
  have hlower : 0 ≤ members i := by
    by_cases hemitted : bounds.lower i < 0
    · have h := hlinks.2
      simp [OptionalLowerLink, hemitted, hselector] at h
      exact h
    · exact le_trans (le_of_not_gt hemitted) (hbound i).1
  exact le_antisymm hupper hlower

/-- Projecting a valid mixed selector formulation recovers the original SOS1 condition. -/
theorem plannedSelectorFormulation_project_sos1
    [Fintype ι] [DecidableEq ι]
    (reused : Finset ι) (bounds : SelectorBounds ι)
    (members freshSelectors : ι → Rat)
    (hbound : WithinSelectorBounds bounds members)
    (hformulation :
      PlannedSelectorFormulation reused bounds members freshSelectors) :
    GenericSOS1 members := by
  rcases hformulation with ⟨hbinary, hlinks, hsum⟩
  have hselectorSOS1 : GenericSOS1 (plannedSelector reused members freshSelectors) := by
    unfold GenericSOS1
    have hcardRat :
        ((genericSupport Finset.univ
          (plannedSelector reused members freshSelectors)).card : Rat) ≤ 1 := by
      rw [← generic_binary_sum_eq_support_card Finset.univ
        (plannedSelector reused members freshSelectors) hbinary]
      exact hsum
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
        (hlinks i hreused) hselector)
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
    PlannedSelectorFormulation reused bounds members
      (canonicalSelector members) := by
  have hplanned := plannedSelector_canonical reused members hreusedBinary
  refine ⟨?_, ?_, ?_⟩
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

end SOS1SelectorFormulation

end Instance

end OMMXProof
