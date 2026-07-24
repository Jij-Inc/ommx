import OMMXProof.Domain
import OMMXProof.State
import Mathlib.Algebra.BigOperators.Fin
import Mathlib.Algebra.BigOperators.Ring.Finset
import Mathlib.Data.Fintype.BigOperators
import Mathlib.Tactic.Ring

/-!
# Exact affine-function semantics

This module defines affine functions over exact rational states together with
their algebra, evaluation, executable equality, implication, and substitution
laws.
-/

namespace OMMXProof

structure Affine (n : Nat) where
  coeff : Fin n → Rat
  constant : Rat

namespace Affine

def zero : Affine n where
  coeff := fun _ => 0
  constant := 0

def add (lhs rhs : Affine n) : Affine n where
  coeff := fun i => lhs.coeff i + rhs.coeff i
  constant := lhs.constant + rhs.constant

def neg (expr : Affine n) : Affine n where
  coeff := fun i => -expr.coeff i
  constant := -expr.constant

def sub (lhs rhs : Affine n) : Affine n := add lhs (neg rhs)

def scale (scalar : Rat) (expr : Affine n) : Affine n where
  coeff := fun i => scalar * expr.coeff i
  constant := scalar * expr.constant

def eval (expr : Affine n) (state : State n) : Rat :=
  (∑ i, expr.coeff i * state i) + expr.constant

/-- A coefficient-free affine expression evaluates to its constant exactly
once, independently of the state-space dimension. -/
theorem eval_eq_constant_of_coeff_eq_zero {expr : Affine n}
    (hzero : ∀ i, expr.coeff i = 0) (state : State n) :
    eval expr state = expr.constant := by
  simp [eval, hzero]

/-- Executable extensional equality for a finite affine expression. -/
def Same (lhs rhs : Affine n) : Prop :=
  lhs.constant = rhs.constant ∧ ∀ i, lhs.coeff i = rhs.coeff i

instance (lhs rhs : Affine n) : Decidable (Same lhs rhs) := by
  unfold Same
  infer_instance

def same (lhs rhs : Affine n) : Bool := decide (Same lhs rhs)

theorem same_iff {lhs rhs : Affine n} : Same lhs rhs ↔ lhs = rhs := by
  constructor
  · rintro ⟨hconstant, hcoeff⟩
    cases lhs with
    | mk lhsCoeff lhsConstant =>
      cases rhs with
      | mk rhsCoeff rhsConstant =>
        simp only at hconstant hcoeff
        subst rhsConstant
        have : lhsCoeff = rhsCoeff := funext hcoeff
        subst rhsCoeff
        rfl
  · intro h
    subst rhs
    exact ⟨rfl, fun _ => rfl⟩

theorem same_sound {lhs rhs : Affine n} (hcheck : same lhs rhs = true) :
    lhs = rhs := by
  apply same_iff.mp
  simpa [same, decide_eq_true_eq] using hcheck

/-- `source` implies `target` when both denote rows `eval ≤ 0`.
The coefficient identity is exact, while the target may have scalar slack. -/
def Implies (source target : Affine n) : Prop :=
  (∀ i, source.coeff i = target.coeff i) ∧
    target.constant ≤ source.constant

instance (source target : Affine n) : Decidable (Implies source target) := by
  unfold Implies
  infer_instance

theorem eval_le_of_implies {source target : Affine n}
    (h : Implies source target) (state : State n) :
    target.eval state ≤ source.eval state := by
  unfold eval
  have hsum :
      (∑ i, target.coeff i * state i) =
        ∑ i, source.coeff i * state i := by
    apply Finset.sum_congr rfl
    intro i _
    rw [h.1 i]
  rw [hsum]
  simpa [add_comm] using
    add_le_add_left h.2 (∑ i, source.coeff i * state i)

@[simp]
theorem eval_zero (state : State n) :
    eval (zero : Affine n) state = 0 := by
  simp [eval, zero]

@[simp]
theorem eval_add (lhs rhs : Affine n) (state : State n) :
    eval (add lhs rhs) state = eval lhs state + eval rhs state := by
  simp only [eval, add, add_mul, Finset.sum_add_distrib]
  ring

@[simp]
theorem eval_neg (expr : Affine n) (state : State n) :
    eval (neg expr) state = -eval expr state := by
  simp only [eval, neg, neg_mul, Finset.sum_neg_distrib]
  ring

@[simp]
theorem eval_sub (lhs rhs : Affine n) (state : State n) :
    eval (sub lhs rhs) state = eval lhs state - eval rhs state := by
  simp [sub, sub_eq_add_neg]

@[simp]
theorem eval_scale (scalar : Rat) (expr : Affine n)
    (state : State n) :
    eval (scale scalar expr) state = scalar * eval expr state := by
  simp only [eval, scale, mul_assoc, ← Finset.mul_sum]
  ring

/-! ## Affine bounds over domain boxes -/

/-- Fold sound interval/value pairs through `Bound.add`.

This helper does not require or define arithmetic on infinite `Endpoint`
values; all interval arithmetic remains owned by `Bound`. -/
private theorem foldr_add_holds
    (terms : List (Bound × Rat))
    (hterms : ∀ term ∈ terms, term.2 ∈ term.1)
    (constant : Rat) :
    (terms.map Prod.snd).sum + constant ∈
      (terms.map Prod.fst).foldr Bound.add (Bound.point constant) := by
  induction terms with
  | nil =>
      simp [Bound.point]
  | cons term terms ih =>
      have hterm : term.2 ∈ term.1 :=
        hterms term (by simp)
      have htail : ∀ tailTerm ∈ terms, tailTerm.2 ∈ tailTerm.1 := by
        intro tailTerm htailTerm
        exact hterms tailTerm (by simp [htailTerm])
      have hrest := ih htail
      simpa [add_assoc] using Bound.add_holds hterm hrest

/-- A rational interval containing every value of an affine function over the
given decision-variable domains.

Each term is scaled and accumulated with `Bound` arithmetic. Integer domains
contribute their containing rational intervals, so this theorem makes no
tightness claim for the discrete affine image. -/
def evaluateBound (expr : Affine n)
    (domains : Fin n → Domain) : Bound :=
  (List.ofFn fun i =>
      Bound.scale (expr.coeff i) (domains i).bound).foldr
    Bound.add (Bound.point expr.constant)

/-- Every state in the supplied domains evaluates inside `evaluateBound`. -/
theorem evaluateBound_sound (expr : Affine n)
    (domains : Fin n → Domain) {state : State n}
    (hdomains : ∀ i, state i ∈ domains i) :
    expr.eval state ∈ expr.evaluateBound domains := by
  let terms : List (Bound × Rat) :=
    List.ofFn fun i =>
      (Bound.scale (expr.coeff i) (domains i).bound,
        expr.coeff i * state i)
  have hterms : ∀ term ∈ terms, term.2 ∈ term.1 := by
    dsimp only [terms]
    rw [List.forall_mem_ofFn_iff]
    intro i
    exact Bound.scale_holds (Domain.mem_bound (hdomains i))
  have hfold := foldr_add_holds terms hterms expr.constant
  simpa [terms, evaluateBound, eval, Fin.sum_ofFn,
    Function.comp_def] using hfold

/-- The affine expression selecting one coordinate. -/
def coordinate (index : Fin n) : Affine n where
  coeff := fun i => if i = index then 1 else 0
  constant := 0

@[simp]
theorem eval_coordinate (index : Fin n) (state : State n) :
    eval (coordinate index) state = state index := by
  simp [eval, coordinate]

/-- Exact substitution of one coordinate by a rational constant. The coordinate
space is retained; the substituted coefficient becomes zero. -/
def substitute (expr : Affine n) (index : Fin n) (value : Rat) : Affine n where
  coeff := fun i => if i = index then 0 else expr.coeff i
  constant := expr.constant + expr.coeff index * value

theorem eval_substitute {expr : Affine n} {index : Fin n} {value : Rat}
    {state : State n} (hvalue : state index = value) :
    eval (substitute expr index value) state = eval expr state := by
  classical
  simp only [eval, substitute, ite_mul, zero_mul]
  rw [Finset.sum_ite]
  simp only [Finset.sum_const_zero, zero_add]
  rw [Finset.filter_ne']
  rw [← hvalue]
  have hsum :
      (∑ x ∈ Finset.univ.erase index, expr.coeff x * state x) +
          expr.coeff index * state index =
        ∑ x, expr.coeff x * state x :=
    Finset.sum_erase_add Finset.univ
      (fun x => expr.coeff x * state x) (Finset.mem_univ index)
  calc
    (∑ x ∈ Finset.univ.erase index, expr.coeff x * state x) +
          (expr.constant + expr.coeff index * state index) =
        ((∑ x ∈ Finset.univ.erase index, expr.coeff x * state x) +
          expr.coeff index * state index) + expr.constant := by ring
    _ = (∑ x, expr.coeff x * state x) + expr.constant :=
      congrArg (fun total => total + expr.constant) hsum

/-- An affine function is independent of a component when its coefficient is
zero. -/
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
    {privateSet : Finset (Fin n)} {lhs rhs : State n}
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

end OMMXProof
