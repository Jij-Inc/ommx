import OMMXProof.Instance.Transform.SOS1SelectorFormulation

/-!
# Compatibility exports for the SOS1 selector formulation

The direction-independent selector and link semantics are owned by
`OMMXProof.Instance.Transform.SOS1SelectorFormulation`. This module preserves
the previous `Instance.SOS1BigM` names for existing lowering code and imports.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

export SOS1SelectorFormulation (
  GenericBinaryOn
  GenericSOS1
  OptionalLowerLink
  OptionalUpperLink
  PlannedSelectorFormulation
  SelectorBounds
  WithinSelectorBounds
  canonicalSelector
  canonicalSelector_binary
  canonicalSelector_plannedFormulation
  canonicalSelector_support
  generic_binary_sum_eq_support_card
  genericSupport
  member_eq_zero_of_fresh_selector_eq_zero
  plannedSelector
  plannedSelector_canonical
  plannedSelectorFormulation_project_sos1
)

end SOS1BigM

end Instance

end OMMXProof
