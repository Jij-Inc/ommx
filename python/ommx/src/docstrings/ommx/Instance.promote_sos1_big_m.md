Validate and promote a batch of claimed SOS1 Big-M formulations.

The batch request is keyed by regular cardinality constraint ID.
The Rust {class}`Instance` checks every formulation against the same
unchanged instance and reconciles conflicts before applying the plan.
Bounds are inferred using only each formulation's claimed Big-M links.
Successful entries apply their prepared bounds together with promotion;
rejected entries contribute no bound changes, and strict rejection
leaves the entire instance unchanged.
Both modes return one {class}`~ommx.Sos1BigMPromotion` batch report:

- ``mode="best_effort"`` (default) applies independent valid formulations.
  The report's ``promoted`` map associates each successful cardinality
  ID with its allocated SOS1 ID; ``rejections`` maps rejected
  cardinality IDs to diagnostic strings.
- ``mode="strict"`` requires every formulation to be valid. Any
  rejection raises {class}`~ommx.Sos1BigMPromotionBatchRejectedError`
  before mutation. The exception's ``request_count`` is the batch
  size and ``rejections`` contains all rejected cardinality IDs and
  their diagnostics. A successful strict report has no rejections.

Overlapping formulation rows are rejected; independent formulations
may share SOS1 members. An empty request returns an empty report.
Planning and application do not clone the instance.

Promotion preserves the objective and mathematical feasible region on
original members after projecting out fresh selectors. Positive link
scaling is allowed; Big-M must cover the prepared member bounds exactly.
Bounds are derived directly from exact link-coefficient ratios without
tolerance expansion. Integer limits are rounded inward; conversion to
floating-point bounds rounds outward. Exact clipping retains even
sub-tolerance updates and does not read the default tolerance.
Coverage validation is exact.
Promotion does not promise identical
violations or feasibility classification at finite tolerance.
Unknown mode strings raise {class}`ValueError` before planning.

**The claimed formulation**

A Big-M formulation links each member $x_i$ to a Binary selector $y_i$ using
finite coefficients $M_i^+$ and $M_i^-$ that cover its domain:

$$
x_i-M_i^+y_i\leq0,\qquad -x_i-M_i^-y_i\leq0,\qquad \sum_i y_i\leq1.
$$

At $y_i=0$, the links force $x_i=0$, so at most one member can be nonzero.
Conversely, if the coefficients cover the prepared member domains, an
SOS1-feasible assignment can select its nonzero member, or select none when
all members are zero. Projecting out fresh selectors then gives the same feasible
member assignments. Binary members can act as their own selectors, and a side
already implied by a member's domain can be omitted.

The request uses {class}`~ommx.Sos1BigMSelectorClaim` to identify reused members
and fresh selectors with their link-row IDs. It is an unchecked claim about the
current Instance, not a declaration of new rows. The rows need not originate from
{meth}`~ommx.Instance.convert_sos1_to_constraints`. A fresh selector must be
private to its formulation and its domain must permit the canonical indicator
for every possible prepared member value.

All bound preparation reads the original variable domains and only the claimed
links; cardinality rows and unrelated constraints do not contribute. For
example, $x\leq3z$ with Binary $z$ can tighten a stored upper bound of $100$ to
$3$. Only accepted promotions commit their prepared bounds.

**Build a formulation and handle a batch**

The regular rows below encode an SOS1 containing a Binary member and an Integer
member. The Integer member uses a separate selector. A second request entry has
no matching cardinality row, illustrating rejection without relying on a solver.

```pycon
>>> from ommx import (
...     Bound,
...     ConstraintID,
...     DecisionVariable,
...     Instance,
...     Sense,
...     Sos1ConstraintID,
...     Sos1BigMPromotionBatchRejectedError,
...     Sos1BigMPromotionRequest,
...     Sos1BigMSelectorClaim,
... )
>>> binary_member = DecisionVariable.binary(0)
>>> integer_member = DecisionVariable.integer(1, lower=-100, upper=100)
>>> selector = DecisionVariable.binary(10)
>>> instance = Instance.from_components(
...     sense=Sense.Minimize,
...     objective=integer_member,
...     decision_variables=[binary_member, integer_member, selector],
...     constraints={
...         100: integer_member - 3 * selector <= 0,
...         101: -integer_member - 2 * selector <= 0,
...         102: binary_member + selector - 1 <= 0,
...     },
... )
>>> request = Sos1BigMPromotionRequest(
...     {
...         102: {
...             0: Sos1BigMSelectorClaim.reused(),
...             1: Sos1BigMSelectorClaim.fresh(10, upper_link=100, lower_link=101),
...         },
...         999: {},
...     }
... )
>>> before = instance.to_v2_bytes()
>>> try:
...     instance.promote_sos1_big_m(request, mode="strict")
... except Sos1BigMPromotionBatchRejectedError as error:
...     assert error.request_count == 2
...     assert set(error.rejections) == {ConstraintID(999)}
... else:
...     raise AssertionError("strict mode accepted an invalid formulation")
>>> assert instance.to_v2_bytes() == before
>>> report = instance.promote_sos1_big_m(request)
>>> assert report.request_count == 2
>>> assert report.promoted == {ConstraintID(102): Sos1ConstraintID(103)}
>>> assert set(report.rejections) == {ConstraintID(999)}
>>> assert set(report.promoted).isdisjoint(report.rejections)
>>> assert set(report.promoted) | set(report.rejections) == set(request.selector_claims)
>>> assert instance.get_decision_variable_by_id(1).bound == Bound(-2, 3)
>>> assert set(instance.constraints[Sos1ConstraintID(103)].variables) == {0, 1}
>>> assert set(instance.removed_constraints) == {
...     ConstraintID(100),
...     ConstraintID(101),
...     ConstraintID(102),
... }
>>> assert instance.populate_state({0: 0, 1: 2}).entries[10] == 1.0
>>> assert instance.evaluate({0: 0, 1: 2}).feasible

```

The report's maps are snapshots. A successful strict call returns the same
report type with no rejections; when strict mode rejects, entries absent from
the exception's rejection map were accepted during planning but were not applied.
Inspect members and retained formulation history on the mutated Instance using
the returned SOS1 IDs and original regular IDs.
