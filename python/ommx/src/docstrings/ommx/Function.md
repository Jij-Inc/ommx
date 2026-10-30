A scalar expression used in optimization objectives and constraint bodies.

Function refers to variables by ID, without owning their names, kinds, or bounds.
It can be evaluated independently of a solver. Use
{meth}`~ommx.Instance.evaluate` when variable domains and constraints must also be
checked, and {class}`~ommx.InstanceClass` to inspect a solver-input representation.
Being evaluable as a Function does not imply that an Adapter accepts it.

**Evaluate an expression or a candidate**

An expression can have a value even when the supplied candidate violates a
constraint. Here the expression is ``2*x + 3``, while the problem also requires
``x == 0``:

>>> from ommx import Function, Instance
>>> instance = Instance.minimize()
>>> x = instance.new_binary("x")
>>> f = Function(2 * x + 3)
>>> instance.objective = f
>>> _ = instance.add_constraint(x == 0)
>>> candidate = {x.id: 1}
>>> assert f.evaluate(candidate) == 5.0
>>> assert not instance.evaluate(candidate).feasible

The {class}`~ommx.Solution` records the problem-level evaluation. To register
other quantities to report for each candidate, use {class}`~ommx.NamedFunction`;
those quantities do not become objectives or constraints.

**Construction and polynomial forms**

Construct a Function from numbers, decision variables, parameters, polynomial
objects, or another Function. {class}`~ommx.Linear` stores a linear polynomial,
{class}`~ommx.Quadratic` a polynomial of degree at most two, and
{class}`~ommx.Polynomial` an arbitrary-degree polynomial. Function is the common
wrapper for these forms and for composed scalar expressions.

>>> from ommx import DecisionVariable, Function, Linear
>>> x = DecisionVariable.continuous(0)
>>> y = DecisionVariable.continuous(1)
>>> constant = Function(3)
>>> f = Function(x + 2 * y + 3)
>>> by_coefficients = Function(Linear(terms={0: 1, 1: 2}, constant=3))
>>> assert f.almost_equal(by_coefficients)
>>> assert f.evaluate({0: 1, 1: 0}) == 4.0
>>> assert f.linear_terms == {0: 1.0, 1: 2.0}
>>> assert f.constant_term == 3.0
>>> assert f.terms == {(): 3.0, (0,): 1.0, (1,): 2.0}
>>> quadratic = Function(x * x + x * y + y * y)
>>> cubic = Function(x * x * x + y * y)
>>> assert quadratic.degree() == 2
>>> assert cubic.degree() == 3

Addition, subtraction, multiplication, and negation of stored polynomials are
normalized into polynomial form. Terms that become exactly zero through
cancellation or floating-point underflow are removed. Finite nonzero coefficients
are retained regardless of magnitude; the evaluation tolerance does not prune
them. This describes Function arithmetic, not the numerical behavior of a
downstream coefficient-format conversion.

Use {meth}`~ommx.Function.degree` and {meth}`~ommx.Function.num_terms` for polynomial
metadata, and {attr}`~ommx.Function.terms`, {attr}`~ommx.Function.linear_terms`,
{attr}`~ommx.Function.quadratic_terms`, and {attr}`~ommx.Function.constant_term`
for coefficients. Composed expressions return ``None``
for degree/term count and raise ``TypeError`` for coefficient properties; these
accessors do not expand an expression. {meth}`~ommx.Function.almost_equal` compares coefficients
or expression structure, not global mathematical equivalence.

**Composed scalar operations**

Use Python arithmetic and ``abs(f)`` to construct expressions, together with
{meth}`~ommx.Function.signum`, {meth}`~ommx.Function.minimum`,
{meth}`~ommx.Function.maximum`, and {meth}`~ommx.Function.powi`. These calls
construct expressions rather than supplying variable values. For pointwise
minimum and maximum, use the methods rather than Python's built-in ``min`` and
``max``.

>>> fx, fy = Function(x), Function(y)
>>> assert abs(fx - 3).evaluate({0: 1}) == 2.0
>>> assert fx.minimum(fy).evaluate({0: 1, 1: 0}) == 0.0
>>> assert (fx / (fy + 1)).evaluate({0: 1, 1: 1}) == 0.5
>>> assert (fx ** 2).evaluate({0: 1}) == 1.0

Division remains a composed expression even with a constant divisor. Except for
identity operations and finite constant folding, integer powers also remain
composed, including nonnegative powers. To construct an expanded polynomial
square, use ``g * g`` rather than ``g ** 2``. See {meth}`~ommx.Function.powi` for the supported
exponent types and domain rules.

The current {class}`~ommx.InstanceClassClause` requirements describe polynomial
objectives and constraint bodies. A composed expression therefore needs an
appropriate polynomial reformulation before it can be passed to the bundled
solver adapters; evaluating it successfully does not establish input-class
membership.

**Evaluation, substitution, and stored order**

{meth}`~ommx.Function.evaluate` supplies the point-evaluation contract, including tolerance-based
zero tests, undefined operations, and non-finite values. {meth}`~ommx.Function.partial_evaluate`
returns an expression with selected values substituted and does not fix the
tolerance for a later evaluation. {meth}`~ommx.Function.evaluate_bound` returns an enclosing
interval, which need not be the tightest possible range.

Composed expressions retain parentheses and operand order. They evaluate left
to right, combining two values per operator application rather than automatically
reassociating expressions such as ``(abs(a) + abs(b)) + abs(c)`` and
``abs(a) + (abs(b) + abs(c))``. A domain or overflow error can depend on that
order.

The wire representation uses a flat reverse-Polish-notation (RPN) instruction
sequence rather than a recursively nested tree. Its encoding belongs to the
[OMMX Message specification](https://github.com/Jij-Inc/ommx/tree/main/proto);
constructing a Python Function does not require creating RPN instructions.

**Read expressions using model labels**

The context-free ``str`` and ``repr`` representations use variable IDs. For
names and subscripts, use {meth}`~ommx.Instance.format_function`, or
{meth}`~ommx.Instance.display_function` for a bounded Notebook preview. These
operations resolve IDs using the supplied model's labels; they do not remap
variables from another model by name. A {class}`~ommx.ParametricInstance` also
provides these methods and can resolve parameter labels.
