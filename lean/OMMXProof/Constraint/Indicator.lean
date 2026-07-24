import OMMXProof.Constraint.Linear
import Mathlib.Tactic.Linarith

/-!
# Indicator promotion obligations

Active-branch equality is checked by exact substitution. Generic semantic
theorems state the obligations for augmentation and replacement, while the
Big-M results describe the forward lowering used by the SDK.
-/

namespace OMMXProof

inductive IndicatorPolarity where
  | activeOnZero
  | activeOnOne
  deriving DecidableEq, Repr

namespace IndicatorPolarity

def activeValue : IndicatorPolarity → Rat
  | .activeOnZero => 0
  | .activeOnOne => 1

def inactiveValue : IndicatorPolarity → Rat
  | .activeOnZero => 1
  | .activeOnOne => 0

def Active (polarity : IndicatorPolarity) (value : Rat) : Prop :=
  value = polarity.activeValue

instance (polarity : IndicatorPolarity) (value : Rat) :
    Decidable (Active polarity value) := by
  unfold Active
  infer_instance

theorem active_or_inactive_of_binary {polarity : IndicatorPolarity} {value : Rat}
    (hbinary : value ∈ Domain.binary) :
    Active polarity value ∨ value = polarity.inactiveValue := by
  rcases hbinary with rfl | rfl <;> cases polarity <;>
    simp [Active, activeValue, inactiveValue]

end IndicatorPolarity

structure IndicatorConstraint (n : Nat) where
  trigger : Fin n
  polarity : IndicatorPolarity
  body : LinearConstraint n

namespace IndicatorConstraint

def Holds (constraint : IndicatorConstraint n) (state : State n) : Prop :=
  constraint.polarity.Active (state constraint.trigger) →
    constraint.body.Holds state

instance (constraint : IndicatorConstraint n) (state : State n) :
    Decidable (constraint.Holds state) := by
  unfold Holds
  infer_instance

def IndependentAt (constraint : IndicatorConstraint n) (index : Fin n) : Prop :=
  index ≠ constraint.trigger ∧ constraint.body.IndependentAt index

instance (constraint : IndicatorConstraint n) (index : Fin n) :
    Decidable (constraint.IndependentAt index) := by
  unfold IndependentAt
  infer_instance

def IndependentOf (constraint : IndicatorConstraint n)
    (privateSet : Finset (Fin n)) : Prop :=
  ∀ i ∈ privateSet, constraint.IndependentAt i

theorem holds_iff_of_independentOf {constraint : IndicatorConstraint n}
    {privateSet : Finset (Fin n)} {lhs rhs : State n}
    (hindependent : constraint.IndependentOf privateSet)
    (hagree : AgreeOutside privateSet lhs rhs) :
    constraint.Holds lhs ↔ constraint.Holds rhs := by
  have htriggerOutside : constraint.trigger ∉ privateSet := by
    intro hprivate
    exact (hindependent constraint.trigger hprivate).1 rfl
  have htrigger := hagree constraint.trigger htriggerOutside
  have hbody := LinearConstraint.holds_iff_of_independentOf
    (fun i hi => (hindependent i hi).2) hagree
  constructor
  · intro hleft hactive
    apply hbody.mp
    apply hleft
    simpa [htrigger] using hactive
  · intro hright hactive
    apply hbody.mpr
    apply hright
    simpa [htrigger] using hactive

end IndicatorConstraint

def IndicatorPredicate (trigger : Fin n) (polarity : IndicatorPolarity)
    (body : State n → Prop) (state : State n) : Prop :=
  polarity.Active (state trigger) → body state

/-- Active-branch exactness is sufficient for augmentation while retaining the
source row. -/
theorem indicator_augment
    (base source consequent : State n → Prop)
    (trigger : Fin n) (polarity : IndicatorPolarity)
    (activeForward : ∀ {state},
      base state →
      state trigger ∈ Domain.binary →
      source state →
      polarity.Active (state trigger) →
      consequent state) :
    ∀ state,
      base state →
      state trigger ∈ Domain.binary →
      (source state ↔
        source state ∧
          IndicatorPredicate trigger polarity consequent state) := by
  intro state hbase hbinary
  constructor
  · intro hsource
    refine ⟨hsource, ?_⟩
    intro hactive
    exact activeForward hbase hbinary hsource hactive
  · exact And.left

/-- Replacement is exact when the active branch agrees in both directions and
the source row follows from the surviving base on the inactive branch. -/
theorem indicator_replace
    (base source consequent : State n → Prop)
    (trigger : Fin n) (polarity : IndicatorPolarity)
    (activeExact : ∀ {state},
      base state →
      state trigger ∈ Domain.binary →
      polarity.Active (state trigger) →
      (source state ↔ consequent state))
    (inactiveSource : ∀ {state},
      base state →
      state trigger ∈ Domain.binary →
      state trigger = polarity.inactiveValue →
      source state) :
    ∀ state,
      base state →
      state trigger ∈ Domain.binary →
      (source state ↔
        IndicatorPredicate trigger polarity consequent state) := by
  intro state hbase hbinary
  rcases IndicatorPolarity.active_or_inactive_of_binary hbinary with hactive | hinactive
  · constructor
    · intro hsource _
      exact (activeExact hbase hbinary hactive).mp hsource
    · intro hindicator
      exact (activeExact hbase hbinary hactive).mpr (hindicator hactive)
  · have hnotActive : ¬polarity.Active (state trigger) := by
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
      State n → Prop)
    (trigger : Fin n) (polarity : IndicatorPolarity)
    (activeLower : ∀ {state},
      base state →
      state trigger ∈ Domain.binary →
      polarity.Active (state trigger) →
      (sourceLower state ↔ consequentLower state))
    (activeUpper : ∀ {state},
      base state →
      state trigger ∈ Domain.binary →
      polarity.Active (state trigger) →
      (sourceUpper state ↔ consequentUpper state))
    (inactiveLower : ∀ {state},
      base state →
      state trigger ∈ Domain.binary →
      state trigger = polarity.inactiveValue →
      sourceLower state)
    (inactiveUpper : ∀ {state},
      base state →
      state trigger ∈ Domain.binary →
      state trigger = polarity.inactiveValue →
      sourceUpper state) :
    ∀ state,
      base state →
      state trigger ∈ Domain.binary →
      (sourceLower state ∧ sourceUpper state ↔
        IndicatorPredicate trigger polarity
          (fun x => consequentLower x ∧ consequentUpper x) state) := by
  apply indicator_replace base
    (fun x => sourceLower x ∧ sourceUpper x)
    (fun x => consequentLower x ∧ consequentUpper x)
    trigger polarity
  · intro state hbase hbinary hactive
    exact and_congr
      (activeLower hbase hbinary hactive)
      (activeUpper hbase hbinary hactive)
  · intro state hbase hbinary hinactive
    exact ⟨inactiveLower hbase hbinary hinactive,
      inactiveUpper hbase hbinary hinactive⟩

def checkIndicatorActive (domains : Fin n → Domain)
    (source body : LinearConstraint n) (trigger : Fin n)
    (polarity : IndicatorPolarity) : Bool :=
  decide (domains trigger = .binary) &&
    (source.substitute trigger polarity.activeValue).same body

theorem checkIndicatorActive_sound
    {domains : Fin n → Domain}
    {source body : LinearConstraint n} {trigger : Fin n}
    {polarity : IndicatorPolarity}
    (hcheck : checkIndicatorActive domains source body trigger polarity = true)
    {state : State n}
    (hactive : polarity.Active (state trigger)) :
    source.Holds state ↔ body.Holds state := by
  have hparts := Bool.and_eq_true_iff.mp hcheck
  have hsame := LinearConstraint.same_sound hparts.2
  have hvalue : state trigger = polarity.activeValue := hactive
  rw [← LinearConstraint.substitute_holds_iff hvalue, hsame]

/-- An accepted active-branch check justifies adding the Indicator while the
source row remains present. This is same-state augmentation, not removal. -/
theorem checkIndicatorAugment_sound
    {domains : Fin n → Domain}
    {source body : LinearConstraint n} {trigger : Fin n}
    {polarity : IndicatorPolarity}
    (hcheck : checkIndicatorActive domains source body trigger polarity = true)
    (state : State n) :
    source.Holds state ↔
      source.Holds state ∧
        ({ trigger, polarity, body } : IndicatorConstraint n).Holds state := by
  constructor
  · intro hsource
    refine ⟨hsource, ?_⟩
    intro hactive
    exact (checkIndicatorActive_sound hcheck hactive).mp hsource
  · exact And.left

/-! ## Big-M lowering semantics

The following exact semantic layer specifies the forward algorithm used by the
SDK: emit the upper side only for a positive upper bound, emit the lower side
only for a negative lower bound, and otherwise rely on the corresponding bound
implication. The denotation is generic in `body`, so the theorem is not limited
to the affine syntax of `Instance`.
-/

namespace IndicatorBigM

/-- The upper Big-M side `f(x) + u y - u ≤ 0`, omitted when `u ≤ 0`. -/
def UpperSide (body : State n → Rat) (trigger : Fin n) (upper : Rat)
    (state : State n) : Prop :=
  if 0 < upper then
    body state + upper * state trigger - upper ≤ 0
  else
    True

/-- The lower Big-M side `-f(x) - l y + l ≤ 0`, omitted when `l ≥ 0`. -/
def LowerSide (body : State n → Rat) (trigger : Fin n) (lower : Rat)
    (state : State n) : Prop :=
  if lower < 0 then
    -body state - lower * state trigger + lower ≤ 0
  else
    True

theorem upperSide_iff_indicator {body : State n → Rat} {trigger : Fin n}
    {upper : Rat} {state : State n}
    (hbinary : state trigger ∈ Domain.binary)
    (hbound : body state ≤ upper) :
    UpperSide body trigger upper state ↔
      IndicatorPredicate trigger .activeOnOne
        (fun x => body x ≤ 0) state := by
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
      have hbody : body state ≤ 0 := le_trans hbound hnonpos
      simp [UpperSide, hupper, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hone, hbody]

theorem lowerSide_iff_indicator {body : State n → Rat} {trigger : Fin n}
    {lower : Rat} {state : State n}
    (hbinary : state trigger ∈ Domain.binary)
    (hbound : lower ≤ body state) :
    LowerSide body trigger lower state ↔
      IndicatorPredicate trigger .activeOnOne
        (fun x => 0 ≤ body x) state := by
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
      have hbody : 0 ≤ body state := le_trans hnonneg hbound
      simp [LowerSide, hlower, IndicatorPredicate, IndicatorPolarity.Active,
        IndicatorPolarity.activeValue, hone, hbody]

theorem equalitySides_iff_indicator {body : State n → Rat}
    {trigger : Fin n} {lower upper : Rat} {state : State n}
    (hbinary : state trigger ∈ Domain.binary)
    (hlower : lower ≤ body state) (hupper : body state ≤ upper) :
    UpperSide body trigger upper state ∧
        LowerSide body trigger lower state ↔
      IndicatorPredicate trigger .activeOnOne
        (fun x => body x = 0) state := by
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

end IndicatorBigM

end OMMXProof
