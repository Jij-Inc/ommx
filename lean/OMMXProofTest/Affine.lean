import OMMXProof.Function.Affine

/-!
# Affine box-bound fixtures

These fixtures exercise coefficient signs, one-sided unbounded domains, and
the rule that zero coefficients do not require domain endpoints.
-/

namespace OMMXProof.Test.Affine

/-- Regression fixture for the affine denotation: the constant is added once,
not once per component. -/
def constantOnlyExpr : OMMXProof.Affine 2 where
  coeff := fun _ => 0
  constant := 1

example : constantOnlyExpr.eval (fun _ => 0) = 1 := by native_decide

def mixedDomains : Fin 2 → Domain :=
  fun i =>
    if i = 0 then .binary
    else .integer (.finite (-2) 4 (by norm_num))

def mixedExpr : OMMXProof.Affine 2 where
  coeff := fun i => if i = 0 then 3 else -2
  constant := 1

example :
    mixedExpr.evaluateBoundFromDomains mixedDomains =
      .finite (-7) 8 (by norm_num) := by
  native_decide

example {state : State 2}
    (hdomains : ∀ i, state i ∈ mixedDomains i) :
    mixedExpr.eval state ∈ mixedExpr.evaluateBoundFromDomains mixedDomains :=
  OMMXProof.Affine.evaluateBoundFromDomains_sound mixedExpr mixedDomains hdomains

def identityExpr : OMMXProof.Affine 1 where
  coeff := fun _ => 1
  constant := 0

def upperOnlyDomain : Fin 1 → Domain :=
  fun _ => .continuous (.upperBounded 3)

/-- A finite upper bound remains available when the lower side is unbounded. -/
example :
    identityExpr.evaluateBoundFromDomains upperOnlyDomain =
      .upperBounded 3 := by
  native_decide

def zeroExpr : OMMXProof.Affine 1 where
  coeff := fun _ => 0
  constant := 5

def unboundedDomain : Fin 1 → Domain :=
  fun _ => .continuous

/-- An unbounded variable is irrelevant when its coefficient is zero. -/
example :
    zeroExpr.evaluateBoundFromDomains unboundedDomain =
      .point 5 := by
  native_decide

/-- Identity maps a two-sided unbounded domain to an unbounded interval. -/
example :
    identityExpr.evaluateBoundFromDomains unboundedDomain =
      .unbounded := by
  native_decide

end OMMXProof.Test.Affine
