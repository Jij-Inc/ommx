import OMMXProof.State

/-!
# Semantic optimization problems and preservation contracts

`SemanticProblem` retains only feasibility, objective value, and optimization
sense. The accompanying contracts distinguish identity-space equivalence,
directed implication, projection/lift equivalence, and infeasibility.
-/

namespace OMMXProof

inductive OptimizationSense where
  | minimize
  | maximize
  deriving DecidableEq, Repr

structure SemanticProblem (α : Type*) where
  feasible : α → Prop
  objective : α → Rat
  sense : OptimizationSense

/-- Exact equivalence in one state space. -/
structure IdentityPreserves {α : Type*} (source target : SemanticProblem α) : Prop where
  feasible_iff : ∀ state, source.feasible state ↔ target.feasible state
  objective_eq : ∀ state, source.objective state = target.objective state
  sense_eq : source.sense = target.sense

namespace IdentityPreserves

theorem refl (problem : SemanticProblem α) : IdentityPreserves problem problem := by
  constructor <;> simp

theorem trans {source middle target : SemanticProblem α}
    (first : IdentityPreserves source middle)
    (second : IdentityPreserves middle target) :
    IdentityPreserves source target := by
  constructor
  · intro state
    exact (first.feasible_iff state).trans (second.feasible_iff state)
  · intro state
    exact (first.objective_eq state).trans (second.objective_eq state)
  · exact first.sense_eq.trans second.sense_eq

end IdentityPreserves

/-- Directed feasible-set implication, used for augmentation and relaxation
dominance statements that are intentionally not equivalences. -/
def FeasibleImplies {α : Type*} (source target : SemanticProblem α) : Prop :=
  ∀ {state}, source.feasible state → target.feasible state

/-- Infeasibility is a proposition about one problem, not a preservation mode. -/
def Infeasible {α : Type*} (problem : SemanticProblem α) : Prop :=
  ¬ ∃ state, problem.feasible state

/-- `source` is an extended problem and `target` its reduced projection.

Only a section law on feasible target states is required. Requiring
`lift (project x) = x` would incorrectly reject valid compression with
noncanonical private auxiliaries. -/
structure ProjectionPreserves {α β : Type*}
    (source : SemanticProblem α) (target : SemanticProblem β) where
  project : α → β
  lift : β → α
  project_feasible : ∀ {x}, source.feasible x → target.feasible (project x)
  lift_feasible : ∀ {y}, target.feasible y → source.feasible (lift y)
  project_lift : ∀ {y}, target.feasible y → project (lift y) = y
  objective_project :
    ∀ {x}, source.feasible x → target.objective (project x) = source.objective x
  objective_lift :
    ∀ {y}, target.feasible y → source.objective (lift y) = target.objective y
  sense_eq : source.sense = target.sense

namespace ProjectionPreserves

/-- Reduction steps compose projections forward and lifts in reverse order. -/
def comp {source : SemanticProblem α} {middle : SemanticProblem β} {target : SemanticProblem γ}
    (first : ProjectionPreserves source middle)
    (second : ProjectionPreserves middle target) :
    ProjectionPreserves source target where
  project := second.project ∘ first.project
  lift := first.lift ∘ second.lift
  project_feasible hx := second.project_feasible (first.project_feasible hx)
  lift_feasible hz := first.lift_feasible (second.lift_feasible hz)
  project_lift hz := by
    change second.project (first.project (first.lift (second.lift _))) = _
    rw [first.project_lift (second.lift_feasible hz)]
    exact second.project_lift hz
  objective_project hx := by
    change target.objective (second.project (first.project _)) = source.objective _
    rw [second.objective_project (first.project_feasible hx), first.objective_project hx]
  objective_lift hz := by
    change source.objective (first.lift (second.lift _)) = target.objective _
    rw [first.objective_lift (second.lift_feasible hz), second.objective_lift hz]
  sense_eq := first.sense_eq.trans second.sense_eq

def ofIdentity {source target : SemanticProblem α}
    (preserves : IdentityPreserves source target) :
    ProjectionPreserves source target where
  project := id
  lift := id
  project_feasible h := (preserves.feasible_iff _).mp h
  lift_feasible h := (preserves.feasible_iff _).mpr h
  project_lift _ := rfl
  objective_project _ := (preserves.objective_eq _).symm
  objective_lift _ := preserves.objective_eq _
  sense_eq := preserves.sense_eq

theorem feasible_nonempty_iff {source : SemanticProblem α} {target : SemanticProblem β}
    (preserves : ProjectionPreserves source target) :
    (∃ x, source.feasible x) ↔ ∃ y, target.feasible y := by
  constructor
  · rintro ⟨x, hx⟩
    exact ⟨preserves.project x, preserves.project_feasible hx⟩
  · rintro ⟨y, hy⟩
    exact ⟨preserves.lift y, preserves.lift_feasible hy⟩

def objectiveRange (problem : SemanticProblem α) : Set Rat :=
  {value | ∃ state, problem.feasible state ∧
    problem.objective state = value}

theorem objectiveRange_eq {source : SemanticProblem α} {target : SemanticProblem β}
    (preserves : ProjectionPreserves source target) :
    objectiveRange source = objectiveRange target := by
  ext value
  constructor
  · rintro ⟨x, hx, rfl⟩
    refine ⟨preserves.project x, preserves.project_feasible hx, ?_⟩
    exact preserves.objective_project hx
  · rintro ⟨y, hy, rfl⟩
    refine ⟨preserves.lift y, preserves.lift_feasible hy, ?_⟩
    exact preserves.objective_lift hy

end ProjectionPreserves

/-- Add one semantic constraint without removing any representation. -/
def augment (problem : SemanticProblem α) (constraint : α → Prop) : SemanticProblem α where
  feasible state := problem.feasible state ∧ constraint state
  objective := problem.objective
  sense := problem.sense

theorem augment_preserves (problem : SemanticProblem α) (constraint : α → Prop)
    (implied : ∀ {state}, problem.feasible state → constraint state) :
    IdentityPreserves problem (augment problem constraint) := by
  constructor
  · intro state
    exact ⟨fun h => ⟨h, implied h⟩, And.left⟩
  · simp [augment]
  · rfl

/-- A common state space with a surviving base and one replaceable
constraint. The base is the only context available to a replacement proof. -/
def replaceProblem (base oldConstraint : α → Prop)
    (objective : α → Rat) (sense : OptimizationSense) : SemanticProblem α where
  feasible state := base state ∧ oldConstraint state
  objective := objective
  sense := sense

theorem replace_preserves
    (base oldConstraint newConstraint : α → Prop)
    (objective : α → Rat) (sense : OptimizationSense)
    (equivalent : ∀ {state},
      base state → (oldConstraint state ↔ newConstraint state)) :
    IdentityPreserves
      (replaceProblem base oldConstraint objective sense)
      (replaceProblem base newConstraint objective sense) := by
  constructor
  · intro state
    simp only [replaceProblem]
    exact and_congr_right fun hbase => equivalent hbase
  · simp [replaceProblem]
  · rfl

end OMMXProof
