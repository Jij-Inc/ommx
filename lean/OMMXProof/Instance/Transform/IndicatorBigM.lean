import OMMXProof.Instance.Transform
import OMMXProof.Instance.Transform.IndicatorBigM.Target
import Mathlib.Tactic

/-!
# Indicator Big-M lowering as an `Instance.Transform`

This module writes the SDK Indicator conversion as an actual
`Instance.Transform`, consisting of a target instance and encode/decode maps
between the source and target state spaces.

The current lowering supports active-on-one Indicators. It introduces no
decision variable, so the source and target state spaces are identical and both
state maps are the identity.

## Terminology

- The **trigger** is the binary decision-variable component whose active value
  determines whether the Indicator body must hold.
- The **body** is the affine equality or inequality guarded by the trigger.
- The **body bound** is the rational interval computed from the source domain
  box and proved to contain every body value.
- The **upper side** is the generated Big-M inequality based on the finite upper
  body bound. It is omitted when that bound already implies the body inequality.
- The **lower side** is the corresponding inequality based on the finite lower
  body bound. It is needed only for equality bodies and is omitted when the
  lower bound already implies nonnegativity.
- A **plan** selects the occurrence of the source Indicator constraint to lower.
  A **validated plan** additionally proves that the trigger and polarity are
  supported and that every required body-bound endpoint is finite.
- The **target** removes the selected Indicator and appends its nonredundant
  Big-M sides to the regular linear constraints.
-/

namespace OMMXProof

namespace Instance

namespace IndicatorBigM

/--
# Indicator Big-M lowering

## Lowering rule

- The selected active-on-one Indicator is removed.
- Its nonredundant Big-M sides are appended as regular linear constraints.
- No decision-variable component is added or removed.
- `lowering` is partial: it returns `none` exactly when the plan is invalid.

## Example

Suppose `x ∈ [0, 3]`, `y ∈ Binary`, and the selected Indicator is
`y = 1 → x = 0`. Lowering replaces it with the regular constraint
`x + 3y - 3 ≤ 0`.

- `y` is the trigger and `x = 0` is the equality body.
- `[0, 3]` is the computed body bound.
- `x + 3y - 3 ≤ 0` is the upper side obtained from the upper bound `3`.
- The lower side is omitted because the lower bound is zero, so the source
  domain already guarantees `0 ≤ x`.
- The source and target states are both `(x, y)`.
- `encode` and `decode` are both the identity map.
-/
def lowering {source : Instance n} (plan : Plan source) :
    Option (Instance.Transform source) :=
  plan.validate.map fun validated =>
    { targetDimension := n
      target := validated.target
      encode := some
      decode := some }

/-- Lowering succeeds exactly when the selected Indicator has a supported
trigger and polarity and every required body-bound endpoint is finite.

`lowering` maps target construction over `plan.validate`, so it returns a
transform precisely when validation succeeds. -/
@[simp]
theorem lowering_isSome_iff_valid {source : Instance n} (plan : Plan source) :
    (lowering plan).isSome ↔ plan.Valid := by
  simp [lowering, Plan.validate]

/-- Lowering fails exactly when the plan does not satisfy its validation
conditions.

Validation is the only partial step; target construction and both identity
state maps are total once a validated plan is available. -/
@[simp]
theorem lowering_eq_none_iff_not_valid {source : Instance n}
    (plan : Plan source) :
    lowering plan = none ↔ ¬plan.Valid := by
  simp [lowering, Plan.validate]

/-- Decoding a feasible target state yields a feasible source state.

The generated Big-M rows have exactly the removed Indicator's denotation under
the unchanged source domains, and decoding is the identity. -/
theorem lowering_isReduction {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.IsReduction := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨validated, _, rfl⟩
  intro targetState htarget
  exact ⟨targetState, rfl,
    (validated.target_feasible_iff_source_feasible targetState).mp htarget⟩

/-- Encoding a feasible source state yields a feasible target state.

The target replaces the selected Indicator with equivalent Big-M rows, and
encoding is the identity. -/
theorem lowering_isRelaxation {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.IsRelaxation := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨validated, _, rfl⟩
  intro sourceState hsource
  exact ⟨sourceState, rfl,
    (validated.target_feasible_iff_source_feasible sourceState).mpr hsource⟩

/-- Lowering preserves whether the objective is minimized or maximized.

The target copies the optimization sense directly from the source Instance. -/
theorem lowering_sensePreserving {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.SensePreserving := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  rfl

/-- Encoding preserves the objective value of every feasible source state.

The target copies the source objective and encoding is the identity. -/
theorem lowering_sourceObjectiveValuePreserving {source : Instance n}
    (plan : Plan source) {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.SourceObjectiveValuePreserving := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  intro sourceState _
  rfl

/-- Decoding preserves the objective value of every feasible target state.

The target copies the source objective and decoding is the identity. -/
theorem lowering_targetObjectiveValuePreserving {source : Instance n}
    (plan : Plan source) {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.TargetObjectiveValuePreserving := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  intro targetState _
  rfl

/-- Encoding preserves both the optimization sense and objective value.

This is the conjunction of `lowering_sensePreserving` and
`lowering_sourceObjectiveValuePreserving`. -/
theorem lowering_sourceObjectivePreserving {source : Instance n}
    (plan : Plan source) {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.SourceObjectivePreserving :=
  ⟨lowering_sensePreserving plan hlowering,
    lowering_sourceObjectiveValuePreserving plan hlowering⟩

/-- Decoding preserves both the optimization sense and objective value.

This is the conjunction of `lowering_sensePreserving` and
`lowering_targetObjectiveValuePreserving`. -/
theorem lowering_targetObjectivePreserving {source : Instance n}
    (plan : Plan source) {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.TargetObjectivePreserving :=
  ⟨lowering_sensePreserving plan hlowering,
    lowering_targetObjectiveValuePreserving plan hlowering⟩

/-- Encoding and then decoding recovers every feasible source state.

Both state maps are the identity on the unchanged state space. -/
theorem lowering_sourceRoundTrip {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.SourceRoundTrip := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  intro sourceState _
  rfl

/-- Decoding and then encoding recovers every feasible target state.

Both state maps are the identity on the unchanged state space. -/
theorem lowering_targetRoundTrip {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.TargetRoundTrip := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  intro targetState _
  rfl

end IndicatorBigM

end Instance

end OMMXProof
