import OMMXProof.Instance

/-!
# Instance transformations

An `Instance.Transform source` records a transformed Instance together with
partial state maps in both directions. It deliberately imposes no semantic
correctness by itself: reduction and relaxation are separate predicates.
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

end Transform

end Instance

end OMMXProof
