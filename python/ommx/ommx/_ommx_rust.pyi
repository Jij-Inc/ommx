from __future__ import annotations

class Descriptor:
    @property
    def digest(self) -> str: ...
    @property
    def size(self) -> int: ...
    @property
    def annotations(self) -> dict[str, str]: ...
    @property
    def user_annotations(self) -> dict[str, str]: ...
    @property
    def media_type(self) -> str: ...
    def __str__(self) -> str: ...
    def to_dict(self) -> dict[str, str | int | dict[str, str]]: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_dict(d: dict[str, str | int | dict[str, str]]) -> Descriptor: ...
    @staticmethod
    def from_json(s: str) -> Descriptor: ...

class ArtifactArchive:
    @staticmethod
    def from_oci_archive(path: str) -> ArtifactArchive: ...
    @property
    def image_name(self) -> str | None: ...
    @property
    def annotations(self) -> dict[str, str]: ...
    @property
    def layers(self) -> list[Descriptor]: ...
    def get_blob(self, digest: str) -> bytes: ...
    def push(self): ...

class ArtifactDir:
    @staticmethod
    def from_oci_dir(path: str) -> ArtifactDir: ...
    @staticmethod
    def from_image_name(image_name: str) -> ArtifactDir: ...
    @property
    def image_name(self) -> str | None: ...
    @property
    def annotations(self) -> dict[str, str]: ...
    @property
    def layers(self) -> list[Descriptor]: ...
    def get_blob(self, digest: str) -> bytes: ...
    def push(self): ...

class ArtifactArchiveBuilder:
    @staticmethod
    def new(path: str, image_name: str) -> ArtifactArchiveBuilder: ...
    @staticmethod
    def new_unnamed(path: str) -> ArtifactArchiveBuilder: ...
    @staticmethod
    def temp() -> ArtifactArchiveBuilder: ...
    def add_layer(
        self, media_type: str, blob: bytes, annotations: dict[str, str]
    ) -> Descriptor: ...
    def add_annotation(self, key: str, value: str): ...
    def build(self) -> ArtifactArchive: ...

class ArtifactDirBuilder:
    @staticmethod
    def new(image_name: str) -> ArtifactDirBuilder: ...
    @staticmethod
    def for_github(org: str, repo: str, name: str, tag: str) -> ArtifactDirBuilder: ...
    def add_layer(
        self, media_type: str, blob: bytes, annotations: dict[str, str]
    ) -> Descriptor: ...
    def add_annotation(self, key: str, value: str): ...
    def build(self) -> ArtifactDir: ...

def evaluate_function(evaluated: bytes, state: bytes) -> tuple[float, set[int]]: ...
def evaluate_linear(evaluated: bytes, state: bytes) -> tuple[float, set[int]]: ...
def evaluate_quadratic(evaluated: bytes, state: bytes) -> tuple[float, set[int]]: ...
def evaluate_polynomial(evaluated: bytes, state: bytes) -> tuple[float, set[int]]: ...
def evaluate_constraint(evaluated: bytes, state: bytes) -> tuple[bytes, set[int]]: ...
def evaluate_instance(evaluated: bytes, state: bytes) -> tuple[bytes, set[int]]: ...
def used_decision_variable_ids(function: bytes) -> set[int]: ...
