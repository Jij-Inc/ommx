# This file is automatically generated by pyo3_stub_gen
# ruff: noqa: E501, F401

import os
import pathlib
import typing

class ArtifactArchive:
    image_name: typing.Optional[str]
    annotations: dict[str, str]
    layers: list[Descriptor]
    @staticmethod
    def from_oci_archive(path: str | os.PathLike | pathlib.Path) -> ArtifactArchive: ...
    def get_blob(self, digest: str) -> bytes: ...
    def push(self) -> None: ...

class ArtifactArchiveBuilder:
    @staticmethod
    def new_unnamed(
        path: str | os.PathLike | pathlib.Path,
    ) -> ArtifactArchiveBuilder: ...
    @staticmethod
    def new(
        path: str | os.PathLike | pathlib.Path, image_name: str
    ) -> ArtifactArchiveBuilder: ...
    @staticmethod
    def temp() -> ArtifactArchiveBuilder: ...
    def add_layer(
        self, media_type: str, blob: bytes, annotations: typing.Mapping[str, str]
    ) -> Descriptor: ...
    def add_annotation(self, key: str, value: str) -> None: ...
    def build(self) -> ArtifactArchive: ...

class ArtifactDir:
    image_name: typing.Optional[str]
    annotations: dict[str, str]
    layers: list[Descriptor]
    @staticmethod
    def from_image_name(image_name: str) -> ArtifactDir: ...
    @staticmethod
    def from_oci_dir(path: str | os.PathLike | pathlib.Path) -> ArtifactDir: ...
    def get_blob(self, digest: str) -> bytes: ...
    def push(self) -> None: ...

class ArtifactDirBuilder:
    @staticmethod
    def new(image_name: str) -> ArtifactDirBuilder: ...
    @staticmethod
    def for_github(org: str, repo: str, name: str, tag: str) -> ArtifactDirBuilder: ...
    def add_layer(
        self, media_type: str, blob: bytes, annotations: typing.Mapping[str, str]
    ) -> Descriptor: ...
    def add_annotation(self, key: str, value: str) -> None: ...
    def build(self) -> ArtifactDir: ...

class Descriptor:
    r"""
    Descriptor of blob in artifact
    """

    digest: str
    size: int
    media_type: str
    annotations: dict[str, str]
    user_annotations: dict[str, str]
    def to_dict(self) -> dict: ...
    @staticmethod
    def from_dict(dict: dict) -> Descriptor: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> Descriptor: ...
    def __str__(self) -> str: ...
    def __eq__(self, rhs: typing.Any) -> bool: ...

class Function:
    @staticmethod
    def from_scalar(scalar: float) -> Function: ...
    @staticmethod
    def from_linear(linear: Linear) -> Function: ...
    @staticmethod
    def from_quadratic(quadratic: Quadratic) -> Function: ...
    @staticmethod
    def from_polynomial(polynomial: Polynomial) -> Function: ...
    @staticmethod
    def decode(bytes: bytes) -> Function: ...
    def encode(self) -> bytes: ...
    def almost_equal(self, other: Function, atol: float) -> bool: ...
    def __repr__(self) -> str: ...
    def __add__(self, rhs: Function) -> Function: ...
    def __sub__(self, rhs: Function) -> Function: ...
    def __mul__(self, rhs: Function) -> Function: ...
    def add_scalar(self, scalar: float) -> Function: ...
    def add_linear(self, linear: Linear) -> Function: ...
    def add_quadratic(self, quadratic: Quadratic) -> Function: ...
    def add_polynomial(self, polynomial: Polynomial) -> Function: ...
    def mul_scalar(self, scalar: float) -> Function: ...
    def mul_linear(self, linear: Linear) -> Function: ...
    def mul_quadratic(self, quadratic: Quadratic) -> Function: ...
    def mul_polynomial(self, polynomial: Polynomial) -> Function: ...

class Instance:
    @staticmethod
    def from_bytes(bytes: bytes) -> Instance: ...
    def to_bytes(self) -> bytes: ...
    def validate(self) -> None: ...
    def as_pubo_format(self) -> dict: ...
    def as_qubo_format(self) -> tuple[dict, float]: ...
    def as_parametric_instance(self) -> ParametricInstance: ...
    def penalty_method(self) -> ParametricInstance: ...

class Linear:
    @staticmethod
    def single_term(id: int, coefficient: float) -> Linear: ...
    @staticmethod
    def decode(bytes: bytes) -> Linear: ...
    def encode(self) -> bytes: ...
    def almost_equal(self, other: Linear, atol: float) -> bool: ...
    def __repr__(self) -> str: ...
    def __add__(self, rhs: Linear) -> Linear: ...
    def __sub__(self, rhs: Linear) -> Linear: ...
    def __mul__(self, rhs: Linear) -> Quadratic: ...
    def add_scalar(self, scalar: float) -> Linear: ...
    def mul_scalar(self, scalar: float) -> Linear: ...

class Parameters:
    @staticmethod
    def from_bytes(bytes: bytes) -> Parameters: ...
    def to_bytes(self) -> bytes: ...

class ParametricInstance:
    @staticmethod
    def from_bytes(bytes: bytes) -> ParametricInstance: ...
    def to_bytes(self) -> bytes: ...
    def validate(self) -> None: ...
    def with_parameters(self, parameters: Parameters) -> Instance: ...

class Polynomial:
    @staticmethod
    def decode(bytes: bytes) -> Polynomial: ...
    def encode(self) -> bytes: ...
    def almost_equal(self, other: Polynomial, atol: float) -> bool: ...
    def __repr__(self) -> str: ...
    def __add__(self, rhs: Polynomial) -> Polynomial: ...
    def __sub__(self, rhs: Polynomial) -> Polynomial: ...
    def __mul__(self, rhs: Polynomial) -> Polynomial: ...
    def add_scalar(self, scalar: float) -> Polynomial: ...
    def add_linear(self, linear: Linear) -> Polynomial: ...
    def add_quadratic(self, quadratic: Quadratic) -> Polynomial: ...
    def mul_scalar(self, scalar: float) -> Polynomial: ...
    def mul_linear(self, linear: Linear) -> Polynomial: ...
    def mul_quadratic(self, quadratic: Quadratic) -> Polynomial: ...

class Quadratic:
    @staticmethod
    def decode(bytes: bytes) -> Quadratic: ...
    def encode(self) -> bytes: ...
    def almost_equal(self, other: Quadratic, atol: float) -> bool: ...
    def __repr__(self) -> str: ...
    def __add__(self, rhs: Quadratic) -> Quadratic: ...
    def __sub__(self, rhs: Quadratic) -> Quadratic: ...
    def __mul__(self, rhs: Quadratic) -> Polynomial: ...
    def add_scalar(self, scalar: float) -> Quadratic: ...
    def add_linear(self, linear: Linear) -> Quadratic: ...
    def mul_scalar(self, scalar: float) -> Quadratic: ...
    def mul_linear(self, linear: Linear) -> Polynomial: ...

def evaluate_constraint(function: bytes, state: bytes) -> tuple[bytes, set[int]]: ...
def evaluate_function(function: bytes, state: bytes) -> tuple[float, set[int]]: ...
def evaluate_instance(function: bytes, state: bytes) -> tuple[bytes, set[int]]: ...
def evaluate_linear(function: bytes, state: bytes) -> tuple[float, set[int]]: ...
def evaluate_polynomial(function: bytes, state: bytes) -> tuple[float, set[int]]: ...
def evaluate_quadratic(function: bytes, state: bytes) -> tuple[float, set[int]]: ...
def load_mps_bytes(path: str) -> bytes: ...
def miplib2017_instance_annotations() -> dict[str, dict[str, str]]: ...
def partial_evaluate_constraint(obj: bytes, state: bytes) -> tuple[bytes, set[int]]: ...
def partial_evaluate_function(
    function: bytes, state: bytes
) -> tuple[bytes, set[int]]: ...
def partial_evaluate_instance(obj: bytes, state: bytes) -> tuple[bytes, set[int]]: ...
def partial_evaluate_linear(
    function: bytes, state: bytes
) -> tuple[bytes, set[int]]: ...
def partial_evaluate_polynomial(
    function: bytes, state: bytes
) -> tuple[bytes, set[int]]: ...
def partial_evaluate_quadratic(
    function: bytes, state: bytes
) -> tuple[bytes, set[int]]: ...
def used_decision_variable_ids(function: bytes) -> set[int]: ...
def write_mps_file(instance: bytes, path: str) -> None: ...
