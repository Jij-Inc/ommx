import OMMXProof.Constraint.Indicator
import OMMXProof.Domain
import Mathlib.Tactic

/-!
# Indicator Big-M formulation

This module gives the pointwise semantics used to prove Indicator Big-M
lowering. The formulation currently supports an active-on-one trigger and
omits a side when the corresponding affine bound already implies it.
-/

namespace OMMXProof

namespace Instance

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

/-- The upper Big-M side is exactly an active-on-one `≤` Indicator when the
trigger is binary and `upper` bounds the body value. -/
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

/-- The lower Big-M side is exactly an active-on-one `≥` Indicator when the
trigger is binary and `lower` bounds the body value. -/
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

/-- The upper and lower Big-M sides together are exactly an active-on-one
equality Indicator when both bounds hold. -/
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

end Instance

end OMMXProof
