import OMMXProof

/-!
# Phase A trust audit

This command inspects the environment imported by the production `OMMXProof`
aggregate rather than relying only on source text. Any axiom introduced by an
`OMMXProof` production module fails `lake test`, regardless of its declaration
namespace. Test-only fixture modules are intentionally not imported here.
Lean's standard logical axioms,
such as propositional extensionality and quotient soundness, live outside this
namespace and remain part of the stated trusted base.
-/

open Lean Elab Command

private def fromProductionModule (environment : Environment) (name : Name) : Bool :=
  match (environment.getModuleIdxFor? name).bind (environment.header.modules[·]?) with
  | some header =>
      let moduleName := header.module.toString
      moduleName == "OMMXProof" || moduleName.startsWith "OMMXProof."
  | none => false

private def projectAxioms (environment : Environment) : CommandElabM (Array Name) :=
  environment.constants.foldM (init := #[]) fun names name info => do
    if fromProductionModule environment name then
      match info with
      | .axiomInfo _ => return names.push name
      | _ => return names
    else
      return names

elab "#audit_ommx_axioms" : command => do
  let names ← projectAxioms (← getEnv)
  unless names.isEmpty do
    throwError "Project-defined axioms found: {names.toList}"

#audit_ommx_axioms
