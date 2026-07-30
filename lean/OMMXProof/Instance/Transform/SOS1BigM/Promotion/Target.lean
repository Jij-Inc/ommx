import OMMXProof.Instance.Extend
import OMMXProof.Instance.Transform.SOS1BigM.Formulation
import OMMXProof.Instance.Transform.SOS1BigM.Promotion.Witness
import Mathlib.Tactic

/-!
# Target and sufficient validation for SOS1 Big-M promotion

This module constructs the promoted SOS1 Instance from a flat source Instance
and an untrusted `SOS1BigM.Witness`.  The initial accepted shape is
deliberately conservative:

- retained variables and regular constraints form left/prefix blocks,
- fresh binary selectors and selector-formulation constraints form right/suffix
  blocks,
- every selected member has finite declared bounds,
- the suffix is the standard upper/lower Big-M links followed by one
  cardinality constraint,
- retained affine expressions do not depend on fresh selectors, and
- the flat source contains no pre-existing special constraints.

These are sufficient, not necessary, conditions. Rejection by this initial
checker does not imply that no correct promotion exists through a broader
Big-M recognizer or another SOS1 formulation.
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
  oneHotConstraints := []
  sos1Constraints := [witness.promotedConstraint]
  indicatorConstraints := []
  objective := witness.prefixAffine source.objective
  sense := source.sense

def memberState (state : State n) : witness.Member → Rat :=
  fun i => state i

/-- A virtual selector tuple indexed by every promoted SOS1 member. -/
def freshSelectorState (members : State n)
    (fresh : State witness.freshCount) :
    witness.Member → Rat :=
  fun i =>
    if hi : i ∈ witness.freshMembers then
      fresh (witness.freshIndex i hi)
    else
      canonicalSelector (witness.memberState members) i

/-- Finite declared bounds for every promoted SOS1 member. -/
def FiniteMemberBounds : Prop :=
  ∀ i : witness.Member, ((witness.target source).domains i).bound.IsFinite

instance : Decidable (witness.FiniteMemberBounds source) := by
  unfold FiniteMemberBounds
  infer_instance

/-- Bounds extracted from the retained member domains.

The fallback zero is used only to keep construction total for untrusted
witnesses. `StandardBigMForm.finiteMemberBounds` proves that the fallback is
unreachable in every accepted promotion. -/
def selectorBounds : SelectorBounds witness.Member where
  lower := fun i =>
    if h : ((witness.target source).domains i).bound.IsFinite then
      ((witness.target source).domains i).bound.finiteLower h
    else 0
  upper := fun i =>
    if h : ((witness.target source).domains i).bound.IsFinite then
      ((witness.target source).domains i).bound.finiteUpper h
    else 0

theorem selectorBounds_exact
    (hfinite : witness.FiniteMemberBounds source)
    (i : witness.Member) :
    ((witness.target source).domains i).bound.lower =
        .finite ((witness.selectorBounds source).lower i) ∧
      ((witness.target source).domains i).bound.upper =
        .finite ((witness.selectorBounds source).upper i) := by
  have hi := hfinite i
  simp only [selectorBounds, hi, ↓reduceDIte]
  exact ⟨Bound.lower_eq_finiteLower _ hi,
    Bound.upper_eq_finiteUpper _ hi⟩

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

/-- The witness classifies reused members exactly as retained binary members. -/
def ReusedMembersMatchDomains : Prop :=
  ∀ i : witness.Member,
    i ∈ witness.reusedMembers ↔ (witness.target source).domains i = .binary

instance : Decidable (witness.ReusedMembersMatchDomains source) := by
  unfold ReusedMembersMatchDomains
  infer_instance

theorem reusedBinary_of_domains
    (hlayout : witness.ReusedMembersMatchDomains source)
    {state : State n}
    (hdomains : ∀ i, state i ∈ (witness.target source).domains i) :
    GenericBinaryOn witness.reusedMembers (witness.memberState state) := by
  intro i hi
  have hdomain : (witness.target source).domains i = .binary :=
    (hlayout i).mp hi
  simpa [memberState, hdomain] using hdomains i

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
      · simpa [memberState] using hine
      · simpa [memberState] using hjne
    exact congrArg Subtype.val hij
  · intro h i hi j hj
    apply Subtype.ext
    apply h i.val i.property j.val j.property
    · simpa [memberState] using hi
    · simpa [memberState] using hj

/-! ## The canonical regular selector formulation -/

def upperLink (j : Fin witness.freshCount) :
    LinearConstraint (n + witness.freshCount) where
  expr := Affine.sub
    (Affine.coordinate
      (Fin.castAdd witness.freshCount (witness.freshMember j).val))
    (Affine.scale
      ((witness.selectorBounds source).upper (witness.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
  sense := .lessEqual

def lowerLink (j : Fin witness.freshCount) :
    LinearConstraint (n + witness.freshCount) where
  expr := Affine.sub
    (Affine.scale
      ((witness.selectorBounds source).lower (witness.freshMember j))
      (Affine.coordinate (Fin.natAdd n j)))
    (Affine.coordinate
      (Fin.castAdd witness.freshCount (witness.freshMember j).val))
  sense := .lessEqual

@[simp]
theorem upperLink_holds
    (j : Fin witness.freshCount) (state : State n)
    (selectors : State witness.freshCount) :
    (witness.upperLink source j).Holds (State.append state selectors) ↔
      state (witness.freshMember j).val ≤
        (witness.selectorBounds source).upper (witness.freshMember j) *
          selectors j := by
  simp [upperLink, LinearConstraint.Holds, State.append]

@[simp]
theorem lowerLink_holds
    (j : Fin witness.freshCount) (state : State n)
    (selectors : State witness.freshCount) :
    (witness.lowerLink source j).Holds (State.append state selectors) ↔
      (witness.selectorBounds source).lower (witness.freshMember j) *
          selectors j ≤
        state (witness.freshMember j).val := by
  simp [lowerLink, LinearConstraint.Holds, State.append]

def linksFor (j : Fin witness.freshCount) :
    List (LinearConstraint (n + witness.freshCount)) :=
  (if 0 <
      (witness.selectorBounds source).upper (witness.freshMember j) then
      [witness.upperLink source j]
    else []) ++
    if (witness.selectorBounds source).lower (witness.freshMember j) < 0 then
      [witness.lowerLink source j]
    else []

def linkConstraints :
    List (LinearConstraint (n + witness.freshCount)) :=
  (List.ofFn fun j => witness.linksFor source j).flatten

theorem linkConstraints_hold_iff
    (state : State n) (selectors : State witness.freshCount) :
    (∀ constraint ∈ witness.linkConstraints source,
      constraint.Holds (State.append state selectors)) ↔
      ∀ j,
        OptionalUpperLink
            ((witness.selectorBounds source).upper (witness.freshMember j))
            (state (witness.freshMember j).val) (selectors j) ∧
          OptionalLowerLink
            ((witness.selectorBounds source).lower (witness.freshMember j))
            (state (witness.freshMember j).val) (selectors j) := by
  simp only [linkConstraints, List.forall_mem_flatten,
    List.forall_mem_ofFn_iff]
  constructor
  · intro h j
    have hj := h j
    by_cases hu :
      0 < (witness.selectorBounds source).upper (witness.freshMember j)
    <;> by_cases hl :
      (witness.selectorBounds source).lower (witness.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj
  · intro h j
    have hj := h j
    by_cases hu :
      0 < (witness.selectorBounds source).upper (witness.freshMember j)
    <;> by_cases hl :
      (witness.selectorBounds source).lower (witness.freshMember j) < 0
    <;> simp [linksFor, hu, hl, OptionalUpperLink, OptionalLowerLink] at hj ⊢
    <;> exact hj

/-- Coefficients of the mixed cardinality row. Reused binary members remain in
the retained block, and every fresh selector contributes in the suffix block. -/
def cardinalityExpr : Affine (n + witness.freshCount) where
  coeff := Fin.append
    (fun i =>
      if i ∈ witness.members ∧
          (witness.target source).domains i = .binary then 1 else 0)
    (fun _ => 1)
  constant := -1

def cardinalityConstraint :
    LinearConstraint (n + witness.freshCount) where
  expr := witness.cardinalityExpr source
  sense := .lessEqual

def selectorSum (state : State n)
    (selectors : State witness.freshCount) : Rat :=
  (∑ i ∈ witness.members,
      if (witness.target source).domains i = .binary then state i else 0) +
    ∑ j, selectors j

def reusedContribution (state : State n) (i : witness.Member) : Rat :=
  if i ∈ witness.reusedMembers then witness.memberState state i else 0

def freshContribution (state : State n)
    (selectors : State witness.freshCount) (i : witness.Member) : Rat :=
  if i ∈ witness.freshMembers then
    witness.freshSelectorState state selectors i
  else 0

theorem plannedSelector_eq_contributions
    (state : State n) (selectors : State witness.freshCount)
    (i : witness.Member) :
    plannedSelector witness.reusedMembers (witness.memberState state)
        (witness.freshSelectorState state selectors) i =
      witness.reusedContribution state i +
        witness.freshContribution state selectors i := by
  by_cases hr : i ∈ witness.reusedMembers
  · have hf : i ∉ witness.freshMembers := by
      simpa [reusedMembers] using hr
    simp [plannedSelector, reusedContribution, freshContribution, hr, hf]
  · have hf : i ∈ witness.freshMembers := by
      simp [reusedMembers] at hr
      exact hr
    simp [plannedSelector, reusedContribution, freshContribution, hr, hf]

theorem retainedReusedSum_eq
    (hlayout : witness.ReusedMembersMatchDomains source)
    (state : State n) :
    (∑ i ∈ witness.members,
      if (witness.target source).domains i = .binary then state i else 0) =
      ∑ i : witness.Member, witness.reusedContribution state i := by
  symm
  calc
    (∑ i : witness.Member, witness.reusedContribution state i) =
        ∑ i : witness.Member,
          if (witness.target source).domains i = .binary then state i else 0 := by
      apply Finset.sum_congr rfl
      intro i _
      rw [reusedContribution]
      by_cases hr : i ∈ witness.reusedMembers
      · simp only [hr, ↓reduceIte]
        rw [(hlayout i).mp hr]
        rfl
      · simp only [hr, ↓reduceIte]
        have hne : (witness.target source).domains i ≠ .binary := by
          exact fun hbinary => hr ((hlayout i).mpr hbinary)
        simp [hne]
    _ = ∑ i ∈ witness.members,
          if (witness.target source).domains i = .binary then state i else 0 := by
      exact Finset.sum_coe_sort witness.members
        (fun i =>
          if (witness.target source).domains i = .binary then state i else 0)

theorem freshSelectorSum_eq
    (state : State n) (selectors : State witness.freshCount) :
    (∑ j, selectors j) =
      ∑ i : witness.Member, witness.freshContribution state selectors i := by
  calc
    (∑ j, selectors j) =
        ∑ i : {i // i ∈ witness.freshMembers},
          witness.freshSelectorState state selectors i.val := by
      apply Fintype.sum_equiv
        (witness.freshMembers.orderIsoOfFin rfl).toEquiv
      intro j
      unfold freshSelectorState
      have hi :
          ((witness.freshMembers.orderIsoOfFin rfl).toEquiv j).val ∈
            witness.freshMembers :=
        ((witness.freshMembers.orderIsoOfFin rfl).toEquiv j).property
      rw [dif_pos hi]
      change selectors j =
        selectors ((witness.freshMembers.orderIsoOfFin rfl).symm
          ((witness.freshMembers.orderIsoOfFin rfl) j))
      exact congrArg selectors
        ((witness.freshMembers.orderIsoOfFin rfl).symm_apply_apply j).symm
    _ = ∑ i ∈ witness.freshMembers,
          witness.freshSelectorState state selectors i := by
      exact Finset.sum_coe_sort witness.freshMembers
        (witness.freshSelectorState state selectors)
    _ = ∑ i : witness.Member,
          witness.freshContribution state selectors i := by
      symm
      simpa [freshContribution] using
        (Finset.sum_ite_mem_eq witness.freshMembers
          (witness.freshSelectorState state selectors))

theorem selectorSum_eq_plannedSelector
    (hlayout : witness.ReusedMembersMatchDomains source)
    (state : State n) (selectors : State witness.freshCount) :
    witness.selectorSum source state selectors =
      ∑ i, plannedSelector witness.reusedMembers (witness.memberState state)
        (witness.freshSelectorState state selectors) i := by
  rw [selectorSum, witness.retainedReusedSum_eq source hlayout state,
    witness.freshSelectorSum_eq state selectors]
  rw [← Finset.sum_add_distrib]
  apply Finset.sum_congr rfl
  intro i _
  exact (witness.plannedSelector_eq_contributions state selectors i).symm

@[simp]
theorem cardinalityConstraint_holds
    (state : State n) (selectors : State witness.freshCount) :
    (witness.cardinalityConstraint source).Holds
        (State.append state selectors) ↔
      witness.selectorSum source state selectors ≤ 1 := by
  have hsum :
      (∑ i : Fin n,
        if i ∈ witness.members ∧
            (witness.target source).domains i = .binary then state i else 0) =
        ∑ i ∈ witness.members,
          if (witness.target source).domains i = .binary then state i else 0 := by
    convert Finset.sum_ite_mem_eq witness.members
      (fun i =>
        if (witness.target source).domains i = .binary then state i else 0) using 1
    simp [ite_and]
  simp [cardinalityConstraint, cardinalityExpr, selectorSum,
    LinearConstraint.Holds, Affine.eval, State.append, Fin.sum_univ_add]
  rw [hsum]

def generatedConstraints :
    List (LinearConstraint (n + witness.freshCount)) :=
  witness.linkConstraints source ++ [witness.cardinalityConstraint source]

theorem plannedSelector_binary_of_domains
    (hlayout : witness.ReusedMembersMatchDomains source)
    (state : State n) (selectors : State witness.freshCount)
    (hretainedDomains :
      ∀ i, state i ∈ (witness.target source).domains i)
    (hselectorDomains : ∀ j, selectors j ∈ Domain.binary) :
    GenericBinaryOn Finset.univ
      (plannedSelector witness.reusedMembers (witness.memberState state)
        (witness.freshSelectorState state selectors)) := by
  intro i _
  by_cases hr : i ∈ witness.reusedMembers
  · have hdomain : (witness.target source).domains i = .binary :=
      (hlayout i).mp hr
    simpa [plannedSelector, hr, memberState, hdomain] using
      hretainedDomains i
  · have hfresh : i ∈ witness.freshMembers := by
      simp [reusedMembers] at hr
      exact hr
    simpa [plannedSelector, hr, freshSelectorState, hfresh] using
      hselectorDomains (witness.freshIndex i hfresh)

theorem linksForFresh_iff_linksForMembers
    (state : State n) (selectors : State witness.freshCount) :
    (∀ j,
      OptionalUpperLink
          ((witness.selectorBounds source).upper (witness.freshMember j))
          (state (witness.freshMember j).val) (selectors j) ∧
        OptionalLowerLink
          ((witness.selectorBounds source).lower (witness.freshMember j))
          (state (witness.freshMember j).val) (selectors j)) ↔
      ∀ i, i ∉ witness.reusedMembers →
        OptionalUpperLink ((witness.selectorBounds source).upper i)
            (witness.memberState state i)
            (witness.freshSelectorState state selectors i) ∧
          OptionalLowerLink ((witness.selectorBounds source).lower i)
            (witness.memberState state i)
            (witness.freshSelectorState state selectors i) := by
  constructor
  · intro h i hr
    have hfresh : i ∈ witness.freshMembers := by
      simp [reusedMembers] at hr
      exact hr
    have hi := h (witness.freshIndex i hfresh)
    simpa [memberState, freshSelectorState, hfresh] using hi
  · intro h j
    have hfresh : witness.freshMember j ∈ witness.freshMembers :=
      (witness.freshMembers.orderIsoOfFin rfl j).property
    have hr : witness.freshMember j ∉ witness.reusedMembers := by
      simpa [reusedMembers] using hfresh
    have hj := h (witness.freshMember j) hr
    simpa [memberState, freshSelectorState, hfresh] using hj

theorem generatedConstraints_hold_iff_plannedSelectorFormulation
    (hlayout : witness.ReusedMembersMatchDomains source)
    (state : State n) (selectors : State witness.freshCount)
    (hretainedDomains :
      ∀ i, state i ∈ (witness.target source).domains i)
    (hselectorDomains : ∀ j, selectors j ∈ Domain.binary) :
    (∀ constraint ∈ witness.generatedConstraints source,
      constraint.Holds (State.append state selectors)) ↔
      PlannedSelectorFormulation witness.reusedMembers
        (witness.selectorBounds source)
        (witness.memberState state)
        (witness.freshSelectorState state selectors) := by
  rw [generatedConstraints, List.forall_mem_append,
    List.forall_mem_singleton, witness.linkConstraints_hold_iff,
    witness.cardinalityConstraint_holds,
    witness.linksForFresh_iff_linksForMembers]
  constructor
  · rintro ⟨hlinks, hcardinality⟩
    refine ⟨witness.plannedSelector_binary_of_domains source hlayout
      state selectors hretainedDomains hselectorDomains, hlinks, ?_⟩
    rw [← witness.selectorSum_eq_plannedSelector source hlayout state selectors]
    exact hcardinality
  · rintro ⟨_, hlinks, hcardinality⟩
    refine ⟨hlinks, ?_⟩
    rw [witness.selectorSum_eq_plannedSelector source hlayout state selectors]
    exact hcardinality

theorem freshSelectors_binary_of_plannedSelectorFormulation
    (state : State n) (selectors : State witness.freshCount)
    (hformulation :
      PlannedSelectorFormulation witness.reusedMembers
        (witness.selectorBounds source)
        (witness.memberState state)
        (witness.freshSelectorState state selectors)) :
    ∀ j, selectors j ∈ Domain.binary := by
  intro j
  have hfresh : witness.freshMember j ∈ witness.freshMembers :=
    (witness.freshMembers.orderIsoOfFin rfl j).property
  have hr : witness.freshMember j ∉ witness.reusedMembers := by
    simpa [reusedMembers] using hfresh
  have hbinary := hformulation.1 (witness.freshMember j) (by simp)
  simpa [plannedSelector, hr, freshSelectorState, hfresh] using hbinary

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

/-! ## Conservative validation of the flat source shape -/

/-- Pointwise, computable equality for regular constraint rows.

`LinearConstraint` does not have a global `DecidableEq` instance because its
coefficient vector is a function.  Promotion only needs equality over the
finite current dimension, so validation compares the sense, constant, and
every coefficient explicitly. -/
def SameRow (lhs rhs : LinearConstraint m) : Prop :=
  lhs.sense = rhs.sense ∧
    lhs.expr.constant = rhs.expr.constant ∧
    ∀ i, lhs.expr.coeff i = rhs.expr.coeff i

instance (lhs rhs : LinearConstraint m) : Decidable (SameRow lhs rhs) := by
  unfold SameRow
  infer_instance

theorem sameRow_iff_eq {lhs rhs : LinearConstraint m} :
    SameRow lhs rhs ↔ lhs = rhs := by
  constructor
  · rcases lhs with ⟨⟨lhsCoeff, lhsConstant⟩, lhsSense⟩
    rcases rhs with ⟨⟨rhsCoeff, rhsConstant⟩, rhsSense⟩
    rintro ⟨hsense, hconstant, hcoeff⟩
    simp only at hsense hconstant hcoeff
    subst rhsSense
    subst rhsConstant
    have hcoefficients : lhsCoeff = rhsCoeff := funext hcoeff
    subst rhsCoeff
    rfl
  · rintro rfl
    exact ⟨rfl, rfl, fun _ => rfl⟩

/-- A computable row-by-row comparison for two ordered constraint blocks. -/
def RowsMatch (actual expected : List (LinearConstraint m)) : Prop :=
  List.Forall₂ SameRow actual expected

instance (actual expected : List (LinearConstraint m)) :
    Decidable (RowsMatch actual expected) := by
  unfold RowsMatch
  infer_instance

theorem rowsMatch_iff_eq {actual expected : List (LinearConstraint m)} :
    RowsMatch actual expected ↔ actual = expected := by
  constructor
  · intro h
    induction h with
    | nil => rfl
    | cons hrow _ ih =>
        rw [sameRow_iff_eq] at hrow
        simp [hrow, ih]
  · rintro rfl
    induction actual with
    | nil => exact .nil
    | cons head tail ih =>
        exact .cons (sameRow_iff_eq.mpr rfl) ih

/-- Initial sufficient conditions recognized by SOS1 Big-M promotion.

The condition deliberately fixes a prefix/suffix layout.  It is not intended
to characterize every flat formulation equivalent to an SOS1 constraint.
Failure therefore means "unsupported by this validator", not that the proposed
promotion is mathematically invalid. -/
structure StandardBigMForm : Prop where
  finiteMemberBounds : witness.FiniteMemberBounds source
  reusedMembersMatchDomains : witness.ReusedMembersMatchDomains source
  freshSelectorDomains :
    ∀ j, source.domains (Fin.natAdd n j) = .binary
  retainedConstraintCount_le :
    witness.retainedConstraintCount ≤ source.constraints.length
  retainedConstraintsFreshIndependent :
    (source.constraints.take witness.retainedConstraintCount).Forall
      fun constraint => witness.FreshIndependent constraint.expr
  formulationSuffixMatches :
    RowsMatch
      (source.constraints.drop witness.retainedConstraintCount)
      (witness.generatedConstraints source)
  noOneHotConstraints : source.oneHotConstraints.length = 0
  noSOS1Constraints : source.sos1Constraints.length = 0
  noIndicatorConstraints : source.indicatorConstraints.length = 0
  objectiveFreshIndependent :
    witness.FreshIndependent source.objective

private def standardBigMFormConditions : Prop :=
  witness.FiniteMemberBounds source ∧
    witness.ReusedMembersMatchDomains source ∧
    (∀ j, source.domains (Fin.natAdd n j) = .binary) ∧
    witness.retainedConstraintCount ≤ source.constraints.length ∧
    (source.constraints.take witness.retainedConstraintCount).Forall
      (fun constraint => witness.FreshIndependent constraint.expr) ∧
    RowsMatch
      (source.constraints.drop witness.retainedConstraintCount)
      (witness.generatedConstraints source) ∧
    source.oneHotConstraints.length = 0 ∧
    source.sos1Constraints.length = 0 ∧
    source.indicatorConstraints.length = 0 ∧
    witness.FreshIndependent source.objective

private instance : Decidable (witness.standardBigMFormConditions source) := by
  unfold standardBigMFormConditions
  infer_instance

private theorem standardBigMForm_iff_conditions :
    witness.StandardBigMForm source ↔
      witness.standardBigMFormConditions source := by
  constructor
  · intro h
    exact ⟨h.finiteMemberBounds, h.reusedMembersMatchDomains,
      h.freshSelectorDomains, h.retainedConstraintCount_le,
      h.retainedConstraintsFreshIndependent, h.formulationSuffixMatches,
      h.noOneHotConstraints, h.noSOS1Constraints,
      h.noIndicatorConstraints, h.objectiveFreshIndependent⟩
  · rintro ⟨hfinite, hlayout, hfreshDomains, hcount, hretained,
      hsuffix, honeHot, hsos1, hindicator, hobjective⟩
    exact
      { finiteMemberBounds := hfinite
        reusedMembersMatchDomains := hlayout
        freshSelectorDomains := hfreshDomains
        retainedConstraintCount_le := hcount
        retainedConstraintsFreshIndependent := hretained
        formulationSuffixMatches := hsuffix
        noOneHotConstraints := honeHot
        noSOS1Constraints := hsos1
        noIndicatorConstraints := hindicator
        objectiveFreshIndependent := hobjective }

instance : Decidable (witness.StandardBigMForm source) :=
  decidable_of_iff (witness.standardBigMFormConditions source)
    (witness.standardBigMForm_iff_conditions source).symm

/-- A witness bundled with evidence that the current flat source has the
initial supported selector-formulation shape. -/
structure Validated : Type where
  standardBigMForm : witness.StandardBigMForm source

/-- Check the current flat Instance against the conservative supported shape. -/
def validate : Option (witness.Validated source) :=
  if hvalid : witness.StandardBigMForm source then
    some ⟨hvalid⟩
  else
    none

@[simp]
theorem validate_isSome_iff_standardBigMForm :
    (witness.validate source).isSome ↔
      witness.StandardBigMForm source := by
  simp [validate]

@[simp]
theorem validate_eq_none_iff_not_standardBigMForm :
    witness.validate source = none ↔
      ¬witness.StandardBigMForm source := by
  simp [validate]

namespace Validated

theorem extendedPrefixConstraints_eq
    (validated : witness.Validated source) :
    ((source.constraints.take witness.retainedConstraintCount).map
        witness.prefixConstraint).map
          (fun constraint => constraint.extend witness.freshCount) =
      source.constraints.take witness.retainedConstraintCount := by
  rw [List.map_map]
  calc
    List.map
        ((fun constraint => constraint.extend witness.freshCount) ∘
          witness.prefixConstraint)
        (source.constraints.take witness.retainedConstraintCount) =
      List.map id
        (source.constraints.take witness.retainedConstraintCount) := by
      rw [List.map_eq_map_iff]
      intro constraint hconstraint
      apply witness.extend_prefixConstraint_of_freshIndependent
      exact
        (List.forall_iff_forall_mem.mp
          validated.standardBigMForm.retainedConstraintsFreshIndependent)
          constraint hconstraint
    _ = source.constraints.take witness.retainedConstraintCount :=
      List.map_id _

theorem formulationSuffix_eq
    (validated : witness.Validated source) :
    source.constraints.drop witness.retainedConstraintCount =
      witness.generatedConstraints source := by
  exact rowsMatch_iff_eq.mp
    validated.standardBigMForm.formulationSuffixMatches

theorem sourceDomains_eq
    (validated : witness.Validated source) :
    source.domains =
      Domain.append (witness.target source).domains (fun _ => .binary) := by
  funext i
  refine Fin.addCases ?_ ?_ i
  · intro j
    simp [target, Domain.append]
  · intro j
    simp [Domain.append,
      validated.standardBigMForm.freshSelectorDomains j]

theorem sourceConstraints_eq
    (validated : witness.Validated source) :
    source.constraints =
      ((witness.target source).constraints.map
        fun constraint => constraint.extend witness.freshCount) ++
        witness.generatedConstraints source := by
  calc
    source.constraints =
        source.constraints.take witness.retainedConstraintCount ++
          source.constraints.drop witness.retainedConstraintCount :=
      (List.take_append_drop witness.retainedConstraintCount
        source.constraints).symm
    _ =
        source.constraints.take witness.retainedConstraintCount ++
          witness.generatedConstraints source := by
      rw [validated.formulationSuffix_eq]
    _ =
        ((witness.target source).constraints.map
          fun constraint => constraint.extend witness.freshCount) ++
          witness.generatedConstraints source := by
      rw [target, validated.extendedPrefixConstraints_eq]

theorem sourceOneHotConstraints_eq_nil
    (validated : witness.Validated source) :
    source.oneHotConstraints = [] := by
  simpa using validated.standardBigMForm.noOneHotConstraints

theorem sourceSOS1Constraints_eq_nil
    (validated : witness.Validated source) :
    source.sos1Constraints = [] := by
  simpa using validated.standardBigMForm.noSOS1Constraints

theorem sourceIndicatorConstraints_eq_nil
    (validated : witness.Validated source) :
    source.indicatorConstraints = [] := by
  simpa using validated.standardBigMForm.noIndicatorConstraints

theorem sourceObjective_eq
    (validated : witness.Validated source) :
    source.objective =
      (witness.target source).objective.extend witness.freshCount := by
  symm
  simpa [target] using
    witness.extend_prefixAffine_of_freshIndependent
      validated.standardBigMForm.objectiveFreshIndependent

end Validated

/-! ## Feasibility and objective characterizations -/

/-- Feasibility facts retained by promotion before adding the SOS1 condition. -/
def BaseFeasible (state : State n) : Prop :=
  (∀ i, state i ∈ (witness.target source).domains i) ∧
    ∀ constraint ∈ (witness.target source).constraints,
      constraint.Holds state

theorem target_feasible_iff_base_and_selected (state : State n) :
    (witness.target source).Feasible state ↔
      witness.BaseFeasible source state ∧
        witness.promotedConstraint.Holds state := by
  simp only [Instance.Feasible, target, BaseFeasible,
    List.forall_mem_singleton]
  aesop

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
    validated.sourceOneHotConstraints_eq_nil,
    validated.sourceSOS1Constraints_eq_nil,
    validated.sourceIndicatorConstraints_eq_nil,
    Domain.append, Fin.forall_fin_add, hretainedAt, hfreshAt,
    Fin.append_left, Fin.append_right, List.forall_mem_append,
    List.forall_mem_map, LinearConstraint.holds_extend_append, BaseFeasible]
  constructor
  · rintro ⟨⟨hdomains, hselectors⟩,
      ⟨hretained, hgenerated⟩, _, _, _⟩
    have hformulation :=
      (witness.generatedConstraints_hold_iff_plannedSelectorFormulation
        source
        validated.standardBigMForm.reusedMembersMatchDomains
        state selectors hdomains hselectors).mp hgenerated
    exact ⟨⟨hdomains, hretained⟩, hformulation⟩
  · rintro ⟨⟨hdomains, hretained⟩, hformulation⟩
    have hselectors :=
      witness.freshSelectors_binary_of_plannedSelectorFormulation
        source state selectors hformulation
    have hgenerated :=
      (witness.generatedConstraints_hold_iff_plannedSelectorFormulation
        source
        validated.standardBigMForm.reusedMembersMatchDomains
        state selectors hdomains hselectors).mpr hformulation
    refine
      ⟨⟨hdomains, hselectors⟩, ⟨hretained, hgenerated⟩, ?_, ?_, ?_⟩
    all_goals simp

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
