import OMMXProof.Instance

/-!
# Instance transformations

An `Instance.Transform source` records a transformed Instance together with
partial state maps in both directions. It deliberately imposes no semantic
correctness by itself: reduction, relaxation, objective preservation, and
round trips are separate predicates.
-/

namespace OMMXProof

namespace Instance

/-- An unrestricted transformation result for one source Instance.

`encode` follows the transformation from the source state space to the target
state space. `decode` reconstructs a source state from a target state. Both are
partial because transformations may intentionally discard states. -/
structure Transform (source : Instance n) where
  targetDimension : Nat
  target : Instance targetDimension
  encode : State n → Option (State targetDimension)
  decode : State targetDimension → Option (State n)

namespace Transform

/-- Every feasible target state can be decoded to a feasible source state.

This is the feasibility-soundness condition required of a reduction. It rules
out transformed feasible solutions which have no meaning for the source
Instance. -/
def IsReduction {source : Instance n} (transform : Transform source) : Prop :=
  ∀ {targetState},
    transform.target.Feasible targetState →
      ∃ sourceState,
        transform.decode targetState = some sourceState ∧
          source.Feasible sourceState

/-- Every feasible source state can be encoded as a feasible target state.

For identical state spaces and the identity encoding, this is the usual
feasible-region inclusion defining a relaxation. -/
def IsRelaxation {source : Instance n} (transform : Transform source) : Prop :=
  ∀ {sourceState},
    source.Feasible sourceState →
      ∃ targetState,
        transform.encode sourceState = some targetState ∧
          transform.target.Feasible targetState

/-- The optimization sense is unchanged by the transformation. -/
def SensePreserving {source : Instance n}
    (transform : Transform source) : Prop :=
  source.sense = transform.target.sense

/-- Encoding preserves the objective value of every feasible source state.

The `Option.map` equality also requires `encode` to be defined on every
feasible source state. This is the objective condition paired with
`IsRelaxation`. -/
def SourceObjectiveValuePreserving {source : Instance n}
    (transform : Transform source) : Prop :=
  ∀ {sourceState},
    source.Feasible sourceState →
      Option.map transform.target.ObjectiveValue
        (transform.encode sourceState) =
          some (source.ObjectiveValue sourceState)

/-- Decoding preserves the objective value of every feasible target state.

The `Option.map` equality also requires `decode` to be defined on every
feasible target state. This is the objective condition paired with
`IsReduction`. -/
def TargetObjectiveValuePreserving {source : Instance n}
    (transform : Transform source) : Prop :=
  ∀ {targetState},
    transform.target.Feasible targetState →
      Option.map source.ObjectiveValue
        (transform.decode targetState) =
          some (transform.target.ObjectiveValue targetState)

/-- Encoding preserves the full objective: both optimization sense and value
on every feasible source state. -/
def SourceObjectivePreserving {source : Instance n}
    (transform : Transform source) : Prop :=
  transform.SensePreserving ∧
    transform.SourceObjectiveValuePreserving

/-- Decoding preserves the full objective: both optimization sense and value
on every feasible target state. -/
def TargetObjectivePreserving {source : Instance n}
    (transform : Transform source) : Prop :=
  transform.SensePreserving ∧
    transform.TargetObjectiveValuePreserving

/-- Encoding and then decoding recovers every feasible source state.

The `Option.bind` equality also requires both partial maps to be defined along
the round trip. -/
def SourceRoundTrip {source : Instance n} (transform : Transform source) : Prop :=
  ∀ {sourceState},
    source.Feasible sourceState →
      transform.encode sourceState >>= transform.decode = some sourceState

/-- Decoding and then encoding recovers every feasible target state.

This stronger property excludes noncanonical target representations which
decode to the same source state. -/
def TargetRoundTrip {source : Instance n} (transform : Transform source) : Prop :=
  ∀ {targetState},
    transform.target.Feasible targetState →
      transform.decode targetState >>= transform.encode = some targetState

/-- Reduction and relaxation together preserve existence of feasible states. -/
theorem feasible_nonempty_iff {source : Instance n}
    {transform : Transform source}
    (hreduction : transform.IsReduction)
    (hrelaxation : transform.IsRelaxation) :
    (∃ sourceState, source.Feasible sourceState) ↔
      ∃ targetState, transform.target.Feasible targetState := by
  constructor
  · rintro ⟨sourceState, hsource⟩
    rcases hrelaxation hsource with ⟨targetState, _, htarget⟩
    exact ⟨targetState, htarget⟩
  · rintro ⟨targetState, htarget⟩
    rcases hreduction htarget with ⟨sourceState, _, hsource⟩
    exact ⟨sourceState, hsource⟩

/-- Bidirectional feasibility and objective-value preservation identify the
sets of objective values attained by feasible states. -/
theorem objectiveRange_eq {source : Instance n}
    {transform : Transform source}
    (hreduction : transform.IsReduction)
    (hrelaxation : transform.IsRelaxation)
    (hsourceObjective : transform.SourceObjectiveValuePreserving)
    (htargetObjective : transform.TargetObjectiveValuePreserving) :
    source.ObjectiveRange = transform.target.ObjectiveRange := by
  ext value
  constructor
  · rintro ⟨sourceState, hsource, hvalue⟩
    rcases hrelaxation hsource with
      ⟨targetState, hencode, htarget⟩
    have hobjective :
        transform.target.ObjectiveValue targetState =
          source.ObjectiveValue sourceState := by
      simpa [hencode] using hsourceObjective hsource
    exact ⟨targetState, htarget, hobjective.trans hvalue⟩
  · rintro ⟨targetState, htarget, hvalue⟩
    rcases hreduction htarget with
      ⟨sourceState, hdecode, hsource⟩
    have hobjective :
        source.ObjectiveValue sourceState =
          transform.target.ObjectiveValue targetState := by
      simpa [hdecode] using htargetObjective htarget
    exact ⟨sourceState, hsource, hobjective.trans hvalue⟩

/-- The identity transformation. -/
def refl (source : Instance n) : Transform source where
  targetDimension := n
  target := source
  encode := some
  decode := some

theorem refl_isReduction (source : Instance n) :
    (refl source).IsReduction := by
  intro targetState hfeasible
  exact ⟨targetState, rfl, hfeasible⟩

theorem refl_isRelaxation (source : Instance n) :
    (refl source).IsRelaxation := by
  intro sourceState hfeasible
  exact ⟨sourceState, rfl, hfeasible⟩

theorem refl_sensePreserving (source : Instance n) :
    (refl source).SensePreserving :=
  rfl

theorem refl_sourceObjectiveValuePreserving (source : Instance n) :
    (refl source).SourceObjectiveValuePreserving := by
  intro sourceState _
  rfl

theorem refl_targetObjectiveValuePreserving (source : Instance n) :
    (refl source).TargetObjectiveValuePreserving := by
  intro targetState _
  rfl

theorem refl_sourceObjectivePreserving (source : Instance n) :
    (refl source).SourceObjectivePreserving :=
  ⟨refl_sensePreserving source,
    refl_sourceObjectiveValuePreserving source⟩

theorem refl_targetObjectivePreserving (source : Instance n) :
    (refl source).TargetObjectivePreserving :=
  ⟨refl_sensePreserving source,
    refl_targetObjectiveValuePreserving source⟩

theorem refl_sourceRoundTrip (source : Instance n) :
    (refl source).SourceRoundTrip := by
  intro sourceState _
  rfl

theorem refl_targetRoundTrip (source : Instance n) :
    (refl source).TargetRoundTrip := by
  intro targetState _
  rfl

/-- Compose transformations in their forward order.

`Option.bind` makes the composite encoding undefined whenever either encoding
step is undefined. Decoding composes in the reverse order. -/
def comp {source : Instance n} (first : Transform source)
    (second : Transform first.target) : Transform source where
  targetDimension := second.targetDimension
  target := second.target
  encode := fun state => first.encode state >>= second.encode
  decode := fun state => second.decode state >>= first.decode

theorem comp_isReduction {source : Instance n}
    {first : Transform source} {second : Transform first.target}
    (hfirst : first.IsReduction) (hsecond : second.IsReduction) :
    (comp first second).IsReduction := by
  intro targetState htarget
  rcases hsecond htarget with ⟨middleState, hdecodeSecond, hmiddle⟩
  rcases hfirst hmiddle with ⟨sourceState, hdecodeFirst, hsource⟩
  refine ⟨sourceState, ?_, hsource⟩
  simp [comp, hdecodeSecond, hdecodeFirst]

theorem comp_isRelaxation {source : Instance n}
    {first : Transform source} {second : Transform first.target}
    (hfirst : first.IsRelaxation) (hsecond : second.IsRelaxation) :
    (comp first second).IsRelaxation := by
  intro sourceState hsource
  rcases hfirst hsource with ⟨middleState, hencodeFirst, hmiddle⟩
  rcases hsecond hmiddle with ⟨targetState, hencodeSecond, htarget⟩
  refine ⟨targetState, ?_, htarget⟩
  simp [comp, hencodeFirst, hencodeSecond]

theorem comp_sensePreserving {source : Instance n}
    {first : Transform source} {second : Transform first.target}
    (hfirst : first.SensePreserving)
    (hsecond : second.SensePreserving) :
    (comp first second).SensePreserving :=
  hfirst.trans hsecond

theorem comp_sourceObjectiveValuePreserving {source : Instance n}
    {first : Transform source} {second : Transform first.target}
    (hfirstRelaxation : first.IsRelaxation)
    (hfirst : first.SourceObjectiveValuePreserving)
    (hsecond : second.SourceObjectiveValuePreserving) :
    (comp first second).SourceObjectiveValuePreserving := by
  intro sourceState hsource
  rcases hfirstRelaxation hsource with
    ⟨middleState, hencodeFirst, hmiddle⟩
  have hfirstObjective :
      first.target.ObjectiveValue middleState =
        source.ObjectiveValue sourceState := by
    simpa [hencodeFirst] using hfirst hsource
  simpa [comp, hencodeFirst, hfirstObjective] using hsecond hmiddle

theorem comp_targetObjectiveValuePreserving {source : Instance n}
    {first : Transform source} {second : Transform first.target}
    (hsecondReduction : second.IsReduction)
    (hfirst : first.TargetObjectiveValuePreserving)
    (hsecond : second.TargetObjectiveValuePreserving) :
    (comp first second).TargetObjectiveValuePreserving := by
  intro targetState htarget
  rcases hsecondReduction htarget with
    ⟨middleState, hdecodeSecond, hmiddle⟩
  have hsecondObjective :
      first.target.ObjectiveValue middleState =
        second.target.ObjectiveValue targetState := by
    simpa [hdecodeSecond] using hsecond htarget
  simpa [comp, hdecodeSecond, hsecondObjective] using hfirst hmiddle

theorem comp_sourceObjectivePreserving {source : Instance n}
    {first : Transform source} {second : Transform first.target}
    (hfirstRelaxation : first.IsRelaxation)
    (hfirst : first.SourceObjectivePreserving)
    (hsecond : second.SourceObjectivePreserving) :
    (comp first second).SourceObjectivePreserving :=
  ⟨comp_sensePreserving hfirst.1 hsecond.1,
    comp_sourceObjectiveValuePreserving
      hfirstRelaxation hfirst.2 hsecond.2⟩

theorem comp_targetObjectivePreserving {source : Instance n}
    {first : Transform source} {second : Transform first.target}
    (hsecondReduction : second.IsReduction)
    (hfirst : first.TargetObjectivePreserving)
    (hsecond : second.TargetObjectivePreserving) :
    (comp first second).TargetObjectivePreserving :=
  ⟨comp_sensePreserving hfirst.1 hsecond.1,
    comp_targetObjectiveValuePreserving
      hsecondReduction hfirst.2 hsecond.2⟩

theorem comp_sourceRoundTrip {source : Instance n}
    {first : Transform source} {second : Transform first.target}
    (hfirstRelaxation : first.IsRelaxation)
    (hfirst : first.SourceRoundTrip)
    (hsecond : second.SourceRoundTrip) :
    (comp first second).SourceRoundTrip := by
  intro sourceState hsource
  rcases hfirstRelaxation hsource with
    ⟨middleState, hencodeFirst, hmiddle⟩
  have hdecodeFirst :
      first.decode middleState = some sourceState := by
    simpa [hencodeFirst] using hfirst hsource
  have hmiddleRoundTrip := hsecond hmiddle
  simpa [comp, hencodeFirst, Option.bind_assoc, hdecodeFirst] using
    congrArg (fun state => state >>= first.decode) hmiddleRoundTrip

theorem comp_targetRoundTrip {source : Instance n}
    {first : Transform source} {second : Transform first.target}
    (hsecondReduction : second.IsReduction)
    (hfirst : first.TargetRoundTrip)
    (hsecond : second.TargetRoundTrip) :
    (comp first second).TargetRoundTrip := by
  intro targetState htarget
  rcases hsecondReduction htarget with
    ⟨middleState, hdecodeSecond, hmiddle⟩
  have hencodeSecond :
      second.encode middleState = some targetState := by
    have hroundTrip := hsecond htarget
    rw [hdecodeSecond] at hroundTrip
    exact hroundTrip
  have hmiddleRoundTrip := hfirst hmiddle
  simpa [comp, hdecodeSecond, Option.bind_assoc, hencodeSecond] using
    congrArg (fun state => state >>= second.encode) hmiddleRoundTrip

end Transform

end Instance

end OMMXProof
