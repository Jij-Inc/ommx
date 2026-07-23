import OMMXProof.Instance.Transform

namespace OMMXProof.Test.Transform

def emptyInstance : Instance 0 where
  domains := fun i => nomatch i
  constraints := []
  objective := Affine.zero
  sense := .minimize

example : (Instance.Transform.refl emptyInstance).IsReduction :=
  Instance.Transform.refl_isReduction emptyInstance

example : (Instance.Transform.refl emptyInstance).IsRelaxation :=
  Instance.Transform.refl_isRelaxation emptyInstance

example : (Instance.Transform.refl emptyInstance).SourceRoundTrip :=
  Instance.Transform.refl_sourceRoundTrip emptyInstance

example : (Instance.Transform.refl emptyInstance).TargetRoundTrip :=
  Instance.Transform.refl_targetRoundTrip emptyInstance

/-- A raw Transform may fail both directional feasibility contracts. -/
def nowhereDefined : Instance.Transform emptyInstance where
  targetDimension := 0
  target := emptyInstance
  encode := fun _ => none
  decode := fun _ => none

theorem nowhereDefined_not_reduction :
    ¬nowhereDefined.IsReduction := by
  intro h
  let state : State 0 := fun i => nomatch i
  have hfeasible : emptyInstance.Feasible state := by
    simp [emptyInstance, Instance.Feasible]
  rcases h hfeasible with ⟨sourceState, hdecode, _⟩
  simp [nowhereDefined] at hdecode

theorem nowhereDefined_not_relaxation :
    ¬nowhereDefined.IsRelaxation := by
  intro h
  let state : State 0 := fun i => nomatch i
  have hfeasible : emptyInstance.Feasible state := by
    simp [emptyInstance, Instance.Feasible]
  rcases h hfeasible with ⟨targetState, hencode, _⟩
  simp [nowhereDefined] at hencode

theorem nowhereDefined_not_sourceRoundTrip :
    ¬nowhereDefined.SourceRoundTrip := by
  intro h
  let state : State 0 := fun i => nomatch i
  have hfeasible : emptyInstance.Feasible state := by
    simp [emptyInstance, Instance.Feasible]
  have hroundTrip := h hfeasible
  simp [nowhereDefined] at hroundTrip

theorem nowhereDefined_not_targetRoundTrip :
    ¬nowhereDefined.TargetRoundTrip := by
  intro h
  let state : State 0 := fun i => nomatch i
  have hfeasible : emptyInstance.Feasible state := by
    simp [emptyInstance, Instance.Feasible]
  have hroundTrip := h hfeasible
  simp [nowhereDefined] at hroundTrip

end OMMXProof.Test.Transform
