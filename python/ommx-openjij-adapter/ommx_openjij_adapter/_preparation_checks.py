"""OpenJij preparation source class and shared identity helpers."""

from __future__ import annotations

from ommx import (
    DegreeBound,
    Equality,
    Instance,
    InstanceClass,
    InstanceClassClause,
    Kind,
    Sense,
)
from ommx.adapter import ConstraintRef

from ._preparation import OpenJijPreparationSourceCheck


# This describes sources admitted to explicit preparation, not inputs accepted
# directly by the OpenJij Adapter. Operation-specific availability conditions
# belong to the phase that performs that operation.
PREPARATION_SOURCE_CLASS = InstanceClass(
    [
        InstanceClassClause(
            label="openjij-preparation-source",
            allowed_variable_kinds={Kind.Binary, Kind.Integer},
            objective_degree_bound=DegreeBound.unbounded(),
            regular_constraint_degree_bounds={
                Equality.EqualToZero: DegreeBound.unbounded(),
                Equality.LessThanOrEqualToZero: DegreeBound.unbounded(),
            },
            indicator_constraint_degree_bounds={
                Equality.EqualToZero: DegreeBound.unbounded(),
                Equality.LessThanOrEqualToZero: DegreeBound.unbounded(),
            },
            allows_one_hot=True,
            allows_sos1=True,
            allowed_senses={Sense.Minimize, Sense.Maximize},
        )
    ]
)


def active_constraint_refs(ommx_instance: Instance) -> frozenset[ConstraintRef]:
    return frozenset(
        [
            *(ConstraintRef("regular", id) for id in ommx_instance.constraints),
            *(
                ConstraintRef("indicator", id)
                for id in ommx_instance.indicator_constraints
            ),
            *(ConstraintRef("one_hot", id) for id in ommx_instance.one_hot_constraints),
            *(ConstraintRef("sos1", id) for id in ommx_instance.sos1_constraints),
        ]
    )


def check_preparation_source(ommx_instance: Instance) -> OpenJijPreparationSourceCheck:
    membership = PREPARATION_SOURCE_CLASS.check_membership(ommx_instance)
    return OpenJijPreparationSourceCheck(source_membership=membership)
