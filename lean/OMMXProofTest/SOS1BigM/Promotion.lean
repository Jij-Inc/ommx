import OMMXProof.Instance.Transform.SOS1BigM.Promotion

/-!
# SOS1 Big-M promotion fixtures

The accepted fixture is an independently written flat selector formulation.
It does not call SOS1 lowering, so promotion validation depends only on the
flat `Instance` and the user-supplied witness.

The retained variables are a reused binary member `x` and a bounded continuous
member `y ∈ [-2, 3]`.  A fresh binary selector `z` is appended at the end.
The retained regular row is followed by the exact upper link, lower link, and
cardinality suffix:

* `x + y ≤ 2`,
* `y - 3z ≤ 0`,
* `-y - 2z ≤ 0`,
* `x + z ≤ 1`.

The rejection fixtures exercise the initial checker as a conservative
sufficient-condition recognizer.  Rejection is intentionally not interpreted
as evidence that no correct promotion exists.
-/

namespace OMMXProof.Test.SOS1BigM.Promotion

open Instance.SOS1BigM

def members : Finset (Fin 2) := Finset.univ

def freshMembers : Finset {i // i ∈ members} :=
  {⟨1, by native_decide⟩}

def witness : Witness 2 where
  members := members
  freshMembers := freshMembers
  retainedConstraintCount := 1

example : witness.freshCount = 1 := by
  native_decide

def freshZero : Fin witness.freshCount :=
  ⟨0, by native_decide⟩

theorem freshMember_zero_val :
    (witness.freshMember freshZero).val = 1 := by
  native_decide

/-! ## Independently written flat source -/

def flatDomains : Fin (2 + witness.freshCount) → Domain :=
  fun i =>
    if i.val = 0 then .binary
    else if i.val = 1 then .continuous (.finite (-2) 3 (by norm_num))
    else .binary

def affine (x y z constant : Rat) :
    Affine (2 + witness.freshCount) where
  coeff := fun i =>
    if i.val = 0 then x
    else if i.val = 1 then y
    else z
  constant := constant

def row (x y z constant : Rat) :
    LinearConstraint (2 + witness.freshCount) where
  expr := affine x y z constant
  sense := .lessEqual

def retainedRow : LinearConstraint (2 + witness.freshCount) :=
  row 1 1 0 (-2)

def upperLink : LinearConstraint (2 + witness.freshCount) :=
  row 0 1 (-3) 0

def lowerLink : LinearConstraint (2 + witness.freshCount) :=
  row 0 (-1) (-2) 0

def cardinality : LinearConstraint (2 + witness.freshCount) :=
  row 1 0 1 (-1)

def objective : Affine (2 + witness.freshCount) :=
  affine 1 2 0 0

def source : Instance (2 + witness.freshCount) where
  domains := flatDomains
  constraints := [retainedRow, upperLink, lowerLink, cardinality]
  oneHotConstraints := []
  sos1Constraints := []
  indicatorConstraints := []
  objective := objective
  sense := .minimize

theorem source_validate_isSome :
    (witness.validate source).isSome = true := by
  native_decide

def validated : witness.Validated source :=
  (witness.validate source).get source_validate_isSome

def transform : Instance.Transform source :=
  promotion witness source

example : transform.targetDimension = 2 := by
  rfl

example : transform.target.constraints.length = 1 := by
  native_decide

example : transform.target.sos1Constraints.length = 1 := by
  native_decide

example :
    (transform.target.sos1Constraints.get ⟨0, by native_decide⟩).members =
      members := by
  native_decide

example : transform.IsReduction :=
  promotion_isReduction validated

example : transform.IsRelaxation :=
  promotion_isRelaxation validated

example : transform.SensePreserving :=
  promotion_sensePreserving witness source

example : transform.SourceObjectiveValuePreserving :=
  promotion_sourceObjectiveValuePreserving validated

example : transform.TargetObjectiveValuePreserving :=
  promotion_targetObjectiveValuePreserving validated

example : transform.SourceObjectivePreserving :=
  promotion_sourceObjectivePreserving validated

example : transform.TargetObjectivePreserving :=
  promotion_targetObjectivePreserving validated

example : transform.TargetRoundTrip :=
  promotion_targetRoundTrip witness source

/-! ## Conservative rejection fixtures -/

def selectorNonbinarySource : Instance (2 + witness.freshCount) :=
  { source with
    domains := fun i =>
      if i.val = 2 then .continuous (.finite 0 1 (by norm_num))
      else flatDomains i }

def unboundedMemberSource : Instance (2 + witness.freshCount) :=
  { source with
    domains := fun i =>
      if i.val = 0 then .binary
      else if i.val = 1 then .continuous
      else .binary }

def badUpperSource : Instance (2 + witness.freshCount) :=
  { source with
    constraints :=
      [retainedRow, row 0 1 (-4) 0, lowerLink, cardinality] }

def badCardinalitySource : Instance (2 + witness.freshCount) :=
  { source with
    constraints :=
      [retainedRow, upperLink, lowerLink, row 1 0 1 (-2)] }

def objectiveDependencySource : Instance (2 + witness.freshCount) :=
  { source with objective := affine 1 2 1 0 }

def retainedDependencySource : Instance (2 + witness.freshCount) :=
  { source with
    constraints :=
      [row 1 1 1 (-2), upperLink, lowerLink, cardinality] }

/-- Member `x` is still declared reused by the witness but is no longer
binary. The adjusted cardinality row matches the domains, isolating the
reused-member classification check. -/
def reusedNonbinarySource : Instance (2 + witness.freshCount) :=
  { source with
    domains := fun i =>
      if i.val = 0 then .continuous (.finite 0 1 (by norm_num))
      else flatDomains i
    constraints :=
      [retainedRow, upperLink, lowerLink, row 0 0 1 (-1)] }

def existingOneHotSource : Instance (2 + witness.freshCount) :=
  { source with
    oneHotConstraints := [{ members := {0} }] }

def existingSOS1Source : Instance (2 + witness.freshCount) :=
  { source with
    sos1Constraints := [{ members := {0} }] }

def existingIndicatorSource : Instance (2 + witness.freshCount) :=
  { source with
    indicatorConstraints :=
      [{ trigger := 0
         polarity := .activeOnOne
         body := retainedRow }] }

theorem existingOneHotSource_validate_isSome :
    (witness.validate existingOneHotSource).isSome = true := by
  native_decide

def existingOneHotTransform : Instance.Transform existingOneHotSource :=
  promotion witness existingOneHotSource

example : existingOneHotTransform.target.oneHotConstraints.length = 1 := by
  native_decide

example :
    (existingOneHotTransform.target.oneHotConstraints.get
      ⟨0, by native_decide⟩).members = ({0} : Finset (Fin 2)) := by
  native_decide

theorem existingSOS1Source_validate_isSome :
    (witness.validate existingSOS1Source).isSome = true := by
  native_decide

def existingSOS1Transform : Instance.Transform existingSOS1Source :=
  promotion witness existingSOS1Source

/-- A previously promoted SOS1 is retained before the newly promoted one. -/
example : existingSOS1Transform.target.sos1Constraints.length = 2 := by
  native_decide

example :
    (existingSOS1Transform.target.sos1Constraints.get
      ⟨0, by native_decide⟩).members = ({0} : Finset (Fin 2)) := by
  native_decide

example :
    (existingSOS1Transform.target.sos1Constraints.get
      ⟨1, by native_decide⟩).members = members := by
  native_decide

theorem existingIndicatorSource_validate_isSome :
    (witness.validate existingIndicatorSource).isSome = true := by
  native_decide

def existingIndicatorTransform : Instance.Transform existingIndicatorSource :=
  promotion witness existingIndicatorSource

example : existingIndicatorTransform.target.indicatorConstraints.length = 1 := by
  native_decide

example :
    (existingIndicatorTransform.target.indicatorConstraints.get
      ⟨0, by native_decide⟩).trigger.val = 0 := by
  native_decide

example :
    let body :=
      (existingIndicatorTransform.target.indicatorConstraints.get
        ⟨0, by native_decide⟩).body
    body.expr.coeff (0 : Fin 2) = 1 ∧
      body.expr.coeff (1 : Fin 2) = 1 ∧
      body.expr.constant = -2 ∧
      body.sense = .lessEqual := by
  native_decide

def freshOneHotMemberSource : Instance (2 + witness.freshCount) :=
  { source with
    oneHotConstraints := [{ members := {0, 2} }] }

def freshSOS1MemberSource : Instance (2 + witness.freshCount) :=
  { source with
    sos1Constraints := [{ members := {0, 2} }] }

def freshIndicatorTriggerSource : Instance (2 + witness.freshCount) :=
  { source with
    indicatorConstraints :=
      [{ trigger := 2
         polarity := .activeOnOne
         body := retainedRow }] }

def freshIndicatorBodySource : Instance (2 + witness.freshCount) :=
  { source with
    indicatorConstraints :=
      [{ trigger := 0
         polarity := .activeOnOne
         body := row 0 0 1 0 }] }

example : witness.validate selectorNonbinarySource = none := by
  native_decide

example : witness.validate unboundedMemberSource = none := by
  native_decide

example : witness.validate badUpperSource = none := by
  native_decide

example : witness.validate badCardinalitySource = none := by
  native_decide

example : witness.validate objectiveDependencySource = none := by
  native_decide

example : witness.validate retainedDependencySource = none := by
  native_decide

example : witness.validate reusedNonbinarySource = none := by
  native_decide

example : witness.validate freshOneHotMemberSource = none := by
  native_decide

example : witness.validate freshSOS1MemberSource = none := by
  native_decide

example : witness.validate freshIndicatorTriggerSource = none := by
  native_decide

example : witness.validate freshIndicatorBodySource = none := by
  native_decide

/-! ## Multiple sequential SOS1 promotions -/

namespace MultipleSOS1

def members : Finset (Fin 3) := {0, 1}

def freshMembers : Finset {i // i ∈ members} :=
  {⟨1, by native_decide⟩}

def witness : Witness 3 where
  members := members
  freshMembers := freshMembers
  retainedConstraintCount := 0

def domains : Fin (3 + witness.freshCount) → Domain :=
  fun i =>
    if i.val = 0 then .binary
    else if i.val = 1 then .continuous (.finite (-2) 3 (by norm_num))
    else if i.val = 2 then .continuous (.finite (-1) 1 (by norm_num))
    else .binary

/-- The formulation rows are generated here to keep this fixture focused on
preservation of a distinct SOS1 recovered by an earlier promotion. -/
def skeleton : Instance (3 + witness.freshCount) where
  domains := domains
  constraints := []
  oneHotConstraints := []
  sos1Constraints := []
  indicatorConstraints := []
  objective := Affine.zero
  sense := .minimize

def source : Instance (3 + witness.freshCount) :=
  { skeleton with
    constraints :=
      witness.selectorLayout.canonicalRows (witness.selectorBounds skeleton)
    sos1Constraints := [{ members := {1, 2} }] }

theorem source_validate_isSome :
    (witness.validate source).isSome = true := by
  native_decide

def validated : witness.Validated source :=
  (witness.validate source).get source_validate_isSome

def transform : Instance.Transform source :=
  promotion witness source

example : transform.target.sos1Constraints.length = 2 := by
  native_decide

example :
    (transform.target.sos1Constraints.get
      ⟨0, by native_decide⟩).members = ({1, 2} : Finset (Fin 3)) := by
  native_decide

example :
    (transform.target.sos1Constraints.get
      ⟨1, by native_decide⟩).members = members := by
  native_decide

example : transform.IsReduction :=
  promotion_isReduction validated

example : transform.IsRelaxation :=
  promotion_isRelaxation validated

end MultipleSOS1

/-! ## Noncanonical feasible flat state -/

def noncanonicalSourceState : State (2 + witness.freshCount) :=
  fun i => if i.val = 2 then 1 else 0

theorem noncanonicalSourceState_feasible :
    source.Feasible noncanonicalSourceState := by
  unfold Instance.Feasible
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · intro i
    fin_cases i <;> native_decide
  · intro constraint hconstraint
    simp only [source, List.mem_cons, List.not_mem_nil, or_false] at hconstraint
    rcases hconstraint with rfl | rfl | rfl | rfl
    all_goals native_decide
  · simp [source]
  · simp [source]
  · simp [source]

/-- The flat source admits a noncanonical selector value when both members are
zero. Promotion projects it away and canonical decoding restores `z = 0`, so
the source-side round trip is intentionally not guaranteed. -/
example : ¬transform.SourceRoundTrip := by
  intro hroundTrip
  have hstate := hroundTrip noncanonicalSourceState_feasible
  change
    some
        (State.append (State.source noncanonicalSourceState) fun j =>
          canonicalSelector
              (witness.memberState (State.source noncanonicalSourceState))
              (witness.freshMember j)) =
      some noncanonicalSourceState at hstate
  have heq :
      State.append (State.source noncanonicalSourceState) (fun j =>
          canonicalSelector
            (witness.memberState (State.source noncanonicalSourceState))
            (witness.freshMember j)) =
        noncanonicalSourceState :=
    Option.some.inj hstate
  have hcomponent := congrArg
    (fun state => state (Fin.natAdd 2 freshZero)) heq
  simp [noncanonicalSourceState, Witness.memberState,
    SelectorLayout.memberState, State.source, State.append,
    canonicalSelector, freshMember_zero_val] at hcomponent

end OMMXProof.Test.SOS1BigM.Promotion
