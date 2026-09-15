import OMMXProof.Instance.Transform.Basic
import OMMXProof.Instance.Transform.SOS1BigM.Lowering.Target
import Mathlib.Tactic

/-!
# SOS1 Big-M lowering implementation

This module writes the SDK SOS1 conversion as an actual `Instance.Transform`,
which consists of a target instance and encode/decode maps between the source
and target state spaces.

## Terminology

- An **SOS1 member** is a source decision-variable component named by the
  selected SOS1 constraint.
- A **selector** is the binary value associated with an SOS1 member in the
  at-most-one formulation. A selector value of zero forces its member to zero;
  a value of one only permits the member to be nonzero.
- A **reused selector** is a binary SOS1 member used directly as its own
  selector.
- A **fresh selector** is a new binary variable appended to the target instance
  for a non-binary SOS1 member.
- The **canonical selector** is the deterministic selector used by `encode`: it
  is zero exactly when its member is zero.
- A **link constraint** is a Big-M inequality connecting a non-binary member
  `vᵢ` to its fresh selector `sᵢ`. For finite bounds `lᵢ ≤ vᵢ ≤ uᵢ`, the
  generated sides are `vᵢ ≤ uᵢ sᵢ` when `0 < uᵢ` and
  `lᵢ sᵢ ≤ vᵢ` when `lᵢ < 0`.
- The **cardinality constraint** is the at-most-one inequality over all reused
  and fresh selectors.
- A **plan** selects the occurrence of the source SOS1 constraint to lower. A
  **validated plan** additionally proves that every member has finite bounds,
  so all required Big-M coefficients can be constructed.

-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

/--
# SOS1 Big-M lowering

## Lowering rule

- Binary members are reused as their own selectors,
- One fresh binary component is appended for every other member.
- `lowering` is partial: it returns `none` exactly when some selected member
  does not have finite bounds.

## Example

For example, `x ∈ Binary, y ∈ [0, 2], SOS1(x, y)` is lowered to
`x, z ∈ Binary, y ∈ [0, 2], x + z ≤ 1, y ≤ 2z`.

- `x ∈ Binary` is a binary source member, so `x` is reused as its own selector
  and no additional selector is appended for it.
- `y ∈ [0, 2]` is a bounded non-binary source member, so its domain is retained
  and the fresh binary selector `z` is appended.
- `SOS1(x, y)` is the selected source constraint removed by this lowering.
- `x, z ∈ Binary` states the domains of the reused selector `x` and fresh
  selector `z` in the target instance.
- `x + z ≤ 1` is the cardinality constraint over those two selectors.
- `y ≤ 2z` is the upper link constraint obtained from the upper bound `2` of
  `y`. No lower link is needed because the lower bound of `y` is zero.
- `(x, y)` is the source state and `(x, y, z)` is the target state.
- `encode` chooses the canonical fresh selector,
  `z = if y = 0 then 0 else 1`.
- `decode` discards `z`: `(x, y, z) ↦ (x, y)`.

When `x = y = 0`, both `z = 0` and `z = 1` are feasible in the target instance.
Thus `decode` is not injective, and the yielded `Instance.Transform` is not
`TargetRoundTrip`.
-/
def lowering {source : Instance n} (plan : Plan source) :
    Option (Instance.Transform source) :=
  plan.validate.map fun validated =>
    { targetDimension := n + plan.freshCount
      target := validated.target
      encode := fun state =>
        some (State.append state fun j =>
          canonicalSelector (plan.memberState state) (plan.freshMember j))
      decode := fun state => some (State.source state) }

/-- Lowering succeeds exactly when every selected member has finite bounds.

`lowering` maps target construction over `plan.validate`, so it returns a
transform precisely when validation succeeds. -/
@[simp]
theorem lowering_isSome_iff_valid {source : Instance n} (plan : Plan source) :
    (lowering plan).isSome ↔ plan.Valid := by
  simp [lowering, Plan.validate]

/-- Lowering fails exactly when some selected member lacks finite bounds.

Validation is the only partial step in `lowering`; target construction and the
state maps are total once a validated plan is available. -/
@[simp]
theorem lowering_eq_none_iff_not_valid {source : Instance n}
    (plan : Plan source) :
    lowering plan = none ↔ ¬plan.Valid := by
  simp [lowering, Plan.validate]

/-- Decoding a feasible target state yields a feasible source state.

Target feasibility supplies the unchanged source constraints and the selector
formulation. Projecting that formulation recovers the lowered SOS1 constraint,
while `State.source` discards the fresh selectors. -/
theorem lowering_isReduction {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.IsReduction := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨validated, _, rfl⟩
  intro targetState htarget
  change validated.target.Feasible targetState at htarget
  refine ⟨State.source targetState, rfl, ?_⟩
  rcases
      (validated.target_feasible_iff_base_and_formulation targetState).mp
        htarget with
    ⟨hbase, hformulation⟩
  apply (plan.source_feasible_iff_base_and_selected
    (State.source targetState)).mpr
  exact ⟨hbase,
    validated.selectedHolds_of_plannedSelectorFormulation
      hbase.1 hformulation⟩

/-- Encoding a feasible source state yields a feasible target state.

The canonical selectors are binary, satisfy the Big-M links by the source
bounds, and satisfy the cardinality row because the selected source constraint
is SOS1. -/
theorem lowering_isRelaxation {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.IsRelaxation := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨validated, _, rfl⟩
  intro sourceState hsource
  let selectors : State plan.freshCount :=
    fun j =>
      canonicalSelector (plan.memberState sourceState) (plan.freshMember j)
  refine ⟨State.append sourceState selectors, rfl, ?_⟩
  change validated.target.Feasible (State.append sourceState selectors)
  apply (validated.target_feasible_append_iff_base_and_formulation
    sourceState selectors).mpr
  rcases (plan.source_feasible_iff_base_and_selected sourceState).mp
      hsource with
    ⟨hbase, hselected⟩
  refine ⟨hbase, ?_⟩
  have hselectors :
      plan.freshSelectorState sourceState selectors =
        canonicalSelector (plan.memberState sourceState) := by
    exact plan.selectorLayout.freshSelectorState_canonical sourceState
  rw [hselectors]
  exact canonicalSelector_plannedFormulation
    plan.reusedMembers validated.bounds (plan.memberState sourceState)
    (validated.withinBounds_of_domains hbase.1)
    (plan.reusedBinary_of_domains hbase.1)
    ((plan.genericSOS1_memberState_iff_holds sourceState).mpr hselected)

/-- Lowering preserves whether the objective is minimized or maximized.

The target Instance copies the optimization sense directly from the source
Instance. -/
theorem lowering_sensePreserving {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.SensePreserving := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  rfl

/-- Encoding preserves the objective value of every feasible source state.

Encoding only appends selector components, and the target objective extends the
source objective with zero coefficients on those new components. -/
theorem lowering_sourceObjectiveValuePreserving {source : Instance n}
    (plan : Plan source) {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.SourceObjectiveValuePreserving := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  intro sourceState _
  simp [Instance.ObjectiveValue, Plan.Validated.target]

/-- Decoding preserves the objective value of every feasible target state.

The target objective ignores the fresh selector components, while decoding
returns exactly the source block on which the original objective is evaluated. -/
theorem lowering_targetObjectiveValuePreserving {source : Instance n}
    (plan : Plan source) {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.TargetObjectiveValuePreserving := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  intro targetState _
  simp [Instance.ObjectiveValue, Plan.Validated.target]

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

Encoding appends fresh selectors and decoding takes the source block, so
`State.source (State.append state selectors) = state`. -/
theorem lowering_sourceRoundTrip {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.SourceRoundTrip := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  intro sourceState _
  simp

end SOS1BigM

end Instance

end OMMXProof
