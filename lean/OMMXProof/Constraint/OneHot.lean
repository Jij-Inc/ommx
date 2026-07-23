import OMMXProof.Reduction
import OMMXProof.Constraint.Linear
import Mathlib.Tactic

/-!
# Exact OneHot recognition

The checker accepts any exact nonzero scalar multiple of `sum xᵢ = 1`, and
separately verifies that every member has a binary domain.
-/

namespace OMMXProof

structure OneHotConstraint (n : Nat) where
  members : Finset (Fin n)

namespace OneHotConstraint

def Holds (constraint : OneHotConstraint n) (state : State n) : Prop :=
  (∀ i ∈ constraint.members, state i ∈ Domain.binary) ∧
    ∑ i ∈ constraint.members, state i = 1

instance (constraint : OneHotConstraint n) (state : State n) :
    Decidable (constraint.Holds state) := by
  unfold Holds
  infer_instance

end OneHotConstraint

private theorem agree_on_oneHot_members
    {constraint : OneHotConstraint n} {privateSet : Finset (Fin n)}
    {lhs rhs : State n}
    (hindependent : ∀ i ∈ privateSet, i ∉ constraint.members)
    (hagree : AgreeOutside privateSet lhs rhs) :
    ∀ i ∈ constraint.members, lhs i = rhs i := by
  intro i himember
  apply hagree i
  intro hiprivate
  exact hindependent i hiprivate himember

namespace OneHotConstraint

def IndependentAt (constraint : OneHotConstraint n) (index : Fin n) : Prop :=
  index ∉ constraint.members

instance (constraint : OneHotConstraint n) (index : Fin n) :
    Decidable (constraint.IndependentAt index) := by
  unfold IndependentAt
  infer_instance

def IndependentOf (constraint : OneHotConstraint n)
    (privateSet : Finset (Fin n)) : Prop :=
  ∀ i ∈ privateSet, constraint.IndependentAt i

theorem holds_iff_of_independentOf {constraint : OneHotConstraint n}
    {privateSet : Finset (Fin n)} {lhs rhs : State n}
    (hindependent : constraint.IndependentOf privateSet)
    (hagree : AgreeOutside privateSet lhs rhs) :
    constraint.Holds lhs ↔ constraint.Holds rhs := by
  have hvalues := agree_on_oneHot_members hindependent hagree
  constructor
  · rintro ⟨hbinary, hsum⟩
    exact ⟨fun i hi => by simpa [← hvalues i hi] using hbinary i hi,
      by
        calc
          ∑ i ∈ constraint.members, rhs i =
              ∑ i ∈ constraint.members, lhs i := by
            apply Finset.sum_congr rfl
            intro i hi
            rw [hvalues i hi]
          _ = 1 := hsum⟩
  · rintro ⟨hbinary, hsum⟩
    exact ⟨fun i hi => by simpa [hvalues i hi] using hbinary i hi,
      by
        calc
          ∑ i ∈ constraint.members, lhs i =
              ∑ i ∈ constraint.members, rhs i := by
            apply Finset.sum_congr rfl
            intro i hi
            rw [hvalues i hi]
          _ = 1 := hsum⟩

end OneHotConstraint

def BinaryOn (members : Finset (Fin n)) (state : State n) : Prop :=
  ∀ i ∈ members, state i ∈ Domain.binary

def support (members : Finset (Fin n)) (state : State n) : Finset (Fin n) :=
  members.filter fun i => state i ≠ 0

def ExactlyOne (members : Finset (Fin n)) (state : State n) : Prop :=
  (support members state).card = 1

/-- Canonical normalized equality `sum xᵢ - 1 = 0`. -/
def oneHotExpr (members : Finset (Fin n)) : Affine n where
  coeff := fun i => if i ∈ members then 1 else 0
  constant := -1

theorem eval_oneHotExpr (members : Finset (Fin n)) (state : State n) :
    (oneHotExpr members).eval state = ∑ i ∈ members, state i - 1 := by
  classical
  simp [oneHotExpr, Affine.eval, sub_eq_add_neg]

/-- Untrusted structural OneHot candidate. -/
structure OneHotDraft (n : Nat) where
  members : Finset (Fin n)
  scale : Rat

def domainsBinaryOn (domains : Fin n → Domain)
    (members : Finset (Fin n)) : Prop :=
  ∀ i ∈ members, domains i = .binary

instance (domains : Fin n → Domain) (members : Finset (Fin n)) :
    Decidable (domainsBinaryOn domains members) := by
  unfold domainsBinaryOn
  infer_instance

/-- Exact structural checker. The source row must itself be an equality; an
inequality with the same affine expression is not a valid OneHot witness. -/
def checkOneHot (domains : Fin n → Domain) (source : LinearConstraint n)
    (draft : OneHotDraft n) : Bool :=
  decide (draft.members.Nonempty ∧
      draft.scale ≠ 0 ∧
      domainsBinaryOn domains draft.members ∧
      source.sense = .equal) &&
    source.expr.same (Affine.scale draft.scale (oneHotExpr draft.members))

theorem binaryOn_of_domains {domains : Fin n → Domain}
    {members : Finset (Fin n)} {state : State n}
    (hbinary : domainsBinaryOn domains members)
    (hdomains : ∀ i, state i ∈ domains i) :
    BinaryOn members state := by
  intro i hi
  have hvalue := hdomains i
  rw [hbinary i hi] at hvalue
  exact hvalue

theorem binary_sum_eq_support_card (members : Finset (Fin n))
    (state : State n) (hbinary : BinaryOn members state) :
    ∑ i ∈ members, state i = ((support members state).card : Rat) := by
  classical
  induction members using Finset.induction_on with
  | empty => simp [support]
  | @insert index members hnotmem ih =>
    have htail : BinaryOn members state := by
      intro i hi
      exact hbinary i (Finset.mem_insert_of_mem hi)
    rcases hbinary index (Finset.mem_insert_self index members) with hzero | hone
    · have hsupport :
          support (insert index members) state = support members state := by
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
          support (insert index members) state =
            insert index (support members state) := by
        ext i
        simp only [support, Finset.mem_filter, Finset.mem_insert]
        constructor
        · rintro ⟨hi | hi, hne⟩
          · exact Or.inl hi
          · exact Or.inr ⟨hi, hne⟩
        · rintro (hi | ⟨hi, hne⟩)
          · exact ⟨Or.inl hi, by subst i; simp [hone]⟩
          · exact ⟨Or.inr hi, hne⟩
      have hnotSupport : index ∉ support members state := by
        intro hmem
        exact hnotmem (Finset.mem_filter.mp hmem).1
      rw [Finset.sum_insert hnotmem, hone, ih htail, hsupport,
        Finset.card_insert_of_notMem hnotSupport]
      push_cast
      ring

theorem oneHot_iff_exactlyOne (members : Finset (Fin n))
    (state : State n) (hbinary : BinaryOn members state) :
    (∑ i ∈ members, state i = 1) ↔ ExactlyOne members state := by
  rw [binary_sum_eq_support_card members state hbinary]
  simp [ExactlyOne]

theorem checkOneHot_sound {domains : Fin n → Domain}
    {source : LinearConstraint n} {draft : OneHotDraft n}
    (hcheck : checkOneHot domains source draft = true)
    {state : State n}
    (hdomains : ∀ i, state i ∈ domains i) :
    (source.Holds state ↔
      ({ members := draft.members } : OneHotConstraint n).Holds state) := by
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
  simp only [OneHotConstraint.Holds]
  constructor
  · intro hzero
    have hsum : ∑ i ∈ draft.members, state i = 1 := by
      apply sub_eq_zero.mp
      exact (mul_eq_zero.mp hzero).resolve_left hscale
    exact ⟨hbinary, hsum⟩
  · rintro ⟨_, hsum⟩
    rw [hsum]
    norm_num

theorem oneHot_replace_preserves {domains : Fin n → Domain}
    {source : LinearConstraint n} {draft : OneHotDraft n}
    (hcheck : checkOneHot domains source draft = true)
    (base : State n → Prop) (objective : State n → Rat)
    (sense : OptimizationSense)
    (baseDomains : ∀ {state}, base state →
      ∀ i, state i ∈ domains i) :
    IdentityPreserves
      (replaceProblem base (fun state => source.Holds state)
        objective sense)
      (replaceProblem base
        (fun state =>
          ({ members := draft.members } : OneHotConstraint n).Holds state)
        objective sense) := by
  apply replace_preserves
  intro state hbase
  exact checkOneHot_sound hcheck (baseDomains hbase)

end OMMXProof
