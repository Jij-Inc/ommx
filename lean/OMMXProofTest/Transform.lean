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

example : (Instance.Transform.refl emptyInstance).SensePreserving :=
  Instance.Transform.refl_sensePreserving emptyInstance

example :
    (Instance.Transform.refl emptyInstance).SourceObjectiveValuePreserving :=
  Instance.Transform.refl_sourceObjectiveValuePreserving emptyInstance

example :
    (Instance.Transform.refl emptyInstance).TargetObjectiveValuePreserving :=
  Instance.Transform.refl_targetObjectiveValuePreserving emptyInstance

example : (Instance.Transform.refl emptyInstance).SourceObjectivePreserving :=
  Instance.Transform.refl_sourceObjectivePreserving emptyInstance

example : (Instance.Transform.refl emptyInstance).TargetObjectivePreserving :=
  Instance.Transform.refl_targetObjectivePreserving emptyInstance

example : (Instance.Transform.refl emptyInstance).SourceRoundTrip :=
  Instance.Transform.refl_sourceRoundTrip emptyInstance

example : (Instance.Transform.refl emptyInstance).TargetRoundTrip :=
  Instance.Transform.refl_targetRoundTrip emptyInstance

example :
    (Instance.Transform.comp
      (Instance.Transform.refl emptyInstance)
      (Instance.Transform.refl emptyInstance)).SourceObjectivePreserving :=
  Instance.Transform.comp_sourceObjectivePreserving
    (Instance.Transform.refl_isRelaxation emptyInstance)
    (Instance.Transform.refl_sourceObjectivePreserving emptyInstance)
    (Instance.Transform.refl_sourceObjectivePreserving emptyInstance)

example :
    (Instance.Transform.comp
      (Instance.Transform.refl emptyInstance)
      (Instance.Transform.refl emptyInstance)).TargetObjectivePreserving :=
  Instance.Transform.comp_targetObjectivePreserving
    (Instance.Transform.refl_isReduction emptyInstance)
    (Instance.Transform.refl_targetObjectivePreserving emptyInstance)
    (Instance.Transform.refl_targetObjectivePreserving emptyInstance)

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

theorem nowhereDefined_not_sourceObjectivePreserving :
    ¬nowhereDefined.SourceObjectivePreserving := by
  intro h
  let state : State 0 := fun i => nomatch i
  have hfeasible : emptyInstance.Feasible state := by
    simp [emptyInstance, Instance.Feasible]
  have hobjective := h.2 hfeasible
  simp [nowhereDefined] at hobjective

theorem nowhereDefined_not_targetObjectivePreserving :
    ¬nowhereDefined.TargetObjectivePreserving := by
  intro h
  let state : State 0 := fun i => nomatch i
  have hfeasible : emptyInstance.Feasible state := by
    simp [emptyInstance, Instance.Feasible]
  have hobjective := h.2 hfeasible
  simp [nowhereDefined] at hobjective

/-- Feasibility can be preserved even when the target objective is changed. -/
def shiftedObjectiveInstance : Instance 0 :=
  { emptyInstance with
    objective :=
      { coeff := fun i => nomatch i
        constant := 1 } }

def shiftedObjective : Instance.Transform emptyInstance where
  targetDimension := 0
  target := shiftedObjectiveInstance
  encode := some
  decode := some

theorem shiftedObjective_isReduction :
    shiftedObjective.IsReduction := by
  intro targetState hfeasible
  exact ⟨targetState, rfl, hfeasible⟩

theorem shiftedObjective_isRelaxation :
    shiftedObjective.IsRelaxation := by
  intro sourceState hfeasible
  exact ⟨sourceState, rfl, hfeasible⟩

theorem shiftedObjective_not_sourceObjectivePreserving :
    ¬shiftedObjective.SourceObjectivePreserving := by
  intro h
  let state : State 0 := fun i => nomatch i
  have hfeasible : emptyInstance.Feasible state := by
    simp [emptyInstance, Instance.Feasible]
  have hobjective := h.2 hfeasible
  simp [shiftedObjective, shiftedObjectiveInstance, emptyInstance,
    Instance.ObjectiveValue, Affine.eval, Affine.zero] at hobjective

theorem shiftedObjective_not_targetObjectivePreserving :
    ¬shiftedObjective.TargetObjectivePreserving := by
  intro h
  let state : State 0 := fun i => nomatch i
  have hfeasible : shiftedObjective.target.Feasible state := by
    change emptyInstance.Feasible state
    simp [emptyInstance, Instance.Feasible]
  have hobjective := h.2 hfeasible
  simp [shiftedObjective, shiftedObjectiveInstance, emptyInstance,
    Instance.ObjectiveValue, Affine.eval, Affine.zero] at hobjective

/-- Equal objective values do not preserve the full objective when the
optimization sense is reversed. -/
def flippedSenseInstance : Instance 0 :=
  { emptyInstance with sense := .maximize }

def flippedSense : Instance.Transform emptyInstance where
  targetDimension := 0
  target := flippedSenseInstance
  encode := some
  decode := some

theorem flippedSense_sourceObjectiveValuePreserving :
    flippedSense.SourceObjectiveValuePreserving := by
  intro sourceState _
  rfl

theorem flippedSense_targetObjectiveValuePreserving :
    flippedSense.TargetObjectiveValuePreserving := by
  intro targetState _
  rfl

theorem flippedSense_not_sensePreserving :
    ¬flippedSense.SensePreserving := by
  simp [Instance.Transform.SensePreserving, flippedSense,
    flippedSenseInstance, emptyInstance]

theorem flippedSense_not_sourceObjectivePreserving :
    ¬flippedSense.SourceObjectivePreserving :=
  fun h => flippedSense_not_sensePreserving h.1

theorem flippedSense_not_targetObjectivePreserving :
    ¬flippedSense.TargetObjectivePreserving :=
  fun h => flippedSense_not_sensePreserving h.1

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
