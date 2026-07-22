import OMMXProof.State
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

end Affine

end OMMXProof
