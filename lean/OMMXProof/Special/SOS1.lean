import OMMXProof.Special.OneHot
import Mathlib.Tactic.Linarith

/-!
# SOS1 semantics and selector compression

The structural binary-cardinality checker is executable. The selector-gadget
theorem proves projection/lift equivalence for an all-fresh, fully linked gadget;
history-specific inverse lowering remains a separate future refinement theorem.
-/

namespace OMMXProof

def SOS1Card (members : Finset (Fin n)) (assignment : Assignment n) : Prop :=
  (support members assignment).card ≤ 1

theorem sos1Card_iff_special (members : Finset (Fin n))
    (assignment : Assignment n) :
    SOS1Card members assignment ↔
      (SpecialConstraint.sos1 members).Holds assignment := by
  classical
  rw [SOS1Card, Finset.card_le_one]
  simp only [SpecialConstraint.Holds, support, Finset.mem_filter]
  constructor
  · intro h i hi j hj hine hjne
    exact h i ⟨hi, hine⟩ j ⟨hj, hjne⟩
  · intro h i hi j hj
    exact h i hi.1 j hj.1 hi.2 hj.2

theorem binary_cardinality_sos1 (members : Finset (Fin n))
    (assignment : Assignment n) (hbinary : BinaryOn members assignment) :
    (∑ i ∈ members, assignment i ≤ 1) ↔ SOS1Card members assignment := by
  rw [binary_sum_eq_support_card members assignment hbinary]
  simp [SOS1Card]

/-- Unlike OneHot equality scaling, a scaled `≤` cardinality row requires a
strictly positive scalar so that its direction is preserved. -/
theorem scaledBinaryCardinality_sos1 (members : Finset (Fin n))
    (assignment : Assignment n) (hbinary : BinaryOn members assignment)
    (scalar : Rat) (hpositive : 0 < scalar) :
    (scalar * ((∑ i ∈ members, assignment i) - 1) ≤ 0) ↔
      SOS1Card members assignment := by
  rw [← binary_cardinality_sos1 members assignment hbinary]
  constructor
  · intro h
    have hmul :
        scalar * ((∑ i ∈ members, assignment i) - 1) ≤ scalar * 0 := by
      simpa using h
    have := le_of_mul_le_mul_left hmul hpositive
    linarith
  · intro h
    have hdiff : (∑ i ∈ members, assignment i) - 1 ≤ 0 := by linarith
    exact mul_nonpos_of_nonneg_of_nonpos (le_of_lt hpositive) hdiff

structure BinaryCardinalitySOS1Draft (n : Nat) where
  members : Finset (Fin n)
  scale : Rat

def checkBinaryCardinalitySOS1 (domains : Fin n → VariableDomain)
    (source : LinearConstraint n) (draft : BinaryCardinalitySOS1Draft n) : Bool :=
  decide (draft.members.Nonempty ∧
      0 < draft.scale ∧
      domainsBinaryOn domains draft.members ∧
      source.sense = .lessEqual) &&
    source.expr.same (Affine.scale draft.scale (oneHotExpr draft.members))

theorem checkBinaryCardinalitySOS1_sound
    {domains : Fin n → VariableDomain} {source : LinearConstraint n}
    {draft : BinaryCardinalitySOS1Draft n}
    (hcheck : checkBinaryCardinalitySOS1 domains source draft = true)
    {assignment : Assignment n}
    (hdomains : ∀ i, (domains i).Holds (assignment i)) :
    (source.Holds assignment ↔
      (SpecialConstraint.sos1 draft.members).Holds assignment) := by
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
    scaledBinaryCardinality_sos1 draft.members assignment hbinary draft.scale hpositive,
    sos1Card_iff_special]

/-! ## Executable selector-isolation contract

Selector compression is sound only when the removed selectors are private to
the selector gadget.  The following checker computes that fact from the exact
Phase A `CoreModel` syntax.  A coordinate is considered used when its domain is
restrictive, a linear/special constraint observes it, or the objective has a
nonzero coefficient.
-/

/-- Two assignments agree on every coordinate other than `privateSet`. -/
def AgreeOutside (privateSet : Finset (Fin n))
    (lhs rhs : Assignment n) : Prop :=
  ∀ i, i ∉ privateSet → lhs i = rhs i

namespace VariableDomain

/-- A domain that imposes no semantic condition on a rational coordinate. -/
def Unrestricted (domain : VariableDomain) : Prop :=
  domain.kind = .continuous ∧
    domain.bounds.lower = none ∧ domain.bounds.upper = none

instance (domain : VariableDomain) : Decidable domain.Unrestricted := by
  unfold Unrestricted
  infer_instance

theorem holds_of_unrestricted {domain : VariableDomain}
    (hunrestricted : domain.Unrestricted) (value : Rat) :
    domain.Holds value := by
  rcases hunrestricted with ⟨hkind, hlower, hupper⟩
  unfold Holds
  constructor
  · simp [hkind, KindHolds]
  · unfold Bounds.Holds
    rw [hlower, hupper]
    exact ⟨trivial, trivial⟩

end VariableDomain

namespace Affine

/-- Syntactic exactness criterion for semantic independence at one coordinate. -/
def IndependentAt (expr : Affine n) (index : Fin n) : Prop :=
  expr.coeff index = 0

instance (expr : Affine n) (index : Fin n) :
    Decidable (expr.IndependentAt index) := by
  unfold IndependentAt
  infer_instance

def IndependentOf (expr : Affine n) (privateSet : Finset (Fin n)) : Prop :=
  ∀ i ∈ privateSet, expr.IndependentAt i

instance (expr : Affine n) (privateSet : Finset (Fin n)) :
    Decidable (expr.IndependentOf privateSet) := by
  unfold IndependentOf
  infer_instance

theorem eval_eq_of_independentOf {expr : Affine n}
    {privateSet : Finset (Fin n)} {lhs rhs : Assignment n}
    (hindependent : expr.IndependentOf privateSet)
    (hagree : AgreeOutside privateSet lhs rhs) :
    expr.eval lhs = expr.eval rhs := by
  unfold eval
  apply congrArg (fun total => total + expr.constant)
  apply Finset.sum_congr rfl
  intro i _
  by_cases hprivate : i ∈ privateSet
  · rw [hindependent i hprivate]
    simp
  · rw [hagree i hprivate]

end Affine

namespace LinearSystem

def IndependentAt (system : LinearSystem n) (index : Fin n) : Prop :=
  (∀ row, (system.inequalities row).IndependentAt index) ∧
    ∀ row, (system.equalities row).IndependentAt index

instance (system : LinearSystem n) (index : Fin n) :
    Decidable (system.IndependentAt index) := by
  unfold IndependentAt
  infer_instance

def IndependentOf (system : LinearSystem n)
    (privateSet : Finset (Fin n)) : Prop :=
  ∀ i ∈ privateSet, system.IndependentAt i

theorem feasible_iff_of_independentOf {system : LinearSystem n}
    {privateSet : Finset (Fin n)} {lhs rhs : Assignment n}
    (hindependent : system.IndependentOf privateSet)
    (hagree : AgreeOutside privateSet lhs rhs) :
    system.Feasible lhs ↔ system.Feasible rhs := by
  have hineq (row : Fin system.ineqCount) :
      (system.inequalities row).eval lhs =
        (system.inequalities row).eval rhs :=
    Affine.eval_eq_of_independentOf
      (fun i hi => (hindependent i hi).1 row) hagree
  have heq (row : Fin system.eqCount) :
      (system.equalities row).eval lhs =
        (system.equalities row).eval rhs :=
    Affine.eval_eq_of_independentOf
      (fun i hi => (hindependent i hi).2 row) hagree
  constructor
  · rintro ⟨hleftIneq, hleftEq⟩
    exact ⟨fun row => (hineq row) ▸ hleftIneq row,
      fun row => (heq row) ▸ hleftEq row⟩
  · rintro ⟨hrightIneq, hrightEq⟩
    exact ⟨fun row => (hineq row).symm ▸ hrightIneq row,
      fun row => (heq row).symm ▸ hrightEq row⟩

end LinearSystem

namespace LinearConstraint

def IndependentAt (constraint : LinearConstraint n) (index : Fin n) : Prop :=
  constraint.expr.IndependentAt index

instance (constraint : LinearConstraint n) (index : Fin n) :
    Decidable (constraint.IndependentAt index) := by
  unfold IndependentAt
  infer_instance

def IndependentOf (constraint : LinearConstraint n)
    (privateSet : Finset (Fin n)) : Prop :=
  constraint.expr.IndependentOf privateSet

theorem holds_iff_of_independentOf {constraint : LinearConstraint n}
    {privateSet : Finset (Fin n)} {lhs rhs : Assignment n}
    (hindependent : constraint.IndependentOf privateSet)
    (hagree : AgreeOutside privateSet lhs rhs) :
    constraint.Holds lhs ↔ constraint.Holds rhs := by
  have heval := Affine.eval_eq_of_independentOf hindependent hagree
  unfold Holds
  cases constraint.sense
  · exact heval ▸ Iff.rfl
  · exact heval ▸ Iff.rfl

end LinearConstraint

namespace SpecialConstraint

def IndependentAt : SpecialConstraint n → Fin n → Prop
  | .oneHot members, index => index ∉ members
  | .indicator trigger _ body, index =>
      index ≠ trigger ∧ body.IndependentAt index
  | .sos1 members, index => index ∉ members

instance (constraint : SpecialConstraint n) (index : Fin n) :
    Decidable (constraint.IndependentAt index) := by
  cases constraint with
  | oneHot members =>
      simp only [IndependentAt]
      infer_instance
  | indicator trigger polarity body =>
      simp only [IndependentAt]
      infer_instance
  | sos1 members =>
      simp only [IndependentAt]
      infer_instance

def IndependentOf (constraint : SpecialConstraint n)
    (privateSet : Finset (Fin n)) : Prop :=
  ∀ i ∈ privateSet, constraint.IndependentAt i

instance (constraint : SpecialConstraint n) (privateSet : Finset (Fin n)) :
    Decidable (constraint.IndependentOf privateSet) := by
  unfold IndependentOf
  infer_instance

private theorem agree_on_members {members privateSet : Finset (Fin n)}
    {lhs rhs : Assignment n}
    (hindependent : ∀ i ∈ privateSet, i ∉ members)
    (hagree : AgreeOutside privateSet lhs rhs) :
    ∀ i ∈ members, lhs i = rhs i := by
  intro i himember
  apply hagree i
  intro hiprivate
  exact hindependent i hiprivate himember

theorem holds_iff_of_independentOf {constraint : SpecialConstraint n}
    {privateSet : Finset (Fin n)} {lhs rhs : Assignment n}
    (hindependent : constraint.IndependentOf privateSet)
    (hagree : AgreeOutside privateSet lhs rhs) :
    constraint.Holds lhs ↔ constraint.Holds rhs := by
  cases constraint with
  | oneHot members =>
      have hvalues := agree_on_members hindependent hagree
      simp only [Holds]
      constructor
      · rintro ⟨hbinary, hsum⟩
        constructor
        · intro i hi
          simpa [← hvalues i hi] using hbinary i hi
        · calc
            ∑ i ∈ members, rhs i = ∑ i ∈ members, lhs i := by
              apply Finset.sum_congr rfl
              intro i hi
              rw [hvalues i hi]
            _ = 1 := hsum
      · rintro ⟨hbinary, hsum⟩
        constructor
        · intro i hi
          simpa [hvalues i hi] using hbinary i hi
        · calc
            ∑ i ∈ members, lhs i = ∑ i ∈ members, rhs i := by
              apply Finset.sum_congr rfl
              intro i hi
              rw [hvalues i hi]
            _ = 1 := hsum
  | indicator trigger polarity body =>
      have htriggerOutside : trigger ∉ privateSet := by
        intro hprivate
        exact (hindependent trigger hprivate).1 rfl
      have htrigger := hagree trigger htriggerOutside
      have hbodyIndependent : body.IndependentOf privateSet :=
        fun i hi => (hindependent i hi).2
      have hbody := LinearConstraint.holds_iff_of_independentOf
        hbodyIndependent hagree
      simp only [Holds]
      constructor
      · intro hleft hactive
        apply hbody.mp
        apply hleft
        simpa [htrigger] using hactive
      · intro hright hactive
        apply hbody.mpr
        apply hright
        simpa [htrigger] using hactive
  | sos1 members =>
      have hvalues := agree_on_members hindependent hagree
      simp only [Holds]
      constructor
      · intro hleft i hi j hj hir hjr
        apply hleft i hi j hj
        · simpa [hvalues i hi] using hir
        · simpa [hvalues j hj] using hjr
      · intro hright i hi j hj hil hjl
        apply hright i hi j hj
        · simpa [hvalues i hi] using hil
        · simpa [hvalues j hj] using hjl

end SpecialConstraint

namespace CoreModel

/-- Exact semantic independence of one coordinate in the Phase A model AST. -/
def IndependentAt (model : CoreModel n) (index : Fin n) : Prop :=
  (model.domains index).Unrestricted ∧
    model.linear.IndependentAt index ∧
    (∀ constraint ∈ model.specialConstraints,
      constraint.IndependentAt index) ∧
    model.objective.IndependentAt index

instance (model : CoreModel n) (index : Fin n) :
    Decidable (model.IndependentAt index) := by
  unfold IndependentAt
  infer_instance

/-- Executable semantic-use set. A selector is fresh for the base model exactly
when it is absent from this set. -/
def usedVariables (model : CoreModel n) : Finset (Fin n) :=
  Finset.univ.filter fun index => ¬model.IndependentAt index

structure SelectorIsolationWitness (n : Nat) where
  privateSelectors : Finset (Fin n)

def SelectorIsolated (model : CoreModel n)
    (witness : SelectorIsolationWitness n) : Prop :=
  witness.privateSelectors.Nonempty ∧
    Disjoint witness.privateSelectors model.usedVariables

instance (model : CoreModel n) (witness : SelectorIsolationWitness n) :
    Decidable (model.SelectorIsolated witness) := by
  unfold SelectorIsolated
  infer_instance

/-- Check an untrusted set of claimed all-fresh selector coordinates. -/
def checkSelectorIsolation (model : CoreModel n)
    (witness : SelectorIsolationWitness n) : Bool :=
  decide (model.SelectorIsolated witness)

theorem independentAt_of_selectorIsolated {model : CoreModel n}
    {witness : SelectorIsolationWitness n}
    (hisolated : model.SelectorIsolated witness)
    {index : Fin n} (hprivate : index ∈ witness.privateSelectors) :
    model.IndependentAt index := by
  by_contra hdependent
  have hused : index ∈ model.usedVariables := by
    simp [usedVariables, hdependent]
  exact Finset.disjoint_left.mp hisolated.2 hprivate hused

theorem feasible_iff_of_selectorIsolated {model : CoreModel n}
    {witness : SelectorIsolationWitness n}
    (hisolated : model.SelectorIsolated witness)
    {lhs rhs : Assignment n}
    (hagree : AgreeOutside witness.privateSelectors lhs rhs) :
    model.Feasible lhs ↔ model.Feasible rhs := by
  have hindependent (i : Fin n) (hi : i ∈ witness.privateSelectors) :=
    independentAt_of_selectorIsolated hisolated hi
  have hdomains :
      (∀ i, (model.domains i).Holds (lhs i)) ↔
        ∀ i, (model.domains i).Holds (rhs i) := by
    constructor
    · intro hleft i
      by_cases hprivate : i ∈ witness.privateSelectors
      · exact VariableDomain.holds_of_unrestricted
          (hindependent i hprivate).1 (rhs i)
      · simpa [← hagree i hprivate] using hleft i
    · intro hright i
      by_cases hprivate : i ∈ witness.privateSelectors
      · exact VariableDomain.holds_of_unrestricted
          (hindependent i hprivate).1 (lhs i)
      · simpa [hagree i hprivate] using hright i
  have hlinear := LinearSystem.feasible_iff_of_independentOf
    (fun i hi => (hindependent i hi).2.1) hagree
  have hspecial (constraint : SpecialConstraint n)
      (hconstraint : constraint ∈ model.specialConstraints) :=
    SpecialConstraint.holds_iff_of_independentOf
      (fun i hi => (hindependent i hi).2.2.1 constraint hconstraint) hagree
  unfold Feasible
  constructor
  · rintro ⟨hleftDomains, hleftLinear, hleftSpecial⟩
    exact ⟨hdomains.mp hleftDomains, hlinear.mp hleftLinear,
      fun constraint hconstraint =>
        (hspecial constraint hconstraint).mp
          (hleftSpecial constraint hconstraint)⟩
  · rintro ⟨hrightDomains, hrightLinear, hrightSpecial⟩
    exact ⟨hdomains.mpr hrightDomains, hlinear.mpr hrightLinear,
      fun constraint hconstraint =>
        (hspecial constraint hconstraint).mpr
          (hrightSpecial constraint hconstraint)⟩

theorem objective_eq_of_selectorIsolated {model : CoreModel n}
    {witness : SelectorIsolationWitness n}
    (hisolated : model.SelectorIsolated witness)
    {lhs rhs : Assignment n}
    (hagree : AgreeOutside witness.privateSelectors lhs rhs) :
    model.ObjectiveValue lhs = model.ObjectiveValue rhs := by
  apply Affine.eval_eq_of_independentOf
  · intro i hi
    exact (independentAt_of_selectorIsolated hisolated hi).2.2.2
  · exact hagree

theorem checkSelectorIsolation_sound {model : CoreModel n}
    {witness : SelectorIsolationWitness n}
    (hcheck : checkSelectorIsolation model witness = true)
    {lhs rhs : Assignment n}
    (hagree : AgreeOutside witness.privateSelectors lhs rhs) :
    model.Feasible lhs ↔ model.Feasible rhs := by
  apply feasible_iff_of_selectorIsolated
  · simpa [checkSelectorIsolation, decide_eq_true_eq] using hcheck
  · exact hagree

theorem checkSelectorIsolation_objective_sound {model : CoreModel n}
    {witness : SelectorIsolationWitness n}
    (hcheck : checkSelectorIsolation model witness = true)
    {lhs rhs : Assignment n}
    (hagree : AgreeOutside witness.privateSelectors lhs rhs) :
    model.ObjectiveValue lhs = model.ObjectiveValue rhs := by
  apply objective_eq_of_selectorIsolated
  · simpa [checkSelectorIsolation, decide_eq_true_eq] using hcheck
  · exact hagree

end CoreModel

def GenericBinaryOn (members : Finset ι) (assignment : ι → Rat) : Prop :=
  ∀ i ∈ members, VariableDomain.KindHolds .binary (assignment i)

def genericSupport [DecidableEq ι]
    (members : Finset ι) (assignment : ι → Rat) : Finset ι :=
  members.filter fun i => assignment i ≠ 0

def GenericSOS1 [Fintype ι] [DecidableEq ι] (members : ι → Rat) : Prop :=
  (genericSupport Finset.univ members).card ≤ 1

theorem generic_binary_sum_eq_support_card [DecidableEq ι]
    (members : Finset ι) (assignment : ι → Rat)
    (hbinary : GenericBinaryOn members assignment) :
    ∑ i ∈ members, assignment i = ((genericSupport members assignment).card : Rat) := by
  classical
  induction members using Finset.induction_on with
  | empty => simp [genericSupport]
  | @insert index rest hnotmem ih =>
    have htail : GenericBinaryOn rest assignment := by
      intro i hi
      exact hbinary i (Finset.mem_insert_of_mem hi)
    rcases hbinary index (Finset.mem_insert_self index rest) with hzero | hone
    · have hsupport :
          genericSupport (insert index rest) assignment =
            genericSupport rest assignment := by
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
          genericSupport (insert index rest) assignment =
            insert index (genericSupport rest assignment) := by
        ext i
        simp only [genericSupport, Finset.mem_filter, Finset.mem_insert]
        constructor
        · rintro ⟨hi | hi, hne⟩
          · exact Or.inl hi
          · exact Or.inr ⟨hi, hne⟩
        · rintro (hi | ⟨hi, hne⟩)
          · exact ⟨Or.inl hi, by subst i; simp [hone]⟩
          · exact ⟨Or.inr hi, hne⟩
      have hnotSupport : index ∉ genericSupport rest assignment := by
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
isolation by type; the finite `CoreModel` compression theorem additionally
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
    VariableDomain.KindHolds .binary (canonicalSelector members i) := by
  by_cases hzero : members i = 0 <;>
    simp [canonicalSelector, VariableDomain.KindHolds, hzero]

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
    Problem ((ι → Rat) × (ι → Rat)) where
  feasible pair := base pair.1 ∧ SelectorGadget bounds pair.1 pair.2
  objective pair := objective pair.1
  sense := sense

def sos1TargetProblem [Fintype ι] [DecidableEq ι]
    (base : (ι → Rat) → Prop) (objective : (ι → Rat) → Rat)
    (sense : OptimizationSense) : Problem (ι → Rat) where
  feasible members := base members ∧ GenericSOS1 members
  objective := objective
  sense := sense

/-- In the generic layer, selector isolation is encoded by construction:
`base` and `objective` cannot observe the private selector assignment. -/
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

/-! The generic theorem above makes selector isolation unrepresentable by its
types. The following connected variant starts from a finite `CoreModel` base,
checks an explicit isolation witness, and then derives the same projection
contract. `encode` records how member/selector tuples populate that finite
model; `encodingIsolation` states that changing selectors changes only the
coordinates claimed private by the executable witness. -/

def zeroSelectors (_ : ι) : Rat := 0

def coreSelectorSourceProblem [Fintype ι] [DecidableEq ι]
    (model : CoreModel n)
    (encode : ((ι → Rat) × (ι → Rat)) → Assignment n)
    (bounds : SelectorBounds ι) :
    Problem ((ι → Rat) × (ι → Rat)) where
  feasible pair := model.Feasible (encode pair) ∧
    SelectorGadget bounds pair.1 pair.2
  objective pair := model.ObjectiveValue (encode pair)
  sense := model.sense

def coreSOS1TargetProblem [Fintype ι] [DecidableEq ι]
    (model : CoreModel n)
    (encode : ((ι → Rat) × (ι → Rat)) → Assignment n) :
    Problem (ι → Rat) where
  feasible members :=
    model.Feasible (encode (members, zeroSelectors)) ∧ GenericSOS1 members
  objective members := model.ObjectiveValue (encode (members, zeroSelectors))
  sense := model.sense

/-- The encoding may vary only the coordinates named by the isolation witness
when its private selector tuple changes. -/
def EncodingRespectsIsolation {ι : Type*}
    (encode : ((ι → Rat) × (ι → Rat)) → Assignment n)
    (witness : CoreModel.SelectorIsolationWitness n) : Prop :=
  ∀ members selectors selectors',
    AgreeOutside witness.privateSelectors
      (encode (members, selectors)) (encode (members, selectors'))

/-- Exact all-fresh/full-link SOS1 compression from a checked finite base
model. This theorem is intentionally limited to freshly introduced private
selectors and the complete two-sided link gadget `Lᵢ zᵢ ≤ xᵢ ≤ Uᵢ zᵢ`.
Reused selectors and omitted links require separate refinement theorems. -/
def coreSelectorCompression [Fintype ι] [DecidableEq ι]
    (model : CoreModel n)
    (encode : ((ι → Rat) × (ι → Rat)) → Assignment n)
    (bounds : SelectorBounds ι)
    (isolation : CoreModel.SelectorIsolationWitness n)
    (isolationAccepted : model.checkSelectorIsolation isolation = true)
    (encodingIsolation : EncodingRespectsIsolation encode isolation)
    (baseBounds : ∀ {members},
      model.Feasible (encode (members, zeroSelectors)) →
        WithinSelectorBounds bounds members) :
    ProjectionPreserves
      (coreSelectorSourceProblem model encode bounds)
      (coreSOS1TargetProblem model encode) where
  project := Prod.fst
  lift members := (members, canonicalSelector members)
  project_feasible {x} h := by
    constructor
    · apply (CoreModel.checkSelectorIsolation_sound isolationAccepted
        (encodingIsolation x.1 x.2 zeroSelectors)).mp
      exact h.1
    · exact selectorGadget_project_sos1 bounds _ _ h.2
  lift_feasible {y} h := by
    constructor
    · apply (CoreModel.checkSelectorIsolation_sound isolationAccepted
        (encodingIsolation y zeroSelectors (canonicalSelector y))).mp
      exact h.1
    · exact canonicalSelector_gadget bounds y (baseBounds h.1) h.2
  project_lift _ := rfl
  objective_project {x} h := by
    exact (CoreModel.checkSelectorIsolation_objective_sound isolationAccepted
      (encodingIsolation x.1 x.2 zeroSelectors)).symm
  objective_lift {y} h := by
    exact CoreModel.checkSelectorIsolation_objective_sound isolationAccepted
      (encodingIsolation y (canonicalSelector y) zeroSelectors)
  sense_eq := rfl

/-- A feasible source gadget may contain a noncanonical private selector, so a
source-side retraction law is false in general. -/
theorem canonicalSelector_not_source_retraction :
    let bounds : SelectorBounds (Fin 1) := ⟨fun _ => -1, fun _ => 1⟩
    let members : Fin 1 → Rat := fun _ => 0
    let selectors : Fin 1 → Rat := fun _ => 1
    SelectorGadget bounds members selectors ∧
      (members, canonicalSelector members) ≠ (members, selectors) := by
  dsimp
  constructor
  · constructor
    · intro i
      simp [VariableDomain.KindHolds]
    constructor
    · intro i
      norm_num
    · norm_num [Fin.sum_univ_succ]
  · intro h
    have hselector := congrArg (fun pair => pair.2 0) h
    norm_num [canonicalSelector] at hselector

end OMMXProof
