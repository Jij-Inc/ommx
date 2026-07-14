import OMMXProofTest.Fixtures

/-!
# Independent semantics acceptance fixtures

`lake test` elaborates this test-only library. Executable checker fixtures use
`native_decide`; no OMMX runtime artifact is consumed.
-/

namespace OMMXProof.Test

open Fixtures

example : constantOnlyAffine2.eval (fun _ => 0) = 1 := by native_decide
example : twiceUpperWitness.checkImplication twiceUpper = true := by native_decide
example : twiceUpperWitness.checkImplication tooStrongTarget = false := by native_decide
example : impossibleWitness.checkInfeasibility = true := by native_decide
example : invalidImpossibleWitness.checkInfeasibility = false := by native_decide
example : fixedEqualityWitness.check (oneVarAffine 1 0) = true := by native_decide
example : invalidFixedEqualityWitness.check (oneVarAffine 1 0) = false := by
  native_decide
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
example : checkIndicatorReplace indicatorDomains indicatorSurviving
    indicatorSource indicatorBody 1 .activeOnOne indicatorWitness = true := by
  native_decide
example : checkEqualityIndicatorReplace indicatorDomains indicatorEqualitySurviving
    indicatorEqualitySource indicatorEqualityBody 1 .activeOnOne
      indicatorEqualityWitness = true := by
  native_decide
example : checkEqualityIndicatorReplace indicatorDomains indicatorEqualitySurviving
    indicatorEqualitySource indicatorEqualityBody 1 .activeOnOne
      oneSidedEqualityWitness = false := by
  native_decide
example (assignment : Assignment 2) :
    IndicatorBigM.LowerSide sdkIndicatorBody 1 0 assignment := by
  simp [IndicatorBigM.LowerSide]
example : PlannedSelectorGadget plannedReusedExample plannedBoundsExample
    plannedMembersExample plannedFreshSelectorsExample := by
  native_decide
example : ¬PlannedSelectorGadget plannedReusedExample plannedBoundsExample
    invalidPlannedMembersExample plannedFreshSelectorsExample := by
  native_decide
example : ¬FreshBoundsContainZero plannedReusedExample
    zeroExcludingFreshBoundsExample := by
  native_decide

end OMMXProof.Test
