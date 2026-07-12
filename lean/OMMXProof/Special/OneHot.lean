import OMMXProof.Reduction
import Mathlib.Tactic

/-!
# Exact OneHot recognition

The checker accepts any exact nonzero scalar multiple of `sum xᵢ = 1`, and
separately verifies that every member has a binary domain.
-/

namespace OMMXProof

def BinaryOn (members : Finset (Fin n)) (assignment : Assignment n) : Prop :=
  ∀ i ∈ members, VariableDomain.KindHolds .binary (assignment i)

def support (members : Finset (Fin n)) (assignment : Assignment n) : Finset (Fin n) :=
  members.filter fun i => assignment i ≠ 0

def ExactlyOne (members : Finset (Fin n)) (assignment : Assignment n) : Prop :=
  (support members assignment).card = 1

/-- Canonical normalized equality `sum xᵢ - 1 = 0`. -/
def oneHotExpr (members : Finset (Fin n)) : Affine n where
  coeff := fun i => if i ∈ members then 1 else 0
  constant := -1

theorem eval_oneHotExpr (members : Finset (Fin n)) (assignment : Assignment n) :
    (oneHotExpr members).eval assignment = ∑ i ∈ members, assignment i - 1 := by
  classical
  simp [oneHotExpr, Affine.eval, sub_eq_add_neg]

/-- Untrusted structural OneHot candidate. -/
structure OneHotDraft (n : Nat) where
  members : Finset (Fin n)
  scale : Rat

def domainsBinaryOn (domains : Fin n → VariableDomain)
    (members : Finset (Fin n)) : Prop :=
  ∀ i ∈ members, (domains i).kind = .binary

instance (domains : Fin n → VariableDomain) (members : Finset (Fin n)) :
    Decidable (domainsBinaryOn domains members) := by
  unfold domainsBinaryOn
  infer_instance

/-- Exact structural checker. The source row must itself be an equality; an
inequality with the same affine expression is not a valid OneHot witness. -/
def checkOneHot (domains : Fin n → VariableDomain) (source : LinearConstraint n)
    (draft : OneHotDraft n) : Bool :=
  decide (draft.members.Nonempty ∧
      draft.scale ≠ 0 ∧
      domainsBinaryOn domains draft.members ∧
      source.sense = .equal) &&
    source.expr.same (Affine.scale draft.scale (oneHotExpr draft.members))

theorem binaryOn_of_domains {domains : Fin n → VariableDomain}
    {members : Finset (Fin n)} {assignment : Assignment n}
    (hbinary : domainsBinaryOn domains members)
    (hdomains : ∀ i, (domains i).Holds (assignment i)) :
    BinaryOn members assignment := by
  intro i hi
  have hkind := (hdomains i).1
  rw [hbinary i hi] at hkind
  exact hkind

theorem binary_sum_eq_support_card (members : Finset (Fin n))
    (assignment : Assignment n) (hbinary : BinaryOn members assignment) :
    ∑ i ∈ members, assignment i = ((support members assignment).card : Rat) := by
  classical
  induction members using Finset.induction_on with
  | empty => simp [support]
  | @insert index members hnotmem ih =>
    have htail : BinaryOn members assignment := by
      intro i hi
      exact hbinary i (Finset.mem_insert_of_mem hi)
    rcases hbinary index (Finset.mem_insert_self index members) with hzero | hone
    · have hsupport :
          support (insert index members) assignment = support members assignment := by
        ext i
        simp only [support, Finset.mem_filter, Finset.mem_insert]
        constructor
        · rintro ⟨hi | hi, hne⟩
          · exact False.elim (hne (hi ▸ hzero))
          · exact ⟨hi, hne⟩
        · rintro ⟨hi, hne⟩
          exact ⟨Or.inr hi, hne⟩
      rw [Finset.sum_insert hnotmem, hzero, zero_add, ih htail, hsupport]
    · have hsupport :
          support (insert index members) assignment =
            insert index (support members assignment) := by
        ext i
        simp only [support, Finset.mem_filter, Finset.mem_insert]
        constructor
        · rintro ⟨hi | hi, hne⟩
          · exact Or.inl hi
          · exact Or.inr ⟨hi, hne⟩
        · rintro (hi | ⟨hi, hne⟩)
          · exact ⟨Or.inl hi, by subst i; simp [hone]⟩
          · exact ⟨Or.inr hi, hne⟩
      have hnotSupport : index ∉ support members assignment := by
        intro hmem
        exact hnotmem (Finset.mem_filter.mp hmem).1
      rw [Finset.sum_insert hnotmem, hone, ih htail, hsupport,
        Finset.card_insert_of_notMem hnotSupport]
      push_cast
      ring

theorem oneHot_iff_exactlyOne (members : Finset (Fin n))
    (assignment : Assignment n) (hbinary : BinaryOn members assignment) :
    (∑ i ∈ members, assignment i = 1) ↔ ExactlyOne members assignment := by
  rw [binary_sum_eq_support_card members assignment hbinary]
  simp [ExactlyOne]

theorem checkOneHot_sound {domains : Fin n → VariableDomain}
    {source : LinearConstraint n} {draft : OneHotDraft n}
    (hcheck : checkOneHot domains source draft = true)
    {assignment : Assignment n}
    (hdomains : ∀ i, (domains i).Holds (assignment i)) :
    (source.Holds assignment ↔
      (SpecialConstraint.oneHot draft.members).Holds assignment) := by
  have houter := Bool.and_eq_true_iff.mp hcheck
  have hconditions : draft.members.Nonempty ∧
      draft.scale ≠ 0 ∧
      domainsBinaryOn domains draft.members ∧
      source.sense = .equal := by
    simpa [decide_eq_true_eq] using houter.1
  rcases hconditions with ⟨_hnonempty, hscale, hbinaryDomains, hsense⟩
  have hsame := houter.2
  have hsource : source.expr = Affine.scale draft.scale (oneHotExpr draft.members) :=
    Affine.same_sound hsame
  have hbinary := binaryOn_of_domains hbinaryDomains hdomains
  simp only [LinearConstraint.Holds, hsense]
  rw [hsource, Affine.eval_scale, eval_oneHotExpr]
  simp only [SpecialConstraint.Holds]
  constructor
  · intro hzero
    have hsum : ∑ i ∈ draft.members, assignment i = 1 := by
      apply sub_eq_zero.mp
      exact (mul_eq_zero.mp hzero).resolve_left hscale
    exact ⟨hbinary, hsum⟩
  · rintro ⟨_, hsum⟩
    rw [hsum]
    norm_num

theorem oneHot_replace_preserves {domains : Fin n → VariableDomain}
    {source : LinearConstraint n} {draft : OneHotDraft n}
    (hcheck : checkOneHot domains source draft = true)
    (base : Assignment n → Prop) (objective : Assignment n → Rat)
    (sense : OptimizationSense)
    (baseDomains : ∀ {assignment}, base assignment →
      ∀ i, (domains i).Holds (assignment i)) :
    IdentityPreserves
      (replaceProblem base (fun assignment => source.Holds assignment)
        objective sense)
      (replaceProblem base
        (fun assignment =>
          (SpecialConstraint.oneHot draft.members).Holds assignment)
        objective sense) := by
  apply replace_preserves
  intro assignment hbase
  exact checkOneHot_sound hcheck (baseDomains hbase)

end OMMXProof
