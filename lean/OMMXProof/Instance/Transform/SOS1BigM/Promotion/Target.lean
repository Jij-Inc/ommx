import OMMXProof.Instance.Extend
import OMMXProof.Instance.Transform.SOS1BigM.Promotion.Witness
import Mathlib.Tactic

/-!
# Promoted SOS1 target

This module projects the retained prefix of a flat Big-M Instance to the target
Instance and characterizes target feasibility as the retained feasibility facts
together with the promoted SOS1 constraint.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

namespace Witness

section Source

variable (witness : Witness n)
variable (source : Instance (n + witness.freshCount))

/-- Restrict an affine expression to the retained left block. -/
def prefixAffine (expr : Affine (n + witness.freshCount)) : Affine n where
  coeff := fun i => expr.coeff (Fin.castAdd witness.freshCount i)
  constant := expr.constant

/-- Restrict a regular constraint to the retained left block. -/
def prefixConstraint
    (constraint : LinearConstraint (n + witness.freshCount)) :
    LinearConstraint n where
  expr := witness.prefixAffine constraint.expr
  sense := constraint.sense

/-- An affine expression is independent of every fresh selector component. -/
def FreshIndependent
    (expr : Affine (n + witness.freshCount)) : Prop :=
  ∀ j, expr.coeff (Fin.natAdd n j) = 0

instance (expr : Affine (n + witness.freshCount)) :
    Decidable (witness.FreshIndependent expr) := by
  unfold FreshIndependent
  infer_instance

@[simp]
theorem prefixAffine_extend
    (expr : Affine n) :
    witness.prefixAffine (expr.extend witness.freshCount) = expr := by
  cases expr
  simp [prefixAffine, Affine.extend]

theorem extend_prefixAffine_of_freshIndependent
    {expr : Affine (n + witness.freshCount)}
    (h : witness.FreshIndependent expr) :
    (witness.prefixAffine expr).extend witness.freshCount = expr := by
  cases expr with
  | mk coeff constant =>
      have hcoeff :
          Fin.append
              (fun i => coeff (Fin.castAdd witness.freshCount i))
              (fun _ : Fin witness.freshCount => 0) =
            coeff := by
        funext i
        refine Fin.addCases ?_ ?_ i
        · intro j
          simp
        · intro j
          simpa [FreshIndependent] using (h j).symm
      change
        Affine.mk
            (Fin.append
              (fun i => coeff (Fin.castAdd witness.freshCount i))
              (fun _ : Fin witness.freshCount => 0))
            constant =
          Affine.mk coeff constant
      rw [hcoeff]

@[simp]
theorem prefixConstraint_extend
    (constraint : LinearConstraint n) :
    witness.prefixConstraint (constraint.extend witness.freshCount) =
      constraint := by
  cases constraint
  simp [prefixConstraint, LinearConstraint.extend]

theorem extend_prefixConstraint_of_freshIndependent
    {constraint : LinearConstraint (n + witness.freshCount)}
    (h : witness.FreshIndependent constraint.expr) :
    (witness.prefixConstraint constraint).extend witness.freshCount =
      constraint := by
  cases constraint with
  | mk expr sense =>
      simp only [prefixConstraint, LinearConstraint.extend]
      rw [witness.extend_prefixAffine_of_freshIndependent h]

@[simp]
theorem eval_prefixAffine_append_of_freshIndependent
    {expr : Affine (n + witness.freshCount)}
    (h : witness.FreshIndependent expr)
    (state : State n) (selectors : State witness.freshCount) :
    expr.eval (State.append state selectors) =
      (witness.prefixAffine expr).eval state := by
  simp only [Affine.eval, prefixAffine, State.append, Fin.sum_univ_add,
    Fin.append_left, Fin.append_right]
  have hfresh :
      ∑ j, expr.coeff (Fin.natAdd n j) * selectors j = 0 := by
    apply Finset.sum_eq_zero
    intro j _
    rw [h j, zero_mul]
  rw [hfresh, add_zero]

theorem holds_prefixConstraint_append_of_freshIndependent
    {constraint : LinearConstraint (n + witness.freshCount)}
    (h : witness.FreshIndependent constraint.expr)
    (state : State n) (selectors : State witness.freshCount) :
    constraint.Holds (State.append state selectors) ↔
      (witness.prefixConstraint constraint).Holds state := by
  cases constraint with
  | mk expr sense =>
      cases sense <;>
        simp [LinearConstraint.Holds, prefixConstraint,
          witness.eval_prefixAffine_append_of_freshIndependent h]

/-- Restrict a special-constraint member set to the retained left block. -/
def prefixMembers
    (members : Finset (Fin (n + witness.freshCount))) : Finset (Fin n) :=
  Finset.univ.filter fun i ↦ Fin.castAdd witness.freshCount i ∈ members

/-- A special-constraint member set does not reference fresh selectors. -/
def MembersFreshIndependent
    (members : Finset (Fin (n + witness.freshCount))) : Prop :=
  ∀ j, Fin.natAdd n j ∉ members

instance (members : Finset (Fin (n + witness.freshCount))) :
    Decidable (witness.MembersFreshIndependent members) := by
  unfold MembersFreshIndependent
  infer_instance

@[simp]
theorem mem_prefixMembers
    (members : Finset (Fin (n + witness.freshCount))) (i : Fin n) :
    i ∈ witness.prefixMembers members ↔
      Fin.castAdd witness.freshCount i ∈ members := by
  simp [prefixMembers]

theorem castAdd_prefixMembers_of_freshIndependent
    {members : Finset (Fin (n + witness.freshCount))}
    (h : witness.MembersFreshIndependent members) :
    castAddFinsetFin (witness.prefixMembers members) witness.freshCount = members := by
  ext i
  refine Fin.addCases ?_ ?_ i
  · intro j
    simp [prefixMembers]
  · intro j
    constructor
    · intro hmember
      simp only [castAddFinsetFin, Finset.mem_map] at hmember
      rcases hmember with ⟨i, _, heq⟩
      have hval := congrArg Fin.val heq
      simp at hval
      omega
    · exact fun hmember ↦ (h j hmember).elim

/-- A OneHot constraint does not reference fresh selectors. -/
def OneHotFreshIndependent
    (constraint : OneHotConstraint (n + witness.freshCount)) : Prop :=
  witness.MembersFreshIndependent constraint.members

instance (constraint : OneHotConstraint (n + witness.freshCount)) :
    Decidable (witness.OneHotFreshIndependent constraint) := by
  unfold OneHotFreshIndependent
  infer_instance

/-- Project a fresh-independent OneHot constraint to the retained block. -/
def prefixOneHot?
    (constraint : OneHotConstraint (n + witness.freshCount)) :
    Option (OneHotConstraint n) :=
  if witness.OneHotFreshIndependent constraint then
    some { members := witness.prefixMembers constraint.members }
  else
    none

theorem prefixOneHot?_eq_some
    {constraint : OneHotConstraint (n + witness.freshCount)}
    (h : witness.OneHotFreshIndependent constraint) :
    witness.prefixOneHot? constraint =
      some { members := witness.prefixMembers constraint.members } := by
  simp [prefixOneHot?, h]

theorem castAdd_prefixOneHot_of_freshIndependent
    {constraint : OneHotConstraint (n + witness.freshCount)}
    (h : witness.OneHotFreshIndependent constraint) :
    ({ members := witness.prefixMembers constraint.members } : OneHotConstraint n).castAdd
        witness.freshCount = constraint := by
  cases constraint
  simp only [OneHotConstraint.castAdd, OneHotFreshIndependent] at h ⊢
  rw [witness.castAdd_prefixMembers_of_freshIndependent h]

theorem map_castAdd_filterMap_prefixOneHot
    {constraints : List (OneHotConstraint (n + witness.freshCount))}
    (h : constraints.Forall witness.OneHotFreshIndependent) :
    (constraints.filterMap witness.prefixOneHot?).map
        (fun constraint ↦ constraint.castAdd witness.freshCount) = constraints := by
  induction constraints with
  | nil => rfl
  | cons head tail ih =>
      simp only [List.forall_cons] at h
      simp [witness.prefixOneHot?_eq_some h.1,
        witness.castAdd_prefixOneHot_of_freshIndependent h.1, ih h.2]

/-- An SOS1 constraint does not reference fresh selectors. -/
def SOS1FreshIndependent
    (constraint : SOS1Constraint (n + witness.freshCount)) : Prop :=
  witness.MembersFreshIndependent constraint.members

instance (constraint : SOS1Constraint (n + witness.freshCount)) :
    Decidable (witness.SOS1FreshIndependent constraint) := by
  unfold SOS1FreshIndependent
  infer_instance

/-- Project a fresh-independent SOS1 constraint to the retained block. -/
def prefixSOS1?
    (constraint : SOS1Constraint (n + witness.freshCount)) :
    Option (SOS1Constraint n) :=
  if witness.SOS1FreshIndependent constraint then
    some { members := witness.prefixMembers constraint.members }
  else
    none

theorem prefixSOS1?_eq_some
    {constraint : SOS1Constraint (n + witness.freshCount)}
    (h : witness.SOS1FreshIndependent constraint) :
    witness.prefixSOS1? constraint =
      some { members := witness.prefixMembers constraint.members } := by
  simp [prefixSOS1?, h]

theorem castAdd_prefixSOS1_of_freshIndependent
    {constraint : SOS1Constraint (n + witness.freshCount)}
    (h : witness.SOS1FreshIndependent constraint) :
    ({ members := witness.prefixMembers constraint.members } : SOS1Constraint n).castAdd
        witness.freshCount = constraint := by
  cases constraint
  simp only [SOS1Constraint.castAdd, SOS1FreshIndependent] at h ⊢
  rw [witness.castAdd_prefixMembers_of_freshIndependent h]

theorem map_castAdd_filterMap_prefixSOS1
    {constraints : List (SOS1Constraint (n + witness.freshCount))}
    (h : constraints.Forall witness.SOS1FreshIndependent) :
    (constraints.filterMap witness.prefixSOS1?).map
        (fun constraint ↦ constraint.castAdd witness.freshCount) = constraints := by
  induction constraints with
  | nil => rfl
  | cons head tail ih =>
      simp only [List.forall_cons] at h
      simp [witness.prefixSOS1?_eq_some h.1,
        witness.castAdd_prefixSOS1_of_freshIndependent h.1, ih h.2]

/-- An Indicator constraint has a retained trigger and a fresh-independent body. -/
def IndicatorFreshIndependent
    (constraint : IndicatorConstraint (n + witness.freshCount)) : Prop :=
  constraint.trigger.val < n ∧
    witness.FreshIndependent constraint.body.expr

instance (constraint : IndicatorConstraint (n + witness.freshCount)) :
    Decidable (witness.IndicatorFreshIndependent constraint) := by
  unfold IndicatorFreshIndependent
  infer_instance

/-- Project a fresh-independent Indicator constraint to the retained block. -/
def prefixIndicator?
    (constraint : IndicatorConstraint (n + witness.freshCount)) :
    Option (IndicatorConstraint n) :=
  if h : witness.IndicatorFreshIndependent constraint then
    some
      { trigger := ⟨constraint.trigger.val, h.1⟩
        polarity := constraint.polarity
        body := witness.prefixConstraint constraint.body }
  else
    none

theorem prefixIndicator?_eq_some
    {constraint : IndicatorConstraint (n + witness.freshCount)}
    (h : witness.IndicatorFreshIndependent constraint) :
    witness.prefixIndicator? constraint = some
      { trigger := ⟨constraint.trigger.val, h.1⟩
        polarity := constraint.polarity
        body := witness.prefixConstraint constraint.body } := by
  simp [prefixIndicator?, h]

theorem extend_prefixIndicator_of_freshIndependent
    {constraint : IndicatorConstraint (n + witness.freshCount)}
    (h : witness.IndicatorFreshIndependent constraint) :
    ({ trigger := ⟨constraint.trigger.val, h.1⟩
       polarity := constraint.polarity
       body := witness.prefixConstraint constraint.body } : IndicatorConstraint n).extend
        witness.freshCount = constraint := by
  cases constraint with
  | mk trigger polarity body =>
      simp only [IndicatorConstraint.extend]
      have htrigger :
          Fin.castAdd witness.freshCount ⟨trigger.val, h.1⟩ = trigger := by
        apply Fin.ext
        rfl
      rw [htrigger]
      rw [witness.extend_prefixConstraint_of_freshIndependent h.2]

theorem map_extend_filterMap_prefixIndicator
    {constraints : List (IndicatorConstraint (n + witness.freshCount))}
    (h : constraints.Forall witness.IndicatorFreshIndependent) :
    (constraints.filterMap witness.prefixIndicator?).map
        (fun constraint ↦ constraint.extend witness.freshCount) = constraints := by
  induction constraints with
  | nil => rfl
  | cons head tail ih =>
      simp only [List.forall_cons] at h
      simp [witness.prefixIndicator?_eq_some h.1,
        witness.extend_prefixIndicator_of_freshIndependent h.1, ih h.2]

/-- Existing OneHot constraints projected to the retained block. -/
def retainedOneHotConstraints : List (OneHotConstraint n) :=
  source.oneHotConstraints.filterMap witness.prefixOneHot?

/-- Existing SOS1 constraints projected to the retained block. -/
def retainedSOS1Constraints : List (SOS1Constraint n) :=
  source.sos1Constraints.filterMap witness.prefixSOS1?

/-- Existing Indicator constraints projected to the retained block. -/
def retainedIndicatorConstraints : List (IndicatorConstraint n) :=
  source.indicatorConstraints.filterMap witness.prefixIndicator?

/-- The first-class SOS1 constraint created by this promotion. -/
def promotedConstraint : SOS1Constraint n where
  members := witness.members

/-- The promoted target Instance determined unconditionally by the witness.

Invalid witnesses still determine a target.  Correctness properties are proved
only from `StandardBigMForm`. -/
def target : Instance n where
  domains := fun i => source.domains (Fin.castAdd witness.freshCount i)
  constraints :=
    (source.constraints.take witness.retainedConstraintCount).map
      witness.prefixConstraint
  oneHotConstraints := witness.retainedOneHotConstraints source
  sos1Constraints :=
    witness.retainedSOS1Constraints source ++ [witness.promotedConstraint]
  indicatorConstraints := witness.retainedIndicatorConstraints source
  objective := witness.prefixAffine source.objective
  sense := source.sense

/-- Feasibility facts retained by promotion before adding the new SOS1 condition. -/
def BaseFeasible (state : State n) : Prop :=
  (∀ i, state i ∈ (witness.target source).domains i) ∧
    (∀ constraint ∈ (witness.target source).constraints,
      constraint.Holds state) ∧
    (∀ constraint ∈ witness.retainedOneHotConstraints source,
      constraint.Holds state) ∧
    (∀ constraint ∈ witness.retainedSOS1Constraints source,
      constraint.Holds state) ∧
    ∀ constraint ∈ witness.retainedIndicatorConstraints source,
      constraint.Holds state

theorem target_feasible_iff_base_and_selected (state : State n) :
    (witness.target source).Feasible state ↔
      witness.BaseFeasible source state ∧
        witness.promotedConstraint.Holds state := by
  simp only [Instance.Feasible, target, BaseFeasible,
    List.forall_mem_append, List.forall_mem_singleton]
  aesop

end Source

end Witness

end SOS1BigM

end Instance

end OMMXProof
