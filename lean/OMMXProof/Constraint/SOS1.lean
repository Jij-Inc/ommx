import OMMXProof.Constraint.OneHot
import OMMXProof.Constraint.Indicator
import Mathlib.Tactic.Linarith

/-!
# SOS1 semantics and selector compression

The structural binary-cardinality checker is executable. Selector-gadget
theorems prove projection/lift equivalence both for the simple all-fresh,
fully-linked formulation and for the SDK plan with reused binary members,
fresh selectors, and omitted zero-bound links. Connecting a committed Rust
history to this independent plan remains a separate future refinement theorem.
-/

namespace OMMXProof

structure SOS1Constraint (n : Nat) where
  members : Finset (Fin n)

namespace SOS1Constraint

def Holds (constraint : SOS1Constraint n) (state : State n) : Prop :=
  ∀ i ∈ constraint.members, ∀ j ∈ constraint.members,
    state i ≠ 0 → state j ≠ 0 → i = j

instance (constraint : SOS1Constraint n) (state : State n) :
    Decidable (constraint.Holds state) := by
  unfold Holds
  infer_instance

end SOS1Constraint

def SOS1Card (members : Finset (Fin n)) (state : State n) : Prop :=
  (support members state).card ≤ 1

theorem sos1Card_iff_holds (members : Finset (Fin n))
    (state : State n) :
    SOS1Card members state ↔
      ({ members } : SOS1Constraint n).Holds state := by
  classical
  rw [SOS1Card, Finset.card_le_one]
  simp only [SOS1Constraint.Holds, support, Finset.mem_filter]
  constructor
  · intro h i hi j hj hine hjne
    exact h i ⟨hi, hine⟩ j ⟨hj, hjne⟩
  · intro h i hi j hj
    exact h i hi.1 j hj.1 hi.2 hj.2

theorem binary_cardinality_sos1 (members : Finset (Fin n))
    (state : State n) (hbinary : BinaryOn members state) :
    (∑ i ∈ members, state i ≤ 1) ↔ SOS1Card members state := by
  rw [binary_sum_eq_support_card members state hbinary]
  simp [SOS1Card]

/-- Unlike OneHot equality scaling, a scaled `≤` cardinality row requires a
strictly positive scalar so that its direction is preserved. -/
theorem scaledBinaryCardinality_sos1 (members : Finset (Fin n))
    (state : State n) (hbinary : BinaryOn members state)
    (scalar : Rat) (hpositive : 0 < scalar) :
    (scalar * ((∑ i ∈ members, state i) - 1) ≤ 0) ↔
      SOS1Card members state := by
  rw [← binary_cardinality_sos1 members state hbinary]
  constructor
  · intro h
    have hmul :
        scalar * ((∑ i ∈ members, state i) - 1) ≤ scalar * 0 := by
      simpa using h
    have := le_of_mul_le_mul_left hmul hpositive
    linarith
  · intro h
    have hdiff : (∑ i ∈ members, state i) - 1 ≤ 0 := by linarith
    exact mul_nonpos_of_nonneg_of_nonpos (le_of_lt hpositive) hdiff

structure BinaryCardinalitySOS1Draft (n : Nat) where
  members : Finset (Fin n)
  scale : Rat

def checkBinaryCardinalitySOS1 (domains : Fin n → Domain)
    (source : LinearConstraint n) (draft : BinaryCardinalitySOS1Draft n) : Bool :=
  decide (draft.members.Nonempty ∧
      0 < draft.scale ∧
      domainsBinaryOn domains draft.members ∧
      source.sense = .lessEqual) &&
    source.expr.same (Affine.scale draft.scale (oneHotExpr draft.members))

theorem checkBinaryCardinalitySOS1_sound
    {domains : Fin n → Domain} {source : LinearConstraint n}
    {draft : BinaryCardinalitySOS1Draft n}
    (hcheck : checkBinaryCardinalitySOS1 domains source draft = true)
    {state : State n}
    (hdomains : ∀ i, state i ∈ domains i) :
    (source.Holds state ↔
      ({ members := draft.members } : SOS1Constraint n).Holds state) := by
  have houter := Bool.and_eq_true_iff.mp hcheck
  have hconditions : draft.members.Nonempty ∧
      0 < draft.scale ∧
      domainsBinaryOn domains draft.members ∧
      source.sense = .lessEqual := by
    simpa [decide_eq_true_eq] using houter.1
  rcases hconditions with
    ⟨_hnonempty, hpositive, hbinaryDomains, hsense⟩
  have hsource : source.expr =
      Affine.scale draft.scale (oneHotExpr draft.members) :=
    Affine.same_sound houter.2
  have hbinary := binaryOn_of_domains hbinaryDomains hdomains
  simp only [LinearConstraint.Holds, hsense]
  rw [hsource, Affine.eval_scale, eval_oneHotExpr,
    scaledBinaryCardinality_sos1 draft.members state hbinary draft.scale hpositive,
    sos1Card_iff_holds]

/-! ## Executable selector-isolation contract

Selector compression is sound only when the removed selectors are private to
the selector gadget. The following constraint-specific lemmas support checking
that condition against the complete `Instance` syntax.
-/

private theorem agree_on_members {members privateSet : Finset (Fin n)}
    {lhs rhs : State n}
    (hindependent : ∀ i ∈ privateSet, i ∉ members)
    (hagree : AgreeOutside privateSet lhs rhs) :
    ∀ i ∈ members, lhs i = rhs i := by
  intro i himember
  apply hagree i
  intro hiprivate
  exact hindependent i hiprivate himember



namespace SOS1Constraint

def IndependentAt (constraint : SOS1Constraint n) (index : Fin n) : Prop :=
  index ∉ constraint.members

instance (constraint : SOS1Constraint n) (index : Fin n) :
    Decidable (constraint.IndependentAt index) := by
  unfold IndependentAt
  infer_instance

def IndependentOf (constraint : SOS1Constraint n)
    (privateSet : Finset (Fin n)) : Prop :=
  ∀ i ∈ privateSet, constraint.IndependentAt i

theorem holds_iff_of_independentOf {constraint : SOS1Constraint n}
    {privateSet : Finset (Fin n)} {lhs rhs : State n}
    (hindependent : constraint.IndependentOf privateSet)
    (hagree : AgreeOutside privateSet lhs rhs) :
    constraint.Holds lhs ↔ constraint.Holds rhs := by
  have hvalues := agree_on_members hindependent hagree
  constructor
  · intro hleft i hi j hj hir hjr
    apply hleft i hi j hj
    · simpa [hvalues i hi] using hir
    · simpa [hvalues j hj] using hjr
  · intro hright i hi j hj hil hjl
    apply hright i hi j hj
    · simpa [hvalues i hi] using hil
    · simpa [hvalues j hj] using hjl

end SOS1Constraint


def GenericBinaryOn (members : Finset ι) (state : ι → Rat) : Prop :=
  ∀ i ∈ members, state i ∈ Domain.binary

def genericSupport [DecidableEq ι]
    (members : Finset ι) (state : ι → Rat) : Finset ι :=
  members.filter fun i => state i ≠ 0

def GenericSOS1 [Fintype ι] [DecidableEq ι] (members : ι → Rat) : Prop :=
  (genericSupport Finset.univ members).card ≤ 1

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

/-- Full-link selector gadget. The generic problem constructors below encode
isolation by type; the finite `Instance` compression theorem additionally
checks an explicit semantic-use witness. -/
def SelectorGadget [Fintype ι] [DecidableEq ι]
    (bounds : SelectorBounds ι) (members selectors : ι → Rat) : Prop :=
  GenericBinaryOn Finset.univ selectors ∧
    (∀ i, bounds.lower i * selectors i ≤ members i ∧
      members i ≤ bounds.upper i * selectors i) ∧
    ∑ i, selectors i ≤ 1

def canonicalSelector (members : ι → Rat) : ι → Rat :=
  fun i => if members i = 0 then 0 else 1

theorem canonicalSelector_binary (members : ι → Rat) (i : ι) :
    canonicalSelector members i ∈ Domain.binary := by
  by_cases hzero : members i = 0 <;>
    simp [canonicalSelector, Membership.mem, Domain.Holds, hzero]

theorem canonicalSelector_support [Fintype ι] [DecidableEq ι]
    (members : ι → Rat) :
    genericSupport Finset.univ (canonicalSelector members) =
      genericSupport Finset.univ members := by
  ext i
  simp [genericSupport, canonicalSelector]

theorem selectorGadget_project_sos1 [Fintype ι] [DecidableEq ι]
    (bounds : SelectorBounds ι) (members selectors : ι → Rat)
    (hgadget : SelectorGadget bounds members selectors) :
    GenericSOS1 members := by
  rcases hgadget with ⟨hbinary, hlink, hsum⟩
  have hselectorSOS1 : GenericSOS1 selectors := by
    unfold GenericSOS1
    have hcardRat :
        ((genericSupport Finset.univ selectors).card : Rat) ≤ 1 := by
      rw [← generic_binary_sum_eq_support_card Finset.univ selectors hbinary]
      exact hsum
    exact_mod_cast hcardRat
  have hsubset :
      genericSupport Finset.univ members ⊆
        genericSupport Finset.univ selectors := by
    intro i hi
    simp only [genericSupport, Finset.mem_filter, Finset.mem_univ, true_and] at hi ⊢
    intro hselectorZero
    have hbounds := hlink i
    rw [hselectorZero] at hbounds
    norm_num at hbounds
    exact hi (le_antisymm hbounds.2 hbounds.1)
  exact le_trans (Finset.card_le_card hsubset) hselectorSOS1

theorem canonicalSelector_gadget [Fintype ι] [DecidableEq ι]
    (bounds : SelectorBounds ι) (members : ι → Rat)
    (hbound : WithinSelectorBounds bounds members)
    (hsos1 : GenericSOS1 members) :
    SelectorGadget bounds members (canonicalSelector members) := by
  refine ⟨?_, ?_, ?_⟩
  · intro i _
    exact canonicalSelector_binary members i
  · intro i
    by_cases hzero : members i = 0
    · simp [canonicalSelector, hzero]
    · simpa [canonicalSelector, hzero] using hbound i
  · rw [generic_binary_sum_eq_support_card Finset.univ
      (canonicalSelector members) (fun i _ => canonicalSelector_binary members i)]
    rw [canonicalSelector_support]
    exact_mod_cast hsos1

def selectorSourceProblem [Fintype ι] [DecidableEq ι]
    (bounds : SelectorBounds ι) (base : (ι → Rat) → Prop)
    (objective : (ι → Rat) → Rat) (sense : OptimizationSense) :
    SemanticProblem ((ι → Rat) × (ι → Rat)) where
  feasible pair := base pair.1 ∧ SelectorGadget bounds pair.1 pair.2
  objective pair := objective pair.1
  sense := sense

def sos1TargetProblem [Fintype ι] [DecidableEq ι]
    (base : (ι → Rat) → Prop) (objective : (ι → Rat) → Rat)
    (sense : OptimizationSense) : SemanticProblem (ι → Rat) where
  feasible members := base members ∧ GenericSOS1 members
  objective := objective
  sense := sense

/-- In the generic layer, selector isolation is encoded by construction:
`base` and `objective` cannot observe the private selector state. -/
def selectorCompression [Fintype ι] [DecidableEq ι]
    (bounds : SelectorBounds ι) (base : (ι → Rat) → Prop)
    (objective : (ι → Rat) → Rat) (sense : OptimizationSense)
    (baseBounds : ∀ {members},
      base members → WithinSelectorBounds bounds members) :
    ProjectionPreserves
      (selectorSourceProblem bounds base objective sense)
      (sos1TargetProblem base objective sense) where
  project := Prod.fst
  lift members := (members, canonicalSelector members)
  project_feasible h := ⟨h.1, selectorGadget_project_sos1 bounds _ _ h.2⟩
  lift_feasible h := ⟨h.1,
    canonicalSelector_gadget bounds _ (baseBounds h.1) h.2⟩
  project_lift _ := rfl
  objective_project _ := rfl
  objective_lift _ := rfl
  sense_eq := rfl

/-! ## SDK selector plan

The SDK may reuse a binary member as its own selector, introduce a fresh
selector for another member, and omit a link side whose bound is zero.  The
following semantics models that plan directly.  `freshSelectors` is ignored at
reused coordinates.
-/

def plannedSelector [DecidableEq ι] (reused : Finset ι)
    (members freshSelectors : ι → Rat) : ι → Rat :=
  fun i => if i ∈ reused then members i else freshSelectors i

def OptionalUpperLink (upper member selector : Rat) : Prop :=
  if 0 < upper then member ≤ upper * selector else True

def OptionalLowerLink (lower member selector : Rat) : Prop :=
  if lower < 0 then lower * selector ≤ member else True

/-- Validation performed before the SDK introduces a fresh selector. -/
def FreshBoundsContainZero [DecidableEq ι] (reused : Finset ι)
    (bounds : SelectorBounds ι) : Prop :=
  ∀ i, i ∉ reused → bounds.lower i ≤ 0 ∧ 0 ≤ bounds.upper i

instance [Fintype ι] [DecidableEq ι] (reused : Finset ι)
    (bounds : SelectorBounds ι) :
    Decidable (FreshBoundsContainZero reused bounds) := by
  unfold FreshBoundsContainZero
  infer_instance

/-- Exact plan-validation obligations enforced before the SDK mutates an
instance.  `Rat` makes finiteness intrinsic; unsupported split domains remain
outside this independent semantic model. -/
structure PlannedSelectorValidation [DecidableEq ι] (reused : Finset ι)
    (bounds : SelectorBounds ι) (base : (ι → Rat) → Prop) : Prop where
  freshBoundsContainZero : FreshBoundsContainZero reused bounds
  baseBounds : ∀ {members},
    base members → WithinSelectorBounds bounds members
  baseReusedBinary : ∀ {members},
    base members → GenericBinaryOn reused members

/-- Exact denotation of a mixed reused/fresh SDK selector plan. -/
def PlannedSelectorGadget [Fintype ι] [DecidableEq ι]
    (reused : Finset ι) (bounds : SelectorBounds ι)
    (members freshSelectors : ι → Rat) : Prop :=
  GenericBinaryOn Finset.univ (plannedSelector reused members freshSelectors) ∧
    (∀ i, i ∉ reused →
      OptionalUpperLink (bounds.upper i) (members i) (freshSelectors i) ∧
        OptionalLowerLink (bounds.lower i) (members i) (freshSelectors i)) ∧
    ∑ i, plannedSelector reused members freshSelectors i ≤ 1

instance [Fintype ι] [DecidableEq ι] (reused : Finset ι)
    (bounds : SelectorBounds ι) (members freshSelectors : ι → Rat) :
    Decidable (PlannedSelectorGadget reused bounds members freshSelectors) := by
  unfold PlannedSelectorGadget GenericBinaryOn OptionalUpperLink OptionalLowerLink
  infer_instance

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

theorem plannedSelectorGadget_project_sos1 [Fintype ι] [DecidableEq ι]
    (reused : Finset ι) (bounds : SelectorBounds ι)
    (members freshSelectors : ι → Rat)
    (hbound : WithinSelectorBounds bounds members)
    (hgadget : PlannedSelectorGadget reused bounds members freshSelectors) :
    GenericSOS1 members := by
  rcases hgadget with ⟨hbinary, hlinks, hsum⟩
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

theorem canonicalSelector_plannedGadget [Fintype ι] [DecidableEq ι]
    (reused : Finset ι) (bounds : SelectorBounds ι) (members : ι → Rat)
    (hbound : WithinSelectorBounds bounds members)
    (hreusedBinary : GenericBinaryOn reused members)
    (hsos1 : GenericSOS1 members) :
    PlannedSelectorGadget reused bounds members (canonicalSelector members) := by
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

def plannedSelectorSourceProblem [Fintype ι] [DecidableEq ι]
    (reused : Finset ι) (bounds : SelectorBounds ι)
    (base : (ι → Rat) → Prop) (objective : (ι → Rat) → Rat)
    (sense : OptimizationSense) : SemanticProblem ((ι → Rat) × (ι → Rat)) where
  feasible pair :=
    base pair.1 ∧ PlannedSelectorGadget reused bounds pair.1 pair.2
  objective pair := objective pair.1
  sense := sense

/-- Projection/lift correctness for the full SDK SOS1 plan: reused binary
members, fresh selectors, and omitted zero-bound link sides may coexist. -/
def plannedSelectorCompression [Fintype ι] [DecidableEq ι]
    (reused : Finset ι) (bounds : SelectorBounds ι)
    (base : (ι → Rat) → Prop) (objective : (ι → Rat) → Rat)
    (sense : OptimizationSense)
    (validation : PlannedSelectorValidation reused bounds base) :
    ProjectionPreserves
      (plannedSelectorSourceProblem reused bounds base objective sense)
      (sos1TargetProblem base objective sense) where
  project := Prod.fst
  lift members := (members, canonicalSelector members)
  project_feasible h := ⟨h.1,
    plannedSelectorGadget_project_sos1 reused bounds _ _
      (validation.baseBounds h.1) h.2⟩
  lift_feasible h := ⟨h.1,
    canonicalSelector_plannedGadget reused bounds _
      (validation.baseBounds h.1) (validation.baseReusedBinary h.1) h.2⟩
  project_lift _ := rfl
  objective_project _ := rfl
  objective_lift _ := rfl
  sense_eq := rfl

end OMMXProof
