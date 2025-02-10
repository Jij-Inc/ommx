from __future__ import annotations
from datetime import datetime
from dateutil import parser
from abc import ABC, abstractmethod
import json


class UserAnnotationBase(ABC):
    @property
    @abstractmethod
    def _annotations(self) -> dict[str, str]: ...

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
