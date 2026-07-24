import Mathlib.Algebra.Order.Ring.Rat
import Mathlib.Order.Basic
import Mathlib.Order.WithBotTop
import Mathlib.Tactic.Linarith

/-!
# Decision-variable domains and exact interval bounds

This module gives unbounded interval endpoints an explicit exact syntax.
`Bound` represents only nonempty rational intervals; an integer domain may
still be empty because such an interval need not contain an integer.
-/

namespace OMMXProof

/-! ## Extended-rational endpoints -/

/-- Exact rational endpoints with both infinities. -/
inductive Endpoint where
  | negInf
  | finite (value : Rat)
  | posInf
  deriving DecidableEq, Repr

namespace Endpoint

private abbrev OrderModel := WithBot (WithTop Rat)

private def toOrderModel : Endpoint → OrderModel
  | .negInf => ⊥
  | .finite value => ↑(value : WithTop Rat)
  | .posInf => ↑(⊤ : WithTop Rat)

private theorem toOrderModel_injective :
    Function.Injective toOrderModel := by
  intro lhs rhs
  cases lhs <;> cases rhs <;> simp [toOrderModel]

instance : LinearOrder Endpoint :=
  LinearOrder.lift' toOrderModel toOrderModel_injective

instance : Coe Rat Endpoint where
  coe := .finite

@[simp]
theorem negInf_le (endpoint : Endpoint) : .negInf ≤ endpoint := by
  cases endpoint <;>
    change (⊥ : OrderModel) ≤ _ <;> simp [toOrderModel]

@[simp]
theorem le_posInf (endpoint : Endpoint) : endpoint ≤ .posInf := by
  cases endpoint <;>
    change _ ≤ (↑(⊤ : WithTop Rat) : OrderModel) <;> simp [toOrderModel]

@[simp]
theorem negInf_lt_finite (value : Rat) :
    Endpoint.negInf < Endpoint.finite value := by
  change (⊥ : OrderModel) < (↑(value : WithTop Rat) : OrderModel)
  simp

@[simp]
theorem finite_lt_posInf (value : Rat) :
    Endpoint.finite value < Endpoint.posInf := by
  change (↑(value : WithTop Rat) : OrderModel) <
    (↑(⊤ : WithTop Rat) : OrderModel)
  exact WithBot.coe_lt_coe.mpr (WithTop.coe_lt_top value)

@[simp]
theorem negInf_lt_posInf :
    Endpoint.negInf < Endpoint.posInf := by
  exact lt_trans (negInf_lt_finite 0) (finite_lt_posInf 0)

@[simp]
theorem finite_le_finite {lhs rhs : Rat} :
    Endpoint.finite lhs ≤ Endpoint.finite rhs ↔ lhs ≤ rhs := by
  change (↑(lhs : WithTop Rat) : OrderModel) ≤
      (↑(rhs : WithTop Rat) : OrderModel) ↔ lhs ≤ rhs
  simp

@[simp]
theorem finite_lt_finite {lhs rhs : Rat} :
    Endpoint.finite lhs < Endpoint.finite rhs ↔ lhs < rhs := by
  change (↑(lhs : WithTop Rat) : OrderModel) <
      (↑(rhs : WithTop Rat) : OrderModel) ↔ lhs < rhs
  simp

@[simp]
theorem posInf_ne_finite (value : Rat) :
    Endpoint.posInf ≠ Endpoint.finite value := by
  intro h
  have := finite_lt_posInf value
  rw [h] at this
  exact (lt_irrefl _ this)

@[simp]
theorem finite_ne_negInf (value : Rat) :
    Endpoint.finite value ≠ Endpoint.negInf := by
  intro h
  have := negInf_lt_finite value
  rw [h] at this
  exact (lt_irrefl _ this)

end Endpoint

/-! ## Nonempty rational interval bounds -/

/-- A nonempty closed rational interval, possibly unbounded on either side.

The four constructors intrinsically enforce `lower ≤ upper`,
`lower ≠ +∞`, and `upper ≠ -∞`. -/
inductive Bound where
  | unbounded
  | lowerBounded (lower : Rat)
  | upperBounded (upper : Rat)
  | finite (lower upper : Rat) (valid : lower ≤ upper)
  deriving DecidableEq, Repr

namespace Bound

/-- The singleton interval `{value}`. -/
def point (value : Rat) : Bound :=
  .finite value value le_rfl

/-- The binary interval `[0, 1]`. Integrality is supplied by `Domain.binary`. -/
def binary : Bound :=
  .finite 0 1 (by norm_num)

def lower : Bound → Endpoint
  | .unbounded => .negInf
  | .lowerBounded lower => .finite lower
  | .upperBounded _ => .negInf
  | .finite lower _ _ => .finite lower

def upper : Bound → Endpoint
  | .unbounded => .posInf
  | .lowerBounded _ => .posInf
  | .upperBounded upper => .finite upper
  | .finite _ upper _ => .finite upper

@[simp]
theorem lower_unbounded : lower .unbounded = .negInf := rfl

@[simp]
theorem upper_unbounded : upper .unbounded = .posInf := rfl

@[simp]
theorem lower_lowerBounded (value : Rat) :
    lower (.lowerBounded value) = .finite value := rfl

@[simp]
theorem upper_lowerBounded (value : Rat) :
    upper (.lowerBounded value) = .posInf := rfl

@[simp]
theorem lower_upperBounded (value : Rat) :
    lower (.upperBounded value) = .negInf := rfl

@[simp]
theorem upper_upperBounded (value : Rat) :
    upper (.upperBounded value) = .finite value := rfl

@[simp]
theorem lower_finite (lowerValue upperValue : Rat)
    (hvalid : lowerValue ≤ upperValue) :
    lower (.finite lowerValue upperValue hvalid) = .finite lowerValue := rfl

@[simp]
theorem upper_finite (lowerValue upperValue : Rat)
    (hvalid : lowerValue ≤ upperValue) :
    upper (.finite lowerValue upperValue hvalid) = .finite upperValue := rfl

theorem lower_le_upper (bound : Bound) :
    bound.lower ≤ bound.upper := by
  cases bound with
  | unbounded => exact le_of_lt Endpoint.negInf_lt_posInf
  | lowerBounded lower =>
      exact le_of_lt (Endpoint.finite_lt_posInf lower)
  | upperBounded upper =>
      exact le_of_lt (Endpoint.negInf_lt_finite upper)
  | finite lower upper hvalid =>
      exact Endpoint.finite_le_finite.mpr hvalid

theorem lower_ne_posInf (bound : Bound) :
    bound.lower ≠ .posInf := by
  cases bound with
  | unbounded => exact ne_of_lt Endpoint.negInf_lt_posInf
  | lowerBounded lower =>
      exact ne_of_lt (Endpoint.finite_lt_posInf lower)
  | upperBounded upper => exact ne_of_lt Endpoint.negInf_lt_posInf
  | finite lower upper hvalid =>
      exact ne_of_lt (Endpoint.finite_lt_posInf lower)

theorem upper_ne_negInf (bound : Bound) :
    bound.upper ≠ .negInf := by
  cases bound with
  | unbounded => exact ne_of_gt Endpoint.negInf_lt_posInf
  | lowerBounded lower => exact ne_of_gt Endpoint.negInf_lt_posInf
  | upperBounded upper =>
      exact ne_of_gt (Endpoint.negInf_lt_finite upper)
  | finite lower upper hvalid =>
      exact ne_of_gt (Endpoint.negInf_lt_finite upper)

/-- Whether a rational value belongs to an interval bound. -/
def Holds : Bound → Rat → Prop
  | .unbounded, _ => True
  | .lowerBounded lower, value => lower ≤ value
  | .upperBounded upper, value => value ≤ upper
  | .finite lower upper _, value => lower ≤ value ∧ value ≤ upper

instance (bound : Bound) (value : Rat) : Decidable (bound.Holds value) := by
  cases bound <;> simp only [Holds] <;> infer_instance

instance : Membership Rat Bound where
  mem bound value := bound.Holds value

instance (bound : Bound) (value : Rat) : Decidable (value ∈ bound) := by
  change Decidable (bound.Holds value)
  infer_instance

@[simp]
theorem mem_unbounded (value : Rat) :
    value ∈ Bound.unbounded := by
  trivial

@[simp]
theorem mem_lowerBounded_iff {lower value : Rat} :
    value ∈ Bound.lowerBounded lower ↔ lower ≤ value :=
  Iff.rfl

@[simp]
theorem mem_upperBounded_iff {upper value : Rat} :
    value ∈ Bound.upperBounded upper ↔ value ≤ upper :=
  Iff.rfl

@[simp]
theorem mem_finite_iff {lower upper value : Rat}
    {hvalid : lower ≤ upper} :
    value ∈ Bound.finite lower upper hvalid ↔
      lower ≤ value ∧ value ≤ upper :=
  Iff.rfl

theorem lower_le_finite {bound : Bound} {value : Rat}
    (hvalue : value ∈ bound) :
    bound.lower ≤ .finite value := by
  cases bound with
  | unbounded => exact Endpoint.negInf_le _
  | lowerBounded lower =>
      exact Endpoint.finite_le_finite.mpr hvalue
  | upperBounded upper => exact Endpoint.negInf_le _
  | finite lower upper hvalid =>
      exact Endpoint.finite_le_finite.mpr hvalue.1

theorem finite_le_upper {bound : Bound} {value : Rat}
    (hvalue : value ∈ bound) :
    (.finite value : Endpoint) ≤ bound.upper := by
  cases bound with
  | unbounded => exact Endpoint.le_posInf _
  | lowerBounded lower => exact Endpoint.le_posInf _
  | upperBounded upper =>
      exact Endpoint.finite_le_finite.mpr hvalue
  | finite lower upper hvalid =>
      exact Endpoint.finite_le_finite.mpr hvalue.2

theorem finite_lower_le {bound : Bound} {value lower : Rat}
    (hvalue : value ∈ bound) (hlower : bound.lower = .finite lower) :
    lower ≤ value := by
  have hendpoint := lower_le_finite hvalue
  rw [hlower] at hendpoint
  exact Endpoint.finite_le_finite.mp hendpoint

theorem le_finite_upper {bound : Bound} {value upper : Rat}
    (hvalue : value ∈ bound) (hupper : bound.upper = .finite upper) :
    value ≤ upper := by
  have hendpoint := finite_le_upper hvalue
  rw [hupper] at hendpoint
  exact Endpoint.finite_le_finite.mp hendpoint

/-- Exact Minkowski sum of two rational intervals. -/
def add : Bound → Bound → Bound
  | .unbounded, _ => .unbounded
  | _, .unbounded => .unbounded
  | .lowerBounded lhs, .lowerBounded rhs =>
      .lowerBounded (lhs + rhs)
  | .lowerBounded _, .upperBounded _ => .unbounded
  | .upperBounded _, .lowerBounded _ => .unbounded
  | .upperBounded lhs, .upperBounded rhs =>
      .upperBounded (lhs + rhs)
  | .lowerBounded lhs, .finite lower _ _ =>
      .lowerBounded (lhs + lower)
  | .finite lower _ _, .lowerBounded rhs =>
      .lowerBounded (lower + rhs)
  | .upperBounded lhs, .finite _ upper _ =>
      .upperBounded (lhs + upper)
  | .finite _ upper _, .upperBounded rhs =>
      .upperBounded (upper + rhs)
  | .finite lhsLower lhsUpper lhsValid,
      .finite rhsLower rhsUpper rhsValid =>
      .finite (lhsLower + rhsLower) (lhsUpper + rhsUpper)
        (add_le_add lhsValid rhsValid)

theorem add_holds {lhs rhs : Bound} {lhsValue rhsValue : Rat}
    (hlhs : lhsValue ∈ lhs) (hrhs : rhsValue ∈ rhs) :
    lhsValue + rhsValue ∈ lhs.add rhs := by
  change lhs.Holds lhsValue at hlhs
  change rhs.Holds rhsValue at hrhs
  change (lhs.add rhs).Holds (lhsValue + rhsValue)
  cases lhs <;> cases rhs <;>
    simp only [add, Holds] at hlhs hrhs ⊢
  all_goals first | (constructor <;> linarith) | linarith

/-- Exact image of an interval under multiplication by a rational scalar. -/
def scale (scalar : Rat) (bound : Bound) : Bound :=
  if _hzero : scalar = 0 then
    point 0
  else if hpositive : 0 < scalar then
    match bound with
    | .unbounded => .unbounded
    | .lowerBounded lower => .lowerBounded (scalar * lower)
    | .upperBounded upper => .upperBounded (scalar * upper)
    | .finite lower upper hvalid =>
        .finite (scalar * lower) (scalar * upper)
          (mul_le_mul_of_nonneg_left hvalid (le_of_lt hpositive))
  else
    match bound with
    | .unbounded => .unbounded
    | .lowerBounded lower => .upperBounded (scalar * lower)
    | .upperBounded upper => .lowerBounded (scalar * upper)
    | .finite lower upper hvalid =>
        .finite (scalar * upper) (scalar * lower)
          (mul_le_mul_of_nonpos_left hvalid (le_of_not_gt hpositive))

theorem scale_holds {bound : Bound} {scalar value : Rat}
    (hvalue : value ∈ bound) :
    scalar * value ∈ bound.scale scalar := by
  change bound.Holds value at hvalue
  change (bound.scale scalar).Holds (scalar * value)
  unfold scale
  split_ifs with hzero hpositive
  · subst scalar
    simp only [point, Holds, zero_mul, le_refl, and_self]
  · cases bound <;> simp only [Holds] at hvalue ⊢
    all_goals first | (constructor <;> nlinarith) | nlinarith
  · have hnonpos : scalar ≤ 0 := le_of_not_gt hpositive
    cases bound <;> simp only [Holds] at hvalue ⊢
    all_goals first | (constructor <;> nlinarith) | nlinarith

end Bound

/-! ## Decision-variable domains -/

/-- The possible values of one decision variable. -/
inductive Domain where
  | binary
  | integer (bound : Bound := .unbounded)
  | continuous (bound : Bound := .unbounded)
  deriving DecidableEq, Repr

namespace Domain

/-- The rational interval owned by a decision-variable domain. -/
def bound : Domain → Bound
  | .binary => .binary
  | .integer bound => bound
  | .continuous bound => bound

/-- Whether a rational value belongs to a decision-variable domain. -/
def Holds : Domain → Rat → Prop
  | .binary, value => value = 0 ∨ value = 1
  | .integer bound, value => value.den = 1 ∧ value ∈ bound
  | .continuous bound, value => value ∈ bound

instance (domain : Domain) (value : Rat) : Decidable (domain.Holds value) := by
  cases domain <;> simp only [Holds] <;> infer_instance

instance : Membership Rat Domain where
  mem domain value := domain.Holds value

instance (domain : Domain) (value : Rat) : Decidable (value ∈ domain) := by
  change Decidable (domain.Holds value)
  infer_instance

@[simp]
theorem mem_binary_iff {value : Rat} :
    value ∈ Domain.binary ↔ value = 0 ∨ value = 1 :=
  Iff.rfl

@[simp]
theorem mem_integer_iff {bound : Bound} {value : Rat} :
    value ∈ Domain.integer bound ↔
      value.den = 1 ∧ value ∈ bound :=
  Iff.rfl

@[simp]
theorem mem_continuous_iff {bound : Bound} {value : Rat} :
    value ∈ Domain.continuous bound ↔ value ∈ bound :=
  Iff.rfl

theorem mem_bound {domain : Domain} {value : Rat}
    (hvalue : value ∈ domain) :
    value ∈ domain.bound := by
  cases domain with
  | binary =>
      rcases hvalue with rfl | rfl <;>
        simp [bound, Bound.binary]
  | integer bound => exact hvalue.2
  | continuous bound => exact hvalue

theorem lower_le {domain : Domain} {value : Rat}
    (hvalue : value ∈ domain) :
    domain.bound.lower ≤ .finite value :=
  Bound.lower_le_finite (mem_bound hvalue)

theorem le_upper {domain : Domain} {value : Rat}
    (hvalue : value ∈ domain) :
    (.finite value : Endpoint) ≤ domain.bound.upper :=
  Bound.finite_le_upper (mem_bound hvalue)

theorem finite_lower_le {domain : Domain} {value lower : Rat}
    (hvalue : value ∈ domain)
    (hlower : domain.bound.lower = .finite lower) :
    lower ≤ value :=
  Bound.finite_lower_le (mem_bound hvalue) hlower

theorem le_finite_upper {domain : Domain} {value upper : Rat}
    (hvalue : value ∈ domain)
    (hupper : domain.bound.upper = .finite upper) :
    value ≤ upper :=
  Bound.le_finite_upper (mem_bound hvalue) hupper

@[simp]
theorem binary_zero : (0 : Rat) ∈ Domain.binary := by
  simp [Membership.mem, Holds]

@[simp]
theorem binary_one : (1 : Rat) ∈ Domain.binary := by
  simp [Membership.mem, Holds]

end Domain

end OMMXProof
