import OMMXProof.Instance.Transform.SOS1BigM.Promotion.Validation

/-!
# Semantics of validated SOS1 Big-M promotion

This module characterizes source feasibility, canonical selector construction,
and objective values under the sufficient conditions established by
`Witness.Validated`.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

namespace Witness

section Source

variable (witness : Witness n)
variable (source : Instance (n + witness.freshCount))

theorem withinSelectorBounds_of_domains
    (hfinite : witness.FiniteMemberBounds source)
    {state : State n}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i) :
    WithinSelectorBounds (witness.selectorBounds source)
      (witness.memberState state) := by
  intro i
  exact
    ⟨Domain.finite_lower_le (hdomains i)
        (witness.selectorBounds_exact source hfinite i).1,
      Domain.le_finite_upper (hdomains i)
        (witness.selectorBounds_exact source hfinite i).2⟩

theorem reusedBinary_of_domains
    (hlayout : witness.ReusedMembersMatchDomains source)
    {state : State n}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i) :
    GenericBinaryOn witness.reusedMembers (witness.memberState state) := by
  intro i hi
  have hdomain : (witness.target source).domains i = .binary :=
    (hlayout i).mp hi
  simpa [Witness.memberState, SelectorLayout.memberState, hdomain] using
    hdomains i

theorem genericSOS1_memberState_iff_holds (state : State n) :
    GenericSOS1 (witness.memberState state) ↔
      witness.promotedConstraint.Holds state := by
  classical
  rw [GenericSOS1, Finset.card_le_one]
  simp only [genericSupport, Finset.mem_filter, Finset.mem_univ, true_and]
  constructor
  · intro h i hi j hj hine hjne
    have hij :
        (⟨i, hi⟩ : witness.Member) = (⟨j, hj⟩ : witness.Member) := by
      apply h
      · simpa [Witness.memberState, SelectorLayout.memberState] using hine
      · simpa [Witness.memberState, SelectorLayout.memberState] using hjne
    exact congrArg Subtype.val hij
  · intro h i hi j hj
    apply Subtype.ext
    apply h i.val i.property j.val j.property
    · simpa [Witness.memberState, SelectorLayout.memberState] using hi
    · simpa [Witness.memberState, SelectorLayout.memberState] using hj

theorem selectedHolds_of_plannedSelectorFormulation
    (hfinite : witness.FiniteMemberBounds source)
    {state : State n} {selectors : State witness.freshCount}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i)
    (hformulation :
      PlannedSelectorFormulation witness.reusedMembers
        (witness.selectorBounds source)
        (witness.memberState state)
        (witness.freshSelectorState state selectors)) :
    witness.promotedConstraint.Holds state := by
  apply (witness.genericSOS1_memberState_iff_holds state).mp
  exact plannedSelectorFormulation_project_sos1
    witness.reusedMembers (witness.selectorBounds source)
    (witness.memberState state)
    (witness.freshSelectorState state selectors)
    (witness.withinSelectorBounds_of_domains source hfinite hdomains)
    hformulation

theorem canonicalSelectorFormulation_of_selected
    (hfinite : witness.FiniteMemberBounds source)
    (hlayout : witness.ReusedMembersMatchDomains source)
    {state : State n}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i)
    (hselected : witness.promotedConstraint.Holds state) :
    PlannedSelectorFormulation witness.reusedMembers
      (witness.selectorBounds source)
      (witness.memberState state)
      (canonicalSelector (witness.memberState state)) := by
  apply canonicalSelector_plannedFormulation
  · exact witness.withinSelectorBounds_of_domains source hfinite hdomains
  · exact witness.reusedBinary_of_domains source hlayout hdomains
  · exact (witness.genericSOS1_memberState_iff_holds state).mpr hselected

namespace Validated

theorem source_feasible_append_iff_base_and_formulation
    (validated : witness.Validated source)
    (state : State n) (selectors : State witness.freshCount) :
    source.Feasible (State.append state selectors) ↔
      witness.BaseFeasible source state ∧
        PlannedSelectorFormulation witness.reusedMembers
          (witness.selectorBounds source)
          (witness.memberState state)
          (witness.freshSelectorState state selectors) := by
  have hretainedAt (i : Fin n) :
      State.append state selectors (Fin.castAdd witness.freshCount i) =
        state i := by
    simp [State.append]
  have hfreshAt (j : Fin witness.freshCount) :
      State.append state selectors (Fin.natAdd n j) = selectors j := by
    simp [State.append]
  simp only [Instance.Feasible, validated.sourceDomains_eq,
    validated.sourceConstraints_eq,
    validated.sourceOneHotConstraints_eq,
    validated.sourceSOS1Constraints_eq,
    validated.sourceIndicatorConstraints_eq,
    Domain.append, Fin.forall_fin_add, hretainedAt, hfreshAt,
    Fin.append_left, Fin.append_right, List.forall_mem_append,
    List.forall_mem_map, LinearConstraint.holds_extend_append,
    OneHotConstraint.holds_extend_append, SOS1Constraint.holds_extend_append,
    IndicatorConstraint.holds_extend_append, BaseFeasible]
  constructor
  · rintro ⟨⟨hdomains, hselectors⟩,
      ⟨hretained, hgenerated⟩, honeHot, hsos1, hindicator⟩
    have hformulation :=
      (witness.selectorLayout.canonicalRows_hold_iff_plannedSelectorFormulation
        (witness.selectorBounds source) state selectors
        (witness.reusedBinary_of_domains source
          validated.standardBigMForm.reusedMembersMatchDomains hdomains)
        hselectors).mp hgenerated
    exact
      ⟨⟨hdomains, hretained, honeHot, hsos1, hindicator⟩, hformulation⟩
  · rintro
      ⟨⟨hdomains, hretained, honeHot, hsos1, hindicator⟩, hformulation⟩
    have hselectors :=
      witness.selectorLayout.freshSelectors_binary_of_plannedSelectorFormulation
        (witness.selectorBounds source) state selectors hformulation
    have hgenerated :=
      (witness.selectorLayout.canonicalRows_hold_iff_plannedSelectorFormulation
        (witness.selectorBounds source) state selectors
        (witness.reusedBinary_of_domains source
          validated.standardBigMForm.reusedMembersMatchDomains hdomains)
        hselectors).mpr hformulation
    exact
      ⟨⟨hdomains, hselectors⟩, ⟨hretained, hgenerated⟩,
        honeHot, hsos1, hindicator⟩

theorem source_feasible_iff_base_and_formulation
    (validated : witness.Validated source)
    (flatState : State (n + witness.freshCount)) :
    source.Feasible flatState ↔
      witness.BaseFeasible source (State.source flatState) ∧
        PlannedSelectorFormulation witness.reusedMembers
          (witness.selectorBounds source)
          (witness.memberState (State.source flatState))
          (witness.freshSelectorState (State.source flatState)
            (State.fresh flatState)) := by
  simpa only [State.append_source_fresh] using
    source_feasible_append_iff_base_and_formulation
      (witness := witness) (source := source) validated
      (State.source flatState) (State.fresh flatState)

theorem selectedHolds_of_plannedSelectorFormulation
    (validated : witness.Validated source)
    {state : State n} {selectors : State witness.freshCount}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i)
    (hformulation :
      PlannedSelectorFormulation witness.reusedMembers
        (witness.selectorBounds source)
        (witness.memberState state)
        (witness.freshSelectorState state selectors)) :
    witness.promotedConstraint.Holds state :=
  witness.selectedHolds_of_plannedSelectorFormulation source
    validated.standardBigMForm.finiteMemberBounds hdomains hformulation

theorem canonicalSelectorFormulation_of_selected
    (validated : witness.Validated source)
    {state : State n}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i)
    (hselected : witness.promotedConstraint.Holds state) :
    PlannedSelectorFormulation witness.reusedMembers
      (witness.selectorBounds source)
      (witness.memberState state)
      (canonicalSelector (witness.memberState state)) :=
  witness.canonicalSelectorFormulation_of_selected source
    validated.standardBigMForm.finiteMemberBounds
    validated.standardBigMForm.reusedMembersMatchDomains
    hdomains hselected

theorem sourceObjectiveValue_append_eq_target
    (validated : witness.Validated source)
    (state : State n) (selectors : State witness.freshCount) :
    source.ObjectiveValue (State.append state selectors) =
      (witness.target source).ObjectiveValue state := by
  rw [Instance.ObjectiveValue, Instance.ObjectiveValue,
    validated.sourceObjective_eq]
  exact Affine.eval_extend_append _ _ _

end Validated

end Source

end Witness

end SOS1BigM

end Instance

end OMMXProof
