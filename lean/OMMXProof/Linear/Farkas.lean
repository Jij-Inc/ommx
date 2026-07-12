import OMMXProof.Core
import Mathlib.Algebra.BigOperators.Fin
import Mathlib.Algebra.Order.BigOperators.Group.Finset
import Mathlib.Tactic.Linarith

/-!
# Exact Farkas checker

The checker consumes rational multipliers over a normalized continuous linear
system. Integer and binary facts are deliberately unavailable except for bound
sides already present as linear rows.
-/

namespace OMMXProof

namespace Affine

/-- Exact finite linear combination. -/
def combination {n : Nat} :
    {m : Nat} → (Fin m → Rat) → (Fin m → Affine n) → Affine n
  | 0, _, _ => zero
  | _m + 1, weights, rows =>
      add (scale (weights 0) (rows 0))
        (combination (fun i => weights i.succ) (fun i => rows i.succ))

@[simp]
theorem combination_zero (weights : Fin 0 → Rat) (rows : Fin 0 → Affine n) :
    combination weights rows = zero := rfl

@[simp]
theorem combination_succ (weights : Fin (m + 1) → Rat)
    (rows : Fin (m + 1) → Affine n) :
    combination weights rows =
      add (scale (weights 0) (rows 0))
        (combination (fun i => weights i.succ) (fun i => rows i.succ)) := rfl

theorem eval_combination (weights : Fin m → Rat) (rows : Fin m → Affine n)
    (assignment : Assignment n) :
    eval (combination weights rows) assignment =
      ∑ i, weights i * eval (rows i) assignment := by
  induction m with
  | zero => simp
  | succ m ih =>
    rw [combination_succ, eval_add, eval_scale, ih]
    simp [Fin.sum_univ_succ]

end Affine

/-- Untrusted exact rational multipliers for one named `LinearSystem`. -/
structure FarkasWitness (system : LinearSystem n) where
  inequalityWeights : Fin system.ineqCount → Rat
  equalityWeights : Fin system.eqCount → Rat

namespace FarkasWitness

variable {n : Nat} {system : LinearSystem n}

def combination (witness : FarkasWitness system) : Affine n :=
  Affine.add
    (Affine.combination witness.inequalityWeights system.inequalities)
    (Affine.combination witness.equalityWeights system.equalities)

/-- Exact normalized implication condition. Equality weights are intentionally
unrestricted in sign. -/
def ValidImplication (witness : FarkasWitness system) (target : Affine n) : Prop :=
  (∀ i, 0 ≤ witness.inequalityWeights i) ∧
    Affine.Implies witness.combination target

instance (witness : FarkasWitness system) (target : Affine n) :
    Decidable (ValidImplication witness target) := by
  unfold ValidImplication
  infer_instance

def checkImplication (witness : FarkasWitness system) (target : Affine n) : Bool :=
  decide (ValidImplication witness target)

theorem inequality_combination_nonpos (witness : FarkasWitness system)
    (assignment : Assignment n)
    (hnonneg : ∀ i, 0 ≤ witness.inequalityWeights i)
    (hrows : ∀ i, (system.inequalities i).eval assignment ≤ 0) :
    (Affine.combination witness.inequalityWeights system.inequalities).eval
      assignment ≤ 0 := by
  rw [Affine.eval_combination]
  exact Finset.sum_nonpos fun i _ =>
    mul_nonpos_of_nonneg_of_nonpos (hnonneg i) (hrows i)

theorem equality_combination_zero (witness : FarkasWitness system)
    (assignment : Assignment n)
    (hrows : ∀ i, (system.equalities i).eval assignment = 0) :
    (Affine.combination witness.equalityWeights system.equalities).eval
      assignment = 0 := by
  rw [Affine.eval_combination]
  exact Finset.sum_eq_zero fun i _ => by simp [hrows i]

theorem checkImplication_sound {witness : FarkasWitness system}
    {target : Affine n} (hcheck : witness.checkImplication target = true)
    {assignment : Assignment n} (hfeasible : system.Feasible assignment) :
    target.eval assignment ≤ 0 := by
  have hvalid : ValidImplication witness target := by
    simpa [checkImplication, decide_eq_true_eq] using hcheck
  have hineq := inequality_combination_nonpos witness assignment hvalid.1 hfeasible.1
  have heq := equality_combination_zero witness assignment hfeasible.2
  have hcombined : witness.combination.eval assignment ≤ 0 := by
    rw [combination, Affine.eval_add, heq]
    simpa using hineq
  exact le_trans (Affine.eval_le_of_implies hvalid.2 assignment) hcombined

/-- Exact Farkas contradiction: zero coefficients and a strictly positive
constant. -/
def ValidInfeasibility (witness : FarkasWitness system) : Prop :=
  (∀ i, 0 ≤ witness.inequalityWeights i) ∧
    (∀ j, witness.combination.coeff j = 0) ∧
    0 < witness.combination.constant

instance (witness : FarkasWitness system) : Decidable witness.ValidInfeasibility := by
  unfold ValidInfeasibility
  infer_instance

def checkInfeasibility (witness : FarkasWitness system) : Bool :=
  decide witness.ValidInfeasibility

theorem checkInfeasibility_sound {witness : FarkasWitness system}
    (hcheck : witness.checkInfeasibility = true) :
    ¬ ∃ assignment, system.Feasible assignment := by
  have hvalid : ValidInfeasibility witness := by
    simpa [checkInfeasibility, decide_eq_true_eq] using hcheck
  rintro ⟨assignment, hfeasible⟩
  have hineq := inequality_combination_nonpos witness assignment hvalid.1 hfeasible.1
  have heq := equality_combination_zero witness assignment hfeasible.2
  have hcombined : witness.combination.eval assignment ≤ 0 := by
    rw [combination, Affine.eval_add, heq]
    simpa using hineq
  have heval : witness.combination.eval assignment = witness.combination.constant := by
    simp [Affine.eval, hvalid.2.1]
  rw [heval] at hcombined
  exact (not_lt_of_ge hcombined) hvalid.2.2

end FarkasWitness

/-- A finite variable-bound side elaborated into the shared linear proof atoms. -/
inductive BoundSide (n : Nat) where
  | lower (index : Fin n) (value : Rat)
  | upper (index : Fin n) (value : Rat)
  deriving DecidableEq, Repr

namespace BoundSide

def toAffine : BoundSide n → Affine n
  | .lower index value =>
      Affine.add (Affine.scale (-1) (Affine.coordinate index))
        { coeff := fun _ => 0, constant := value }
  | .upper index value =>
      Affine.add (Affine.coordinate index)
        { coeff := fun _ => 0, constant := -value }

theorem holds_iff :
    (side : BoundSide n) → (assignment : Assignment n) →
      side.toAffine.eval assignment ≤ 0 ↔
        match side with
        | .lower index value => value ≤ assignment index
        | .upper index value => assignment index ≤ value
  | .lower index value, assignment => by
      have hconstant :
          ({ coeff := fun _ => 0, constant := value } : Affine n).eval assignment =
            value := by simp [Affine.eval]
      simp only [toAffine, Affine.eval_add, Affine.eval_scale,
        Affine.eval_coordinate, hconstant]
      constructor <;> intro h <;> linarith
  | .upper index value, assignment => by
      have hconstant :
          ({ coeff := fun _ => 0, constant := -value } : Affine n).eval assignment =
            -value := by simp [Affine.eval]
      simp only [toAffine, Affine.eval_add, Affine.eval_coordinate, hconstant]
      constructor <;> intro h <;> linarith

/-- The bound atom is justified by the corresponding stored domain side. -/
def ValidFor (domains : Fin n → VariableDomain) : BoundSide n → Prop
  | .lower index value => (domains index).bounds.lower = some value
  | .upper index value => (domains index).bounds.upper = some value

instance (domains : Fin n → VariableDomain) (side : BoundSide n) :
    Decidable (side.ValidFor domains) := by
  cases side <;> simp [ValidFor] <;> infer_instance

theorem validFor_holds {domains : Fin n → VariableDomain}
    {side : BoundSide n} {assignment : Assignment n}
    (hvalid : side.ValidFor domains)
    (hdomains : ∀ i, (domains i).Holds (assignment i)) :
    side.toAffine.eval assignment ≤ 0 := by
  cases side with
  | lower index value =>
    apply (holds_iff (.lower index value) assignment).mpr
    have hbounds := (hdomains index).2
    unfold Bounds.Holds at hbounds
    simp [ValidFor] at hvalid
    rw [hvalid] at hbounds
    exact hbounds.1
  | upper index value =>
    apply (holds_iff (.upper index value) assignment).mpr
    have hbounds := (hdomains index).2
    unfold Bounds.Holds at hbounds
    simp [ValidFor] at hvalid
    rw [hvalid] at hbounds
    exact hbounds.2

/-- `target.Tightens source` means that both sides refer to the same variable
and direction and that the target interval is contained in the source
interval. In particular, a merely implied but weaker replacement is rejected. -/
def Tightens : BoundSide n → BoundSide n → Prop
  | .lower targetIndex targetValue, .lower sourceIndex sourceValue =>
      targetIndex = sourceIndex ∧ sourceValue ≤ targetValue
  | .upper targetIndex targetValue, .upper sourceIndex sourceValue =>
      targetIndex = sourceIndex ∧ targetValue ≤ sourceValue
  | _, _ => False

instance (target source : BoundSide n) : Decidable (target.Tightens source) := by
  cases target <;> cases source <;> unfold Tightens <;> infer_instance

theorem target_holds_implies_source {target source : BoundSide n}
    (htightens : target.Tightens source) {assignment : Assignment n}
    (htarget : target.toAffine.eval assignment ≤ 0) :
    source.toAffine.eval assignment ≤ 0 := by
  cases target with
  | lower targetIndex targetValue =>
    cases source with
    | lower sourceIndex sourceValue =>
      rcases htightens with ⟨hindex, hvalues⟩
      subst sourceIndex
      apply (holds_iff (.lower targetIndex sourceValue) assignment).mpr
      exact le_trans hvalues
        ((holds_iff (.lower targetIndex targetValue) assignment).mp htarget)
    | upper _ _ => exact False.elim htightens
  | upper targetIndex targetValue =>
    cases source with
    | lower _ _ => exact False.elim htightens
    | upper sourceIndex sourceValue =>
      rcases htightens with ⟨hindex, hvalues⟩
      subst sourceIndex
      apply (holds_iff (.upper targetIndex sourceValue) assignment).mpr
      exact le_trans
        ((holds_iff (.upper targetIndex targetValue) assignment).mp htarget)
        hvalues

end BoundSide

/-- A proof system containing only finite variable-bound sides. Activity-bound
proofs elaborate to an ordinary `FarkasWitness` over this system. -/
def boundSystem (sides : Fin m → BoundSide n) : LinearSystem n where
  ineqCount := m
  eqCount := 0
  inequalities := fun i => (sides i).toAffine
  equalities := fun i => nomatch i

abbrev ActivityBoundWitness (sides : Fin m → BoundSide n) :=
  FarkasWitness (boundSystem sides)

theorem checkActivityBound_sound {sides : Fin m → BoundSide n}
    {witness : ActivityBoundWitness sides} {target : Affine n}
    (hcheck : witness.checkImplication target = true)
    {assignment : Assignment n}
    (hbounds : ∀ i, (sides i).toAffine.eval assignment ≤ 0) :
    target.eval assignment ≤ 0 := by
  apply FarkasWitness.checkImplication_sound hcheck
  exact ⟨hbounds, fun i => nomatch i⟩

/-- Executable model-domain binding for an activity-bound proof. -/
def checkActivityBoundForDomains (domains : Fin n → VariableDomain)
    (sides : Fin m → BoundSide n) (witness : ActivityBoundWitness sides)
    (target : Affine n) : Bool :=
  decide (∀ i, (sides i).ValidFor domains) && witness.checkImplication target

theorem checkActivityBoundForDomains_sound
    {domains : Fin n → VariableDomain} {sides : Fin m → BoundSide n}
    {witness : ActivityBoundWitness sides} {target : Affine n}
    (hcheck : checkActivityBoundForDomains domains sides witness target = true)
    {assignment : Assignment n}
    (hdomains : ∀ i, (domains i).Holds (assignment i)) :
    target.eval assignment ≤ 0 := by
  have hparts := Bool.and_eq_true_iff.mp hcheck
  have hvalid : ∀ i, (sides i).ValidFor domains := by
    simpa [decide_eq_true_eq] using hparts.1
  apply checkActivityBound_sound hparts.2
  intro i
  exact BoundSide.validFor_holds (hvalid i) hdomains

/-- Two one-sided implications constitute an implied equality. -/
structure ImpliedEqualityWitness (system : LinearSystem n) where
  upper : FarkasWitness system
  lower : FarkasWitness system

namespace ImpliedEqualityWitness

variable {n : Nat} {system : LinearSystem n}

def check (witness : ImpliedEqualityWitness system) (target : Affine n) : Bool :=
  witness.upper.checkImplication target &&
    witness.lower.checkImplication (Affine.neg target)

theorem check_sound {witness : ImpliedEqualityWitness system}
    {target : Affine n} (hcheck : witness.check target = true)
    {assignment : Assignment n} (hfeasible : system.Feasible assignment) :
    target.eval assignment = 0 := by
  have hparts := Bool.and_eq_true_iff.mp hcheck
  have hu := FarkasWitness.checkImplication_sound hparts.1 hfeasible
  have hl := FarkasWitness.checkImplication_sound hparts.2 hfeasible
  simp only [Affine.eval_neg] at hl
  linarith

end ImpliedEqualityWitness

section BoundTightening

/-- The proof context for replacing one stored bound contains the surviving
linear system and the old bound. This is the exact pre-state of the abstract
bound-replacement operation. -/
def boundReplacementSystem (remaining : LinearSystem n) (source : BoundSide n) :
    LinearSystem n where
  ineqCount := Nat.succ remaining.ineqCount
  eqCount := remaining.eqCount
  inequalities := Fin.cases source.toAffine remaining.inequalities
  equalities := remaining.equalities

def BoundReplacementFeasible (remaining : LinearSystem n) (side : BoundSide n)
    (assignment : Assignment n) : Prop :=
  remaining.Feasible assignment ∧ side.toAffine.eval assignment ≤ 0

theorem boundReplacementSystem_feasible_iff
    {remaining : LinearSystem n} {source : BoundSide n}
    {assignment : Assignment n} :
    (boundReplacementSystem remaining source).Feasible assignment ↔
      BoundReplacementFeasible remaining source assignment := by
  change
    ((∀ i : Fin (Nat.succ remaining.ineqCount),
        ((Fin.cases source.toAffine remaining.inequalities i : Affine n).eval
          assignment ≤ 0)) ∧
      (∀ i, (remaining.equalities i).eval assignment = 0)) ↔
      (((∀ i, (remaining.inequalities i).eval assignment ≤ 0) ∧
        (∀ i, (remaining.equalities i).eval assignment = 0)) ∧
        source.toAffine.eval assignment ≤ 0)
  constructor
  · intro h
    refine ⟨⟨?_, h.2⟩, ?_⟩
    · intro i
      exact h.1 i.succ
    · exact h.1 (Fin.mk 0 (Nat.zero_lt_succ _))
  · rintro ⟨hremaining, hsource⟩
    constructor
    · intro i
      refine Fin.cases hsource (fun j => hremaining.1 j) i
    · exact hremaining.2

abbrev BoundTighteningWitness (remaining : LinearSystem n) (source : BoundSide n) :=
  FarkasWitness (boundReplacementSystem remaining source)

/-- Exact checker for replacing a stored bound. Besides the Farkas implication
from the pre-state, it checks that the source is an actual domain side and that
the proposed replacement is genuinely tighter in the same direction. -/
def checkBoundTightening (domains : Fin n → VariableDomain)
    (remaining : LinearSystem n) (source target : BoundSide n)
    (witness : BoundTighteningWitness remaining source) : Bool :=
  decide (source.ValidFor domains) &&
    (decide (target.Tightens source) &&
      witness.checkImplication target.toAffine)

theorem checkBoundTightening_sound
    {domains : Fin n → VariableDomain} {remaining : LinearSystem n}
    {source target : BoundSide n}
    {witness : BoundTighteningWitness remaining source}
    (hcheck : checkBoundTightening domains remaining source target witness = true) :
    source.ValidFor domains ∧
      ∀ assignment,
        BoundReplacementFeasible remaining source assignment ↔
          BoundReplacementFeasible remaining target assignment := by
  have houter := Bool.and_eq_true_iff.mp hcheck
  have hinner := Bool.and_eq_true_iff.mp houter.2
  have hvalid : source.ValidFor domains := by
    simpa [decide_eq_true_eq] using houter.1
  have htightens : target.Tightens source := by
    simpa [decide_eq_true_eq] using hinner.1
  refine ⟨hvalid, ?_⟩
  intro assignment
  constructor
  · intro hsource
    refine ⟨hsource.1, ?_⟩
    apply FarkasWitness.checkImplication_sound hinner.2
    exact boundReplacementSystem_feasible_iff.mpr hsource
  · intro htarget
    exact ⟨htarget.1,
      BoundSide.target_holds_implies_source htightens htarget.2⟩

end BoundTightening

/-- Semantic feasibility after adding one regular inequality row. -/
def RowExtensionFeasible (remaining : LinearSystem n) (row : Affine n)
    (assignment : Assignment n) : Prop :=
  remaining.Feasible assignment ∧ row.eval assignment ≤ 0

/-- A redundancy proof is structurally unable to use the removed row: its
witness is indexed only by `remaining`. -/
theorem redundantRow_iff {remaining : LinearSystem n} {row : Affine n}
    {witness : FarkasWitness remaining}
    (hcheck : witness.checkImplication row = true)
    (assignment : Assignment n) :
    RowExtensionFeasible remaining row assignment ↔
      remaining.Feasible assignment := by
  constructor
  · exact fun h => h.1
  · intro h
    exact ⟨h, FarkasWitness.checkImplication_sound hcheck h⟩

end OMMXProof
