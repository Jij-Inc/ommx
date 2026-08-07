import OMMXProof.Instance.Transform.IndicatorBigM.Basic
import OMMXProof.Instance.Transform.IndicatorBigM.Formulation
import OMMXProof.Instance.Transform.IndicatorBigM.Plan
import OMMXProof.Instance.Transform.IndicatorBigM.Target

/-!
# Indicator Big-M lowering

This module is the entrypoint for the SDK Indicator conversion as an actual
`Instance.Transform`, including the formulation, validated plans, target
construction, encode/decode maps, and lowering correctness.

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
