from __future__ import annotations

class Descriptor:
    @property
    def digest(self) -> str: ...
    @property
    def size(self) -> int: ...
    @property
    def annotations(self) -> dict[str, str]: ...
    @property
    def media_type(self) -> str: ...
    def __str__(self) -> str: ...
    def to_dict(self) -> dict[str, str | int | dict[str, str]]: ...
    def to_json(self) -> str: ...

class ArtifactArchive:
    @staticmethod
    def from_oci_archive(path: str) -> ArtifactArchive: ...
    @property
    def layers(self) -> list[Descriptor]: ...
    def get_blob(self, digest: str) -> bytes: ...

class ArtifactDir:
    @staticmethod
    def from_oci_dir(path: str) -> ArtifactDir: ...
    @staticmethod
    def from_image_name(image_name: str) -> ArtifactDir: ...
    @property
    def layers(self) -> list[Descriptor]: ...
    def get_blob(self, digest: str) -> bytes: ...

class ArtifactArchiveBuilder:
    @staticmethod
    def new(path: str, image_name: str) -> ArtifactArchiveBuilder: ...
    @staticmethod
    def new_unnamed(path: str) -> ArtifactArchiveBuilder: ...
    def add_layer(
        self, media_type: str, blob: bytes, annotations: dict[str, str]
    ) -> Descriptor: ...
    def build(self) -> ArtifactArchive: ...

class ArtifactDirBuilder:
    @staticmethod
    def new(image_name: str) -> ArtifactDirBuilder: ...
    def add_layer(
        self, media_type: str, blob: bytes, annotations: dict[str, str]
    ) -> Descriptor: ...
    def build(self) -> ArtifactDir: ...

def evaluate_function(evaluated: bytes, state: bytes) -> tuple[float, set[int]]: ...
def evaluate_linear(evaluated: bytes, state: bytes) -> tuple[float, set[int]]: ...
def evaluate_quadratic(evaluated: bytes, state: bytes) -> tuple[float, set[int]]: ...
def evaluate_polynomial(evaluated: bytes, state: bytes) -> tuple[float, set[int]]: ...
def evaluate_constraint(evaluated: bytes, state: bytes) -> tuple[bytes, set[int]]: ...
def evaluate_instance(evaluated: bytes, state: bytes) -> tuple[bytes, set[int]]: ...
