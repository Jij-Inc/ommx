import OMMXProof.Instance
import OMMXProof.Constraint.SOS1

/-!
# Instance selector-isolation checking

This layer checks whether claimed private selector variables are semantically
unobservable to the complete finite Instance AST.
-/

namespace OMMXProof

namespace Instance

/-- Exact semantic independence of one coordinate in the independent model AST. -/
def IndependentAt (inst : Instance n) (index : Fin n) : Prop :=
  (inst.domains index).Unrestricted ∧
    (∀ constraint ∈ inst.constraints,
      constraint.IndependentAt index) ∧
    (∀ constraint ∈ inst.oneHotConstraints,
      constraint.IndependentAt index) ∧
    (∀ constraint ∈ inst.sos1Constraints,
      constraint.IndependentAt index) ∧
    (∀ constraint ∈ inst.indicatorConstraints,
      constraint.IndependentAt index) ∧
    inst.objective.IndependentAt index

instance (inst : Instance n) (index : Fin n) :
    Decidable (inst.IndependentAt index) := by
  unfold IndependentAt
  infer_instance

/-- Executable semantic-use set. A selector is fresh for the base model exactly
when it is absent from this set. -/
def usedVariables (inst : Instance n) : Finset (Fin n) :=
  Finset.univ.filter fun index => ¬inst.IndependentAt index

structure SelectorIsolationWitness (n : Nat) where
  privateSelectors : Finset (Fin n)

def SelectorIsolated (inst : Instance n)
    (witness : SelectorIsolationWitness n) : Prop :=
  witness.privateSelectors.Nonempty ∧
    Disjoint witness.privateSelectors inst.usedVariables

instance (inst : Instance n) (witness : SelectorIsolationWitness n) :
    Decidable (inst.SelectorIsolated witness) := by
  unfold SelectorIsolated
  infer_instance

/-- Check an untrusted set of claimed all-fresh selector coordinates. -/
def checkSelectorIsolation (inst : Instance n)
    (witness : SelectorIsolationWitness n) : Bool :=
  decide (inst.SelectorIsolated witness)

theorem independentAt_of_selectorIsolated {inst : Instance n}
    {witness : SelectorIsolationWitness n}
    (hisolated : inst.SelectorIsolated witness)
    {index : Fin n} (hprivate : index ∈ witness.privateSelectors) :
    inst.IndependentAt index := by
  by_contra hdependent
  have hused : index ∈ inst.usedVariables := by
    simp [usedVariables, hdependent]
  exact Finset.disjoint_left.mp hisolated.2 hprivate hused

theorem feasible_iff_of_selectorIsolated {inst : Instance n}
    {witness : SelectorIsolationWitness n}
    (hisolated : inst.SelectorIsolated witness)
    {lhs rhs : State n}
    (hagree : AgreeOutside witness.privateSelectors lhs rhs) :
    inst.Feasible lhs ↔ inst.Feasible rhs := by
  have hindependent (i : Fin n) (hi : i ∈ witness.privateSelectors) :=
    independentAt_of_selectorIsolated hisolated hi
  have hdomains :
      (∀ i, lhs i ∈ inst.domains i) ↔
        ∀ i, rhs i ∈ inst.domains i := by
    constructor
    · intro hleft i
      by_cases hprivate : i ∈ witness.privateSelectors
      · exact Domain.holds_of_unrestricted
          (hindependent i hprivate).1 (rhs i)
      · simpa [← hagree i hprivate] using hleft i
    · intro hright i
      by_cases hprivate : i ∈ witness.privateSelectors
      · exact Domain.holds_of_unrestricted
          (hindependent i hprivate).1 (lhs i)
      · simpa [hagree i hprivate] using hright i
  have hconstraints :
      (∀ constraint ∈ inst.constraints, constraint.Holds lhs) ↔
        ∀ constraint ∈ inst.constraints, constraint.Holds rhs := by
    constructor
    · intro hleft constraint hconstraint
      exact (LinearConstraint.holds_iff_of_independentOf
        (fun i hi => (hindependent i hi).2.1 constraint hconstraint)
        hagree).mp (hleft constraint hconstraint)
    · intro hright constraint hconstraint
      exact (LinearConstraint.holds_iff_of_independentOf
        (fun i hi => (hindependent i hi).2.1 constraint hconstraint)
        hagree).mpr (hright constraint hconstraint)
  have honeHot :
      (∀ constraint ∈ inst.oneHotConstraints, constraint.Holds lhs) ↔
        ∀ constraint ∈ inst.oneHotConstraints, constraint.Holds rhs := by
    constructor
    · intro hleft constraint hconstraint
      exact (OneHotConstraint.holds_iff_of_independentOf
        (fun i hi => (hindependent i hi).2.2.1 constraint hconstraint)
        hagree).mp (hleft constraint hconstraint)
    · intro hright constraint hconstraint
      exact (OneHotConstraint.holds_iff_of_independentOf
        (fun i hi => (hindependent i hi).2.2.1 constraint hconstraint)
        hagree).mpr (hright constraint hconstraint)
  have hsos1 :
      (∀ constraint ∈ inst.sos1Constraints, constraint.Holds lhs) ↔
        ∀ constraint ∈ inst.sos1Constraints, constraint.Holds rhs := by
    constructor
    · intro hleft constraint hconstraint
      exact (SOS1Constraint.holds_iff_of_independentOf
        (fun i hi => (hindependent i hi).2.2.2.1 constraint hconstraint)
        hagree).mp (hleft constraint hconstraint)
    · intro hright constraint hconstraint
      exact (SOS1Constraint.holds_iff_of_independentOf
        (fun i hi => (hindependent i hi).2.2.2.1 constraint hconstraint)
        hagree).mpr (hright constraint hconstraint)
  have hindicator :
      (∀ constraint ∈ inst.indicatorConstraints, constraint.Holds lhs) ↔
        ∀ constraint ∈ inst.indicatorConstraints, constraint.Holds rhs := by
    constructor
    · intro hleft constraint hconstraint
      exact (IndicatorConstraint.holds_iff_of_independentOf
        (fun i hi => (hindependent i hi).2.2.2.2.1 constraint hconstraint)
        hagree).mp (hleft constraint hconstraint)
    · intro hright constraint hconstraint
      exact (IndicatorConstraint.holds_iff_of_independentOf
        (fun i hi => (hindependent i hi).2.2.2.2.1 constraint hconstraint)
        hagree).mpr (hright constraint hconstraint)
  unfold Feasible
  constructor
  · rintro ⟨hleftDomains, hleftConstraints, hleftOneHot, hleftSOS1,
      hleftIndicator⟩
    exact ⟨hdomains.mp hleftDomains, hconstraints.mp hleftConstraints,
      honeHot.mp hleftOneHot, hsos1.mp hleftSOS1,
      hindicator.mp hleftIndicator⟩
  · rintro ⟨hrightDomains, hrightConstraints, hrightOneHot, hrightSOS1,
      hrightIndicator⟩
    exact ⟨hdomains.mpr hrightDomains, hconstraints.mpr hrightConstraints,
      honeHot.mpr hrightOneHot, hsos1.mpr hrightSOS1,
      hindicator.mpr hrightIndicator⟩

theorem objective_eq_of_selectorIsolated {inst : Instance n}
    {witness : SelectorIsolationWitness n}
    (hisolated : inst.SelectorIsolated witness)
    {lhs rhs : State n}
    (hagree : AgreeOutside witness.privateSelectors lhs rhs) :
    inst.ObjectiveValue lhs = inst.ObjectiveValue rhs := by
  apply Affine.eval_eq_of_independentOf
  · intro i hi
    exact (independentAt_of_selectorIsolated hisolated hi).2.2.2.2.2
  · exact hagree

theorem checkSelectorIsolation_sound {inst : Instance n}
    {witness : SelectorIsolationWitness n}
    (hcheck : checkSelectorIsolation inst witness = true)
    {lhs rhs : State n}
    (hagree : AgreeOutside witness.privateSelectors lhs rhs) :
    inst.Feasible lhs ↔ inst.Feasible rhs := by
  apply feasible_iff_of_selectorIsolated
  · simpa [checkSelectorIsolation, decide_eq_true_eq] using hcheck
  · exact hagree

theorem checkSelectorIsolation_objective_sound {inst : Instance n}
    {witness : SelectorIsolationWitness n}
    (hcheck : checkSelectorIsolation inst witness = true)
    {lhs rhs : State n}
    (hagree : AgreeOutside witness.privateSelectors lhs rhs) :
    inst.ObjectiveValue lhs = inst.ObjectiveValue rhs := by
  apply objective_eq_of_selectorIsolated
  · simpa [checkSelectorIsolation, decide_eq_true_eq] using hcheck
  · exact hagree

end Instance

end OMMXProof
