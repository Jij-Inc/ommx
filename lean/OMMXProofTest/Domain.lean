import OMMXProof.Domain

namespace OMMXProof.Test.Domain

example : (0 : Rat) ∈ OMMXProof.Domain.binary := by native_decide

example : (1 : Rat) ∈ OMMXProof.Domain.binary := by native_decide

example : (2 : Rat) ∉ OMMXProof.Domain.binary := by native_decide

def boundedInteger : OMMXProof.Domain :=
  .integer (some (-2)) (some 3)

example : (2 : Rat) ∈ boundedInteger := by native_decide

example : (4 : Rat) ∉ boundedInteger := by native_decide

example : (1 / 2 : Rat) ∉ boundedInteger := by native_decide

def boundedContinuous : OMMXProof.Domain :=
  .continuous (some (-2)) (some 3)

example : (1 / 2 : Rat) ∈ boundedContinuous := by native_decide

example : (4 : Rat) ∉ boundedContinuous := by native_decide

end OMMXProof.Test.Domain
