import OMMXProof.Linear.Farkas
import OMMXProof.Reduction
import Mathlib.Tactic.Linarith

/-!
# Indicator promotion obligations

Active-branch equality is checked by exact substitution. Replacement additionally
requires a Farkas implication on the inactive branch, indexed only by the
surviving system; the consumed row is unavailable by construction.
-/

namespace OMMXProof

namespace LinearConstraint

def substitute (constraint : LinearConstraint n) (index : Fin n) (value : Rat) :
    LinearConstraint n where
  expr := constraint.expr.substitute index value
  sense := constraint.sense

def Same (lhs rhs : LinearConstraint n) : Prop :=
  lhs.sense = rhs.sense ∧ Affine.Same lhs.expr rhs.expr

instance (lhs rhs : LinearConstraint n) : Decidable (Same lhs rhs) := by
  unfold Same
  infer_instance

def same (lhs rhs : LinearConstraint n) : Bool := decide (Same lhs rhs)

theorem same_sound {lhs rhs : LinearConstraint n} (hcheck : same lhs rhs = true) :
    lhs = rhs := by
  have hsame : Same lhs rhs := by
    simpa [same, decide_eq_true_eq] using hcheck
  cases lhs with
  | mk lhsExpr lhsSense =>
    cases rhs with
    | mk rhsExpr rhsSense =>
      simp only [Same] at hsame
      rcases hsame with ⟨hsense, hexpr⟩
      subst rhsSense
      have : lhsExpr = rhsExpr := Affine.same_iff.mp hexpr
      subst rhsExpr
      rfl

theorem substitute_holds_iff {constraint : LinearConstraint n} {index : Fin n}
    {value : Rat} {assignment : Assignment n}
    (hvalue : assignment index = value) :
    (constraint.substitute index value).Holds assignment ↔
      constraint.Holds assignment := by
  cases constraint with
  | mk expr sense =>
    cases sense <;> simp [substitute, Holds, Affine.eval_substitute hvalue]

end LinearConstraint

/-- Add a branch equation `x_trigger = value` without changing the surviving
inequalities. -/
def branchSystem (surviving : LinearSystem n) (trigger : Fin n) (value : Rat) :
    LinearSystem n where
  ineqCount := surviving.ineqCount
  eqCount := Nat.succ surviving.eqCount
  inequalities := surviving.inequalities
  equalities := Fin.cases
    (Affine.add (Affine.coordinate trigger)
      { coeff := fun _ => 0, constant := -value })
    surviving.equalities

theorem branchSystem_feasible {surviving : LinearSystem n} {trigger : Fin n}
    {value : Rat} {assignment : Assignment n}
    (hsurviving : surviving.Feasible assignment)
    (hvalue : assignment trigger = value) :
    (branchSystem surviving trigger value).Feasible assignment := by
  constructor
  · exact hsurviving.1
  · intro index
    refine Fin.cases ?_ (fun i => hsurviving.2 i) index
    have hconstant :
        ({ coeff := fun _ => 0, constant := -value } : Affine n).eval assignment =
          -value := by simp [Affine.eval]
    change (Affine.add (Affine.coordinate trigger)
      { coeff := fun _ => 0, constant := -value }).eval assignment = 0
    rw [Affine.eval_add, Affine.eval_coordinate, hconstant, hvalue]
    ring

def IndicatorPredicate (trigger : Fin n) (polarity : IndicatorPolarity)
    (body : Assignment n → Prop) (assignment : Assignment n) : Prop :=
  polarity.Active (assignment trigger) → body assignment

/-- Active-branch exactness is sufficient for augmentation while retaining the
source row. -/
theorem indicator_augment
    (base source consequent : Assignment n → Prop)
    (trigger : Fin n) (polarity : IndicatorPolarity)
    (activeForward : ∀ {assignment},
      base assignment →
      VariableDomain.KindHolds .binary (assignment trigger) →
      source assignment →
      polarity.Active (assignment trigger) →
      consequent assignment) :
    ∀ assignment,
      base assignment →
      VariableDomain.KindHolds .binary (assignment trigger) →
      (source assignment ↔
        source assignment ∧
          IndicatorPredicate trigger polarity consequent assignment) := by
  intro assignment hbase hbinary
  constructor
  · intro hsource
    refine ⟨hsource, ?_⟩
    intro hactive
    exact activeForward hbase hbinary hsource hactive
  · exact And.left

/-- Replacement is exact when the active branch agrees in both directions and
the source row follows from the surviving base on the inactive branch. -/
theorem indicator_replace
    (base source consequent : Assignment n → Prop)
    (trigger : Fin n) (polarity : IndicatorPolarity)
    (activeExact : ∀ {assignment},
      base assignment →
      VariableDomain.KindHolds .binary (assignment trigger) →
      polarity.Active (assignment trigger) →
      (source assignment ↔ consequent assignment))
    (inactiveSource : ∀ {assignment},
      base assignment →
      VariableDomain.KindHolds .binary (assignment trigger) →
      assignment trigger = polarity.inactiveValue →
      source assignment) :
    ∀ assignment,
      base assignment →
      VariableDomain.KindHolds .binary (assignment trigger) →
      (source assignment ↔
        IndicatorPredicate trigger polarity consequent assignment) := by
  intro assignment hbase hbinary
  rcases IndicatorPolarity.active_or_inactive_of_binary hbinary with hactive | hinactive
  · constructor
    · intro hsource _
      exact (activeExact hbase hbinary hactive).mp hsource
    · intro hindicator
      exact (activeExact hbase hbinary hactive).mpr (hindicator hactive)
  · have hnotActive : ¬polarity.Active (assignment trigger) := by
      cases polarity <;> simp_all [IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, IndicatorPolarity.inactiveValue]
    constructor
    · intro _ hactive
      exact False.elim (hnotActive hactive)
    · intro _
      exact inactiveSource hbase hbinary hinactive

/-- Equality Indicators use two independent sides. Both inactive obligations
receive the same surviving base, so neither consumed side can prove the other. -/
theorem equalityIndicator_replace
    (base sourceLower sourceUpper consequentLower consequentUpper :
      Assignment n → Prop)
    (trigger : Fin n) (polarity : IndicatorPolarity)
    (activeLower : ∀ {assignment},
      base assignment →
      VariableDomain.KindHolds .binary (assignment trigger) →
      polarity.Active (assignment trigger) →
      (sourceLower assignment ↔ consequentLower assignment))
    (activeUpper : ∀ {assignment},
      base assignment →
      VariableDomain.KindHolds .binary (assignment trigger) →
      polarity.Active (assignment trigger) →
      (sourceUpper assignment ↔ consequentUpper assignment))
    (inactiveLower : ∀ {assignment},
      base assignment →
      VariableDomain.KindHolds .binary (assignment trigger) →
      assignment trigger = polarity.inactiveValue →
      sourceLower assignment)
    (inactiveUpper : ∀ {assignment},
      base assignment →
      VariableDomain.KindHolds .binary (assignment trigger) →
      assignment trigger = polarity.inactiveValue →
      sourceUpper assignment) :
    ∀ assignment,
      base assignment →
      VariableDomain.KindHolds .binary (assignment trigger) →
      (sourceLower assignment ∧ sourceUpper assignment ↔
        IndicatorPredicate trigger polarity
          (fun x => consequentLower x ∧ consequentUpper x) assignment) := by
  apply indicator_replace base
    (fun x => sourceLower x ∧ sourceUpper x)
    (fun x => consequentLower x ∧ consequentUpper x)
    trigger polarity
  · intro assignment hbase hbinary hactive
    exact and_congr
      (activeLower hbase hbinary hactive)
      (activeUpper hbase hbinary hactive)
  · intro assignment hbase hbinary hinactive
    exact ⟨inactiveLower hbase hbinary hinactive,
      inactiveUpper hbase hbinary hinactive⟩

/-- Inactive-branch witness for one consumed inequality. Its system contains
only surviving rows plus the branch equation. -/
structure IndicatorReplaceWitness (surviving : LinearSystem n)
    (trigger : Fin n) (polarity : IndicatorPolarity) where
  inactive : FarkasWitness
    (branchSystem surviving trigger polarity.inactiveValue)

def checkIndicatorActive (domains : Fin n → VariableDomain)
    (source body : LinearConstraint n) (trigger : Fin n)
    (polarity : IndicatorPolarity) : Bool :=
  decide ((domains trigger).kind = .binary) &&
    (source.substitute trigger polarity.activeValue).same body

theorem checkIndicatorActive_sound
    {domains : Fin n → VariableDomain}
    {source body : LinearConstraint n} {trigger : Fin n}
    {polarity : IndicatorPolarity}
    (hcheck : checkIndicatorActive domains source body trigger polarity = true)
    {assignment : Assignment n}
    (hactive : polarity.Active (assignment trigger)) :
    source.Holds assignment ↔ body.Holds assignment := by
  have hparts := Bool.and_eq_true_iff.mp hcheck
  have hsame := LinearConstraint.same_sound hparts.2
  have hvalue : assignment trigger = polarity.activeValue := hactive
  rw [← LinearConstraint.substitute_holds_iff hvalue, hsame]

/-- An accepted active-branch check justifies adding the Indicator while the
source row remains present. This is identity-space augmentation, not removal. -/
theorem checkIndicatorAugment_preserves
    {domains : Fin n → VariableDomain}
    {source body : LinearConstraint n} {trigger : Fin n}
    {polarity : IndicatorPolarity}
    (hcheck : checkIndicatorActive domains source body trigger polarity = true)
    (base : Assignment n → Prop) (objective : Assignment n → Rat)
    (sense : OptimizationSense)
    (baseDomains : ∀ {assignment}, base assignment →
      ∀ i, (domains i).Holds (assignment i)) :
    IdentityPreserves
      (replaceProblem base (fun assignment => source.Holds assignment)
        objective sense)
      (replaceProblem base
        (fun assignment => source.Holds assignment ∧
          (SpecialConstraint.indicator trigger polarity body).Holds assignment)
        objective sense) := by
  apply replace_preserves
  intro assignment hbase
  have hparts := Bool.and_eq_true_iff.mp hcheck
  have hkind : (domains trigger).kind = .binary := by
    simpa [decide_eq_true_eq] using hparts.1
  have hbinary : VariableDomain.KindHolds .binary (assignment trigger) := by
    have hdomain := (baseDomains hbase trigger).1
    rw [hkind] at hdomain
    exact hdomain
  change source.Holds assignment ↔
    source.Holds assignment ∧
      IndicatorPredicate trigger polarity (fun x => body.Holds x) assignment
  apply indicator_augment
      (fun _ => True) (fun x => source.Holds x) (fun x => body.Holds x)
      trigger polarity ?_ assignment trivial hbinary
  intro x _ _ hsource hactive
  exact (checkIndicatorActive_sound hcheck hactive).mp hsource

def checkIndicatorReplace (domains : Fin n → VariableDomain)
    (surviving : LinearSystem n) (source body : LinearConstraint n)
    (trigger : Fin n) (polarity : IndicatorPolarity)
    (witness : IndicatorReplaceWitness surviving trigger polarity) : Bool :=
  decide (source.sense = .lessEqual ∧ body.sense = .lessEqual) &&
    (checkIndicatorActive domains source body trigger polarity &&
      witness.inactive.checkImplication source.expr)

theorem checkIndicatorReplace_sound
    {domains : Fin n → VariableDomain} {surviving : LinearSystem n}
    {source body : LinearConstraint n}
    {trigger : Fin n} {polarity : IndicatorPolarity}
    {witness : IndicatorReplaceWitness surviving trigger polarity}
    (hcheck : checkIndicatorReplace domains surviving source body trigger polarity
      witness = true)
    {assignment : Assignment n}
    (hdomains : ∀ i, (domains i).Holds (assignment i))
    (hsurviving : surviving.Feasible assignment) :
    (source.Holds assignment ↔
      (SpecialConstraint.indicator trigger polarity body).Holds assignment) := by
  have houter := Bool.and_eq_true_iff.mp hcheck
  have hparts := Bool.and_eq_true_iff.mp houter.2
  have hsenses : source.sense = .lessEqual ∧ body.sense = .lessEqual := by
    simpa [decide_eq_true_eq] using houter.1
  have hbinary : VariableDomain.KindHolds .binary (assignment trigger) := by
    have hkind : (domains trigger).kind = .binary := by
      have hactiveParts := Bool.and_eq_true_iff.mp hparts.1
      simpa [decide_eq_true_eq] using hactiveParts.1
    have := (hdomains trigger).1
    rw [hkind] at this
    exact this
  refine indicator_replace
      (fun x => surviving.Feasible x)
      (fun x => source.Holds x)
      (fun x => body.Holds x)
      trigger polarity ?_ ?_ assignment hsurviving hbinary
  · intro x _ hbinaryX hactive
    apply checkIndicatorActive_sound hparts.1
    exact hactive
  · intro x hbase _ hinactive
    have himplied : source.expr.eval x ≤ 0 :=
      FarkasWitness.checkImplication_sound hparts.2
        (branchSystem_feasible hbase hinactive)
    simpa [LinearConstraint.Holds, hsenses.1] using himplied

/-- Equality replacement needs two independent inactive-branch Farkas
certificates over the same surviving system. Neither certificate can mention
the consumed equality because it is absent from the indexed proof system. -/
structure EqualityIndicatorReplaceWitness (surviving : LinearSystem n)
    (trigger : Fin n) (polarity : IndicatorPolarity) where
  upper : FarkasWitness
    (branchSystem surviving trigger polarity.inactiveValue)
  lower : FarkasWitness
    (branchSystem surviving trigger polarity.inactiveValue)

def checkEqualityIndicatorReplace (domains : Fin n → VariableDomain)
    (surviving : LinearSystem n) (source body : LinearConstraint n)
    (trigger : Fin n) (polarity : IndicatorPolarity)
    (witness : EqualityIndicatorReplaceWitness surviving trigger polarity) : Bool :=
  decide (source.sense = .equal ∧ body.sense = .equal) &&
    (checkIndicatorActive domains source body trigger polarity &&
      (witness.upper.checkImplication source.expr &&
        witness.lower.checkImplication (Affine.neg source.expr)))

theorem checkEqualityIndicatorReplace_sound
    {domains : Fin n → VariableDomain} {surviving : LinearSystem n}
    {source body : LinearConstraint n}
    {trigger : Fin n} {polarity : IndicatorPolarity}
    {witness : EqualityIndicatorReplaceWitness surviving trigger polarity}
    (hcheck : checkEqualityIndicatorReplace domains surviving source body trigger polarity
      witness = true)
    {assignment : Assignment n}
    (hdomains : ∀ i, (domains i).Holds (assignment i))
    (hsurviving : surviving.Feasible assignment) :
    (source.Holds assignment ↔
      (SpecialConstraint.indicator trigger polarity body).Holds assignment) := by
  have houter := Bool.and_eq_true_iff.mp hcheck
  have hmiddle := Bool.and_eq_true_iff.mp houter.2
  have hinner := Bool.and_eq_true_iff.mp hmiddle.2
  have hsenses : source.sense = .equal ∧ body.sense = .equal := by
    simpa [decide_eq_true_eq] using houter.1
  have hbinary : VariableDomain.KindHolds .binary (assignment trigger) := by
    have hactiveParts := Bool.and_eq_true_iff.mp hmiddle.1
    have hkind : (domains trigger).kind = .binary := by
      simpa [decide_eq_true_eq] using hactiveParts.1
    have hdomain := (hdomains trigger).1
    rw [hkind] at hdomain
    exact hdomain
  refine indicator_replace
      (fun x => surviving.Feasible x)
      (fun x => source.Holds x)
      (fun x => body.Holds x)
      trigger polarity ?_ ?_ assignment hsurviving hbinary
  · intro x _ _ hactive
    exact checkIndicatorActive_sound hmiddle.1 hactive
  · intro x hbase _ hinactive
    have hbranch := branchSystem_feasible hbase hinactive
    have hu : source.expr.eval x ≤ 0 :=
      FarkasWitness.checkImplication_sound hinner.1 hbranch
    have hl : (Affine.neg source.expr).eval x ≤ 0 :=
      FarkasWitness.checkImplication_sound hinner.2 hbranch
    have heq : source.expr.eval x = 0 := by
      rw [Affine.eval_neg] at hl
      linarith
    simpa [LinearConstraint.Holds, hsenses.1] using heq

/-! ## Big-M lowering semantics

The executable replacement checkers above recover an Indicator from candidate
rows.  The following exact semantic layer specifies the forward algorithm used
by the SDK: emit the upper side only for a positive upper bound, emit the lower
side only for a negative lower bound, and otherwise rely on the corresponding
bound implication.  The denotation is generic in `body`, so the theorem is not
limited to the affine syntax of `CoreModel`.
-/

namespace IndicatorBigM

/-- The upper Big-M side `f(x) + u y - u ≤ 0`, omitted when `u ≤ 0`. -/
def UpperSide (body : Assignment n → Rat) (trigger : Fin n) (upper : Rat)
    (assignment : Assignment n) : Prop :=
  if 0 < upper then
    body assignment + upper * assignment trigger - upper ≤ 0
  else
    True

/-- The lower Big-M side `-f(x) - l y + l ≤ 0`, omitted when `l ≥ 0`. -/
def LowerSide (body : Assignment n → Rat) (trigger : Fin n) (lower : Rat)
    (assignment : Assignment n) : Prop :=
  if lower < 0 then
    -body assignment - lower * assignment trigger + lower ≤ 0
  else
    True

theorem upperSide_iff_indicator {body : Assignment n → Rat} {trigger : Fin n}
    {upper : Rat} {assignment : Assignment n}
    (hbinary : VariableDomain.KindHolds .binary (assignment trigger))
    (hbound : body assignment ≤ upper) :
    UpperSide body trigger upper assignment ↔
      IndicatorPredicate trigger .activeOnOne
        (fun x => body x ≤ 0) assignment := by
  rcases hbinary with hzero | hone
  · by_cases hupper : 0 < upper
    · simp [UpperSide, hupper, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hzero]
      linarith
    · simp [UpperSide, hupper, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hzero]
  · by_cases hupper : 0 < upper
    · simp [UpperSide, hupper, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hone]
    · have hnonpos : upper ≤ 0 := le_of_not_gt hupper
      have hbody : body assignment ≤ 0 := le_trans hbound hnonpos
      simp [UpperSide, hupper, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hone, hbody]

theorem lowerSide_iff_indicator {body : Assignment n → Rat} {trigger : Fin n}
    {lower : Rat} {assignment : Assignment n}
    (hbinary : VariableDomain.KindHolds .binary (assignment trigger))
    (hbound : lower ≤ body assignment) :
    LowerSide body trigger lower assignment ↔
      IndicatorPredicate trigger .activeOnOne
        (fun x => 0 ≤ body x) assignment := by
  rcases hbinary with hzero | hone
  · by_cases hlower : lower < 0
    · simp [LowerSide, hlower, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hzero]
      linarith
    · simp [LowerSide, hlower, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hzero]
  · by_cases hlower : lower < 0
    · simp [LowerSide, hlower, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hone]
    · have hnonneg : 0 ≤ lower := le_of_not_gt hlower
      have hbody : 0 ≤ body assignment := le_trans hnonneg hbound
      simp [LowerSide, hlower, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hone, hbody]

theorem equalitySides_iff_indicator {body : Assignment n → Rat}
    {trigger : Fin n} {lower upper : Rat} {assignment : Assignment n}
    (hbinary : VariableDomain.KindHolds .binary (assignment trigger))
    (hlower : lower ≤ body assignment) (hupper : body assignment ≤ upper) :
    UpperSide body trigger upper assignment ∧
        LowerSide body trigger lower assignment ↔
      IndicatorPredicate trigger .activeOnOne
        (fun x => body x = 0) assignment := by
  rw [upperSide_iff_indicator hbinary hupper,
    lowerSide_iff_indicator hbinary hlower]
  unfold IndicatorPredicate
  constructor
  · rintro ⟨hupperSide, hlowerSide⟩ hactive
    exact le_antisymm (hupperSide hactive) (hlowerSide hactive)
  · intro hequal
    constructor
    · intro hactive
      exact le_of_eq (hequal hactive)
    · intro hactive
      exact le_of_eq (hequal hactive).symm

/-- The SDK's one-sided Indicator Big-M algorithm preserves the exact feasible
set when the claimed upper bound holds on the surviving base. -/
theorem inequality_preserves
    (base : Assignment n → Prop) (body : Assignment n → Rat)
    (trigger : Fin n) (upper : Rat) (objective : Assignment n → Rat)
    (sense : OptimizationSense)
    (binaryOnBase : ∀ {assignment}, base assignment →
      VariableDomain.KindHolds .binary (assignment trigger))
    (upperBoundOnBase : ∀ {assignment}, base assignment →
      body assignment ≤ upper) :
    IdentityPreserves
      (replaceProblem base (UpperSide body trigger upper) objective sense)
      (replaceProblem base
        (IndicatorPredicate trigger .activeOnOne (fun x => body x ≤ 0))
        objective sense) := by
  apply replace_preserves
  intro assignment hbase
  exact upperSide_iff_indicator (binaryOnBase hbase) (upperBoundOnBase hbase)

/-- The SDK's two-sided equality Indicator Big-M algorithm preserves the exact
feasible set, including the cases where either bound makes one side redundant. -/
theorem equality_preserves
    (base : Assignment n → Prop) (body : Assignment n → Rat)
    (trigger : Fin n) (lower upper : Rat) (objective : Assignment n → Rat)
    (sense : OptimizationSense)
    (binaryOnBase : ∀ {assignment}, base assignment →
      VariableDomain.KindHolds .binary (assignment trigger))
    (boundsOnBase : ∀ {assignment}, base assignment →
      lower ≤ body assignment ∧ body assignment ≤ upper) :
    IdentityPreserves
      (replaceProblem base
        (fun assignment =>
          UpperSide body trigger upper assignment ∧
            LowerSide body trigger lower assignment)
        objective sense)
      (replaceProblem base
        (IndicatorPredicate trigger .activeOnOne (fun x => body x = 0))
        objective sense) := by
  apply replace_preserves
  intro assignment hbase
  have hbounds := boundsOnBase hbase
  exact equalitySides_iff_indicator (binaryOnBase hbase) hbounds.1 hbounds.2

end IndicatorBigM

end OMMXProof
