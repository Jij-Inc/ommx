import OMMXProof.Domain
import OMMXProof.State
import Mathlib.Algebra.BigOperators.Fin
import Mathlib.Algebra.BigOperators.Ring.Finset
import Mathlib.Data.Fintype.BigOperators
import Mathlib.Tactic.Ring

/-!
# Exact affine-function semantics

This module defines affine functions over exact rational states together with
their algebra, evaluation, and sound bounds over decision-variable domains.
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

/-- The minimum rational interval containing every value of an affine function
over the given decision-variable bounds. -/
@[simp]
def evaluateBound (expr : Affine n) (bounds : Fin n → Bound) : Bound :=
  (List.ofFn fun i => (bounds i).scale (expr.coeff i)).foldr
    Bound.add (Bound.point expr.constant)

/-- A rational interval containing every value of an affine function over the
given decision-variable domains.

Integer domains contribute their containing rational intervals, so this
interval is in general not a tight bound for the discrete affine image. -/
abbrev evaluateBoundFromDomains (expr : Affine n)
    (domains : Fin n → Domain) : Bound :=
  evaluateBound expr (fun i => (domains i).bound)

/-- Every state in the supplied domains evaluates inside
`evaluateBoundFromDomains`. -/
theorem evaluateBoundFromDomains_sound (expr : Affine n)
    (domains : Fin n → Domain) {state : State n}
    (hdomains : ∀ i, state i ∈ domains i) :
    expr.eval state ∈ expr.evaluateBoundFromDomains domains := by
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
  simpa [terms, evaluateBoundFromDomains, evaluateBound, eval, Fin.sum_ofFn,
    Function.comp_def] using hfold

/-- The affine expression selecting one coordinate. -/
def coordinate (index : Fin n) : Affine n where
  coeff := fun i => if i = index then 1 else 0
  constant := 0

@[simp]
theorem eval_coordinate (index : Fin n) (state : State n) :
    eval (coordinate index) state = state index := by
  simp [eval, coordinate]

end Affine

end OMMXProof
