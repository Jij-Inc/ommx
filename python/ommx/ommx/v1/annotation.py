from __future__ import annotations
from datetime import datetime
from dateutil import parser
from abc import ABC, abstractmethod
import json
from typing import Literal, TypeVar

from .instance_pb2 import Instance as _Instance
from .parametric_instance_pb2 import ParametricInstance as _ParametricInstance
from .solution_pb2 import Solution as _Solution
from .sample_set_pb2 import SampleSet as _SampleSet


_AnnotatedMessage = _Instance | _ParametricInstance | _Solution | _SampleSet
_DESCRIPTION_FIELDS: dict[
    str, Literal["name", "description", "created_by", "license", "dataset", "created"]
] = {
    "title": "name",
    "description": "description",
    "created_by": "created_by",
    "license": "license",
    "dataset": "dataset",
    "created": "created",
}
_PROCESS_FIELDS = ("instance", "solver", "parameters", "start", "end")
_AnnotatedRoot = TypeVar("_AnnotatedRoot", bound="UserAnnotationBase")


def _annotations_from_proto(
    message: _AnnotatedMessage, namespace: str
) -> dict[str, str]:
    annotations = dict(message.annotations)
    for key in annotations:
        if key.startswith("org.ommx.v1."):
            raise ValueError(f"Annotation key {key!r} is reserved for OMMX metadata.")
    if isinstance(message, (_Instance, _ParametricInstance)):
        for key, field_name in _DESCRIPTION_FIELDS.items():
            if message.description.HasField(field_name):
                annotations[f"{namespace}.{key}"] = getattr(
                    message.description, field_name
                )
        if message.description.authors:
            annotations[f"{namespace}.authors"] = ",".join(message.description.authors)
    else:
        for field_name in _PROCESS_FIELDS:
            if message.metadata.HasField(field_name):
                annotations[f"{namespace}.{field_name}"] = getattr(
                    message.metadata, field_name
                )
    return annotations


def _annotations_to_proto(
    message: _AnnotatedMessage, annotations: dict[str, str], namespace: str
) -> None:
    """Project the v2 dictionary into the same wire fields as SDK v3."""
    if isinstance(message, (_Instance, _ParametricInstance)):
        description = message.description
        for key, field_name in _DESCRIPTION_FIELDS.items():
            value = annotations.get(f"{namespace}.{key}")
            description.ClearField(field_name)
            if value is not None:
                setattr(description, field_name, value)
        del description.authors[:]
        authors = annotations.get(f"{namespace}.authors")
        if authors:
            description.authors.extend(authors.split(","))
        if not description.ListFields():
            message.ClearField("description")
        reserved_keys = {*_DESCRIPTION_FIELDS, "authors", "variables", "constraints"}
    else:
        message.ClearField("metadata")
        for field_name in _PROCESS_FIELDS:
            value = annotations.get(f"{namespace}.{field_name}")
            if value is not None:
                setattr(message.metadata, field_name, value)
        reserved_keys = set(_PROCESS_FIELDS)

    allowed_reserved_keys = {f"{namespace}.{name}" for name in reserved_keys}
    extensions = {}
    for key, value in annotations.items():
        if key.startswith("org.ommx.v1."):
            if key not in allowed_reserved_keys:
                raise ValueError(
                    f"Annotation key {key!r} is reserved for OMMX metadata. "
                    "Use a user or third-party namespace for custom annotations."
                )
        else:
            extensions[key] = value
    message.annotations.clear()
    message.annotations.update(extensions)


class UserAnnotationBase(ABC):
    annotation_namespace: str

    @property
    @abstractmethod
    def _annotations(self) -> dict[str, str]: ...

    def _with_annotations(self, target: _AnnotatedRoot) -> _AnnotatedRoot:
        """Copy metadata when an operation returns another model or output view."""
        prefix = f"{self.annotation_namespace}."
        target._annotations.clear()
        target._annotations.update(
            {
                f"{target.annotation_namespace}.{key[len(prefix) :]}"
                if key.startswith(prefix)
                else key: value
                for key, value in self._annotations.items()
            }
        )
        return target

    def add_user_annotation(
        self, key: str, value: str, *, annotation_namespace: str = "org.ommx.user."
    ):
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        self._annotations[annotation_namespace + key] = value

    def add_user_annotations(
        self,
        annotations: dict[str, str],
        *,
        annotation_namespace: str = "org.ommx.user.",
    ):
        for key, value in annotations.items():
            self.add_user_annotation(
                key, value, annotation_namespace=annotation_namespace
            )

    def get_user_annotation(
        self, key: str, *, annotation_namespace: str = "org.ommx.user."
    ):
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        return self._annotations[annotation_namespace + key]

    def get_user_annotations(
        self, *, annotation_namespace: str = "org.ommx.user."
    ) -> dict[str, str]:
        if not annotation_namespace.endswith("."):
            annotation_namespace += "."
        return {
            key[len(annotation_namespace) :]: value
            for key, value in self._annotations.items()
            if key.startswith(annotation_namespace)
        }


def str_annotation_property(name: str):
    def getter(self):
        return self._annotations.get(f"{self.annotation_namespace}.{name}")

    def setter(self, value: str):
        self._annotations[f"{self.annotation_namespace}.{name}"] = value

    return property(getter, setter)


def str_list_annotation_property(name: str):
    def getter(self):
        value = self._annotations.get(f"{self.annotation_namespace}.{name}")
        if value:
            return value.split(",")
        else:
            return []

    def setter(self, value: list[str]):
        self._annotations[f"{self.annotation_namespace}.{name}"] = ",".join(value)

    return property(getter, setter)


def int_annotation_property(name: str):
    def getter(self):
        value = self._annotations.get(f"{self.annotation_namespace}.{name}")
        if value:
            return int(value)
        else:
            return None

    def setter(self, value: int):
        self._annotations[f"{self.annotation_namespace}.{name}"] = str(value)

    return property(getter, setter)


def datetime_annotation_property(name: str):
    def getter(self):
        value = self._annotations.get(f"{self.annotation_namespace}.{name}")
        if value:
            return parser.isoparse(value)
        else:
            return None

    def setter(self, value: datetime):
        self._annotations[f"{self.annotation_namespace}.{name}"] = value.isoformat()

    return property(getter, setter)


def json_annotation_property(name: str):
    def getter(self):
        value = self._annotations.get(f"{self.annotation_namespace}.{name}")
        if value:
            return json.loads(value)
        else:
            return None

    def setter(self, value: dict):
        self._annotations[f"{self.annotation_namespace}.{name}"] = json.dumps(value)

    return property(getter, setter)
