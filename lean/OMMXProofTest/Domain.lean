import OMMXProof.Domain

namespace OMMXProof.Test.Domain

example : (0 : Rat) ∈ OMMXProof.Domain.binary := by native_decide

example : (1 : Rat) ∈ OMMXProof.Domain.binary := by native_decide

example : (2 : Rat) ∉ OMMXProof.Domain.binary := by native_decide

example : Endpoint.negInf < Endpoint.finite (-2) := by native_decide

example : Endpoint.finite (-2) < Endpoint.finite 3 := by native_decide

example : Endpoint.finite 3 < Endpoint.posInf := by native_decide

def finiteBound : Bound :=
  .finite (-2) 3 (by norm_num)

example : (1 / 2 : Rat) ∈ finiteBound := by native_decide

example : (4 : Rat) ∉ finiteBound := by native_decide

example : (-2 : Rat) ∈ Bound.lowerBounded (-2) := by native_decide

example : (3 : Rat) ∈ Bound.upperBounded 3 := by native_decide

def boundedInteger : OMMXProof.Domain :=
  .integer finiteBound

example : (2 : Rat) ∈ boundedInteger := by native_decide

example : (4 : Rat) ∉ boundedInteger := by native_decide

example : (1 / 2 : Rat) ∉ boundedInteger := by native_decide

def boundedContinuous : OMMXProof.Domain :=
  .continuous finiteBound

example : (1 / 2 : Rat) ∈ boundedContinuous := by native_decide

example : (4 : Rat) ∉ boundedContinuous := by native_decide

/-- A nonempty rational interval need not contain an integer. -/
def halfPointInteger : OMMXProof.Domain :=
  .integer (Bound.point (1 / 2))

example : (1 / 2 : Rat) ∈ Bound.point (1 / 2) := by native_decide

example (value : Rat) : value ∉ halfPointInteger := by
  intro hvalue
  have hbounds := hvalue.2
  have hequal : value = 1 / 2 :=
    le_antisymm hbounds.2 hbounds.1
  subst value
  have hnotInteger : (1 / 2 : Rat).den ≠ 1 := by native_decide
  exact hnotInteger hvalue.1

end OMMXProof.Test.Domain
