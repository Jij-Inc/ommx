import OMMXProof.Instance
import OMMXProof.Constraint.SOS1

/-!
# Instance-connected SOS1 selector compression

This layer checks selector isolation against the complete finite Instance AST.
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

/-! The generic theorem above makes selector isolation unrepresentable by its
types. The following connected variant starts from a finite `Instance` base,
checks an explicit isolation witness, and then derives the same projection
contract. `encode` records how member/selector tuples populate that finite
model; `encodingIsolation` states that changing selectors changes only the
coordinates claimed private by the executable witness. -/

def zeroSelectors (_ : ι) : Rat := 0

def coreSelectorSourceProblem [Fintype ι] [DecidableEq ι]
    (inst : Instance n)
    (encode : ((ι → Rat) × (ι → Rat)) → State n)
    (bounds : SelectorBounds ι) :
    SemanticProblem ((ι → Rat) × (ι → Rat)) where
  feasible pair := inst.Feasible (encode pair) ∧
    SelectorGadget bounds pair.1 pair.2
  objective pair := inst.ObjectiveValue (encode pair)
  sense := inst.sense

def coreSOS1TargetProblem [Fintype ι] [DecidableEq ι]
    (inst : Instance n)
    (encode : ((ι → Rat) × (ι → Rat)) → State n) :
    SemanticProblem (ι → Rat) where
  feasible members :=
    inst.Feasible (encode (members, zeroSelectors)) ∧ GenericSOS1 members
  objective members := inst.ObjectiveValue (encode (members, zeroSelectors))
  sense := inst.sense

/-- The encoding may vary only the coordinates named by the isolation witness
when its private selector tuple changes. -/
def EncodingRespectsIsolation {ι : Type*}
    (encode : ((ι → Rat) × (ι → Rat)) → State n)
    (witness : Instance.SelectorIsolationWitness n) : Prop :=
  ∀ members selectors selectors',
    AgreeOutside witness.privateSelectors
      (encode (members, selectors)) (encode (members, selectors'))

/-- Exact all-fresh/full-link SOS1 compression from a checked finite base
model. This theorem retains the minimal formulation with freshly introduced
private selectors and the complete two-sided link gadget
`Lᵢ zᵢ ≤ xᵢ ≤ Uᵢ zᵢ`; `corePlannedSelectorCompression` below covers the SDK's
mixed reuse and omitted-link plan. -/
def coreSelectorCompression [Fintype ι] [DecidableEq ι]
    (inst : Instance n)
    (encode : ((ι → Rat) × (ι → Rat)) → State n)
    (bounds : SelectorBounds ι)
    (isolation : Instance.SelectorIsolationWitness n)
    (isolationAccepted : inst.checkSelectorIsolation isolation = true)
    (encodingIsolation : EncodingRespectsIsolation encode isolation)
    (baseBounds : ∀ {members},
      inst.Feasible (encode (members, zeroSelectors)) →
        WithinSelectorBounds bounds members) :
    ProjectionPreserves
      (coreSelectorSourceProblem inst encode bounds)
      (coreSOS1TargetProblem inst encode) where
  project := Prod.fst
  lift members := (members, canonicalSelector members)
  project_feasible {x} h := by
    constructor
    · apply (Instance.checkSelectorIsolation_sound isolationAccepted
        (encodingIsolation x.1 x.2 zeroSelectors)).mp
      exact h.1
    · exact selectorGadget_project_sos1 bounds _ _ h.2
  lift_feasible {y} h := by
    constructor
    · apply (Instance.checkSelectorIsolation_sound isolationAccepted
        (encodingIsolation y zeroSelectors (canonicalSelector y))).mp
      exact h.1
    · exact canonicalSelector_gadget bounds y (baseBounds h.1) h.2
  project_lift _ := rfl
  objective_project {x} h := by
    exact (Instance.checkSelectorIsolation_objective_sound isolationAccepted
      (encodingIsolation x.1 x.2 zeroSelectors)).symm
  objective_lift {y} h := by
    exact Instance.checkSelectorIsolation_objective_sound isolationAccepted
      (encodingIsolation y (canonicalSelector y) zeroSelectors)
  sense_eq := rfl

def corePlannedSelectorSourceProblem [Fintype ι] [DecidableEq ι]
    (inst : Instance n)
    (encode : ((ι → Rat) × (ι → Rat)) → State n)
    (reused : Finset ι) (bounds : SelectorBounds ι) :
    SemanticProblem ((ι → Rat) × (ι → Rat)) where
  feasible pair :=
    inst.Feasible (encode pair) ∧
      PlannedSelectorGadget reused bounds pair.1 pair.2
  objective pair := inst.ObjectiveValue (encode pair)
  sense := inst.sense

/-- Connected correctness theorem for the SDK SOS1 algorithm.  Reused members
remain observable model variables; only the fresh-selector tuple is allowed to
vary inside the checked private coordinate set. -/
def corePlannedSelectorCompression [Fintype ι] [DecidableEq ι]
    (inst : Instance n)
    (encode : ((ι → Rat) × (ι → Rat)) → State n)
    (reused : Finset ι) (bounds : SelectorBounds ι)
    (isolation : Instance.SelectorIsolationWitness n)
    (isolationAccepted : inst.checkSelectorIsolation isolation = true)
    (encodingIsolation : EncodingRespectsIsolation encode isolation)
    (validation : PlannedSelectorValidation reused bounds
      (fun members => inst.Feasible (encode (members, zeroSelectors)))) :
    ProjectionPreserves
      (corePlannedSelectorSourceProblem inst encode reused bounds)
      (coreSOS1TargetProblem inst encode) where
  project := Prod.fst
  lift members := (members, canonicalSelector members)
  project_feasible {x} h := by
    have hbase : inst.Feasible (encode (x.1, zeroSelectors)) := by
      apply (Instance.checkSelectorIsolation_sound isolationAccepted
        (encodingIsolation x.1 x.2 zeroSelectors)).mp
      exact h.1
    exact ⟨hbase,
      plannedSelectorGadget_project_sos1 reused bounds x.1 x.2
        (validation.baseBounds hbase) h.2⟩
  lift_feasible {y} h := by
    constructor
    · apply (Instance.checkSelectorIsolation_sound isolationAccepted
        (encodingIsolation y zeroSelectors (canonicalSelector y))).mp
      exact h.1
    · exact canonicalSelector_plannedGadget reused bounds y
        (validation.baseBounds h.1) (validation.baseReusedBinary h.1) h.2
  project_lift _ := rfl
  objective_project {x} h := by
    exact (Instance.checkSelectorIsolation_objective_sound isolationAccepted
      (encodingIsolation x.1 x.2 zeroSelectors)).symm
  objective_lift {y} h := by
    exact Instance.checkSelectorIsolation_objective_sound isolationAccepted
      (encodingIsolation y (canonicalSelector y) zeroSelectors)
  sense_eq := rfl

/-- A feasible source gadget may contain a noncanonical private selector, so a
source-side retraction law is false in general. -/
theorem canonicalSelector_not_source_retraction :
    let bounds : SelectorBounds (Fin 1) := ⟨fun _ => -1, fun _ => 1⟩
    let members : Fin 1 → Rat := fun _ => 0
    let selectors : Fin 1 → Rat := fun _ => 1
    SelectorGadget bounds members selectors ∧
      (members, canonicalSelector members) ≠ (members, selectors) := by
  dsimp
  constructor
  · constructor
    · intro i
      simp [Membership.mem, Domain.Holds]
    constructor
    · intro i
      norm_num
    · norm_num [Fin.sum_univ_succ]
  · intro h
    have hselector := congrArg (fun pair => pair.2 0) h
    norm_num [canonicalSelector] at hselector

end OMMXProof
