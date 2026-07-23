import OMMXProofTest.Fixtures

/-!
# Independent semantics acceptance fixtures

`lake test` elaborates this test-only library. Executable checker fixtures use
`native_decide`; no OMMX runtime artifact is consumed.
-/

namespace OMMXProof.Test

open Fixtures

example : constantOnlyAffine2.eval (fun _ => 0) = 1 := by native_decide
example : checkOneHot binaryDomains2 scaledOneHotSource oneHotDraft = true := by
  native_decide
example : checkOneHot binaryDomains2 wrongSenseOneHotSource
    { members := allTwo, scale := 1 } = false := by native_decide
example : checkBinaryCardinalitySOS1 binaryDomains2
    scaledSOS1Source sos1Draft = true := by native_decide
example : checkBinaryCardinalitySOS1 binaryDomains2
    wrongSenseSOS1Source sos1Draft = false := by native_decide
example : selectorIsolationBase.checkSelectorIsolation
    selectorIsolationWitness = true := by native_decide
example : selectorLeakingBase.checkSelectorIsolation
    selectorIsolationWitness = false := by native_decide
example (state : State 2) :
    IndicatorBigM.LowerSide sdkIndicatorBody 1 0 state := by
  simp [IndicatorBigM.LowerSide]

end OMMXProof.Test
