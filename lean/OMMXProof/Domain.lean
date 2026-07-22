import Mathlib.Algebra.Order.Ring.Rat

/-!
# Decision-variable domains

This module defines the possible values of a decision variable.
-/

namespace OMMXProof

/-- The set of values allowed for a decision variable. Missing endpoints denote
an interval unbounded on that side. -/
inductive Domain where
  | binary
  | integer (lower upper : Option Int := none)
  | continuous (lower upper : Option Rat := none)
  deriving DecidableEq, Repr

namespace Domain

private def LowerHolds {α : Type*} [LE α]
    (lower : Option α) (value : α) : Prop :=
  match lower with
  | none => True
  | some lower => lower ≤ value

private def UpperHolds {α : Type*} [LE α]
    (upper : Option α) (value : α) : Prop :=
  match upper with
  | none => True
  | some upper => value ≤ upper

private def IntegerLowerHolds (lower : Option Int) (value : Rat) : Prop :=
  match lower with
  | none => True
  | some lower => (lower : Rat) ≤ value

private def IntegerUpperHolds (upper : Option Int) (value : Rat) : Prop :=
  match upper with
  | none => True
  | some upper => value ≤ (upper : Rat)

/-- Whether a rational value belongs to a decision-variable domain. -/
def Holds : Domain → Rat → Prop
  | .binary, value => value = 0 ∨ value = 1
  | .integer lower upper, value =>
      value.den = 1 ∧
        IntegerLowerHolds lower value ∧ IntegerUpperHolds upper value
  | .continuous lower upper, value =>
      LowerHolds lower value ∧ UpperHolds upper value

instance (domain : Domain) (value : Rat) : Decidable (domain.Holds value) := by
  cases domain with
  | binary => simp only [Holds]; infer_instance
  | integer lower upper =>
      cases lower <;> cases upper <;>
        simp only [Holds, IntegerLowerHolds, IntegerUpperHolds] <;> infer_instance
  | continuous lower upper =>
      cases lower <;> cases upper <;>
        simp only [Holds, LowerHolds, UpperHolds] <;> infer_instance

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
theorem mem_integer_iff {lower upper : Option Int} {value : Rat} :
    value ∈ Domain.integer lower upper ↔
      value.den = 1 ∧
        (match lower with
         | none => True
         | some lower => (lower : Rat) ≤ value) ∧
        (match upper with
         | none => True
         | some upper => value ≤ (upper : Rat)) :=
  Iff.rfl

@[simp]
theorem mem_continuous_iff {lower upper : Option Rat} {value : Rat} :
    value ∈ Domain.continuous lower upper ↔
      (match lower with
       | none => True
       | some lower => lower ≤ value) ∧
      (match upper with
       | none => True
       | some upper => value ≤ upper) := by
  cases lower <;> cases upper <;> rfl

/-- The finite lower endpoint of a domain, if present. -/
def lowerBound : Domain → Option Rat
  | .binary => some 0
  | .integer none _ => none
  | .integer (some lower) _ => some (lower : Rat)
  | .continuous lower _ => lower

/-- The finite upper endpoint of a domain, if present. -/
def upperBound : Domain → Option Rat
  | .binary => some 1
  | .integer _ none => none
  | .integer _ (some upper) => some (upper : Rat)
  | .continuous _ upper => upper

theorem lowerBound_le {domain : Domain} {value lower : Rat}
    (hvalue : value ∈ domain) (hbound : domain.lowerBound = some lower) :
    lower ≤ value := by
  cases domain with
  | binary =>
      simp only [lowerBound, Option.some.injEq] at hbound
      subst lower
      rcases hvalue with rfl | rfl <;> simp
  | integer domainLower domainUpper =>
      cases domainLower with
      | none => simp [lowerBound] at hbound
      | some domainLower =>
          simp only [lowerBound, Option.some.injEq] at hbound
          subst lower
          exact hvalue.2.1
  | continuous domainLower domainUpper =>
      cases domainLower with
      | none => simp [lowerBound] at hbound
      | some domainLower =>
          simp only [lowerBound, Option.some.injEq] at hbound
          subst lower
          exact hvalue.1

theorem le_upperBound {domain : Domain} {value upper : Rat}
    (hvalue : value ∈ domain) (hbound : domain.upperBound = some upper) :
    value ≤ upper := by
  cases domain with
  | binary =>
      simp only [upperBound, Option.some.injEq] at hbound
      subst upper
      rcases hvalue with rfl | rfl <;> simp
  | integer domainLower domainUpper =>
      cases domainUpper with
      | none => simp [upperBound] at hbound
      | some domainUpper =>
          simp only [upperBound, Option.some.injEq] at hbound
          subst upper
          exact hvalue.2.2
  | continuous domainLower domainUpper =>
      cases domainUpper with
      | none => simp [upperBound] at hbound
      | some domainUpper =>
          simp only [upperBound, Option.some.injEq] at hbound
          subst upper
          exact hvalue.2

/-- A domain containing every rational value. -/
def Unrestricted (domain : Domain) : Prop :=
  domain = .continuous none none

instance (domain : Domain) : Decidable domain.Unrestricted := by
  unfold Unrestricted
  infer_instance

theorem holds_of_unrestricted {domain : Domain}
    (hunrestricted : domain.Unrestricted) (value : Rat) :
    value ∈ domain := by
  rw [hunrestricted]
  simp [Membership.mem, Holds, LowerHolds, UpperHolds]

@[simp]
theorem binary_zero : (0 : Rat) ∈ Domain.binary := by
  simp [Membership.mem, Holds]

@[simp]
theorem binary_one : (1 : Rat) ∈ Domain.binary := by
  simp [Membership.mem, Holds]

end Domain

end OMMXProof
