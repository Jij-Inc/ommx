"""
@generated by mypy-protobuf.  Do not edit manually!
isort:skip_file
"""

import builtins
import collections.abc
import google.protobuf.descriptor
import google.protobuf.internal.containers
import google.protobuf.message
import typing

DESCRIPTOR: google.protobuf.descriptor.FileDescriptor

@typing.final
class Linear(google.protobuf.message.Message):
    DESCRIPTOR: google.protobuf.descriptor.Descriptor

    @typing.final
    class Term(google.protobuf.message.Message):
        DESCRIPTOR: google.protobuf.descriptor.Descriptor

        ID_FIELD_NUMBER: builtins.int
        COEFFICIENT_FIELD_NUMBER: builtins.int
        id: builtins.int
        coefficient: builtins.float
        def __init__(
            self,
            *,
            id: builtins.int = ...,
            coefficient: builtins.float = ...,
        ) -> None: ...
        def ClearField(
            self, field_name: typing.Literal["coefficient", b"coefficient", "id", b"id"]
        ) -> None: ...

    TERMS_FIELD_NUMBER: builtins.int
    CONSTANT_FIELD_NUMBER: builtins.int
    constant: builtins.float
    @property
    def terms(
        self,
    ) -> google.protobuf.internal.containers.RepeatedCompositeFieldContainer[
        global___Linear.Term
    ]: ...
    def __init__(
        self,
        *,
        terms: collections.abc.Iterable[global___Linear.Term] | None = ...,
        constant: builtins.float = ...,
    ) -> None: ...
    def ClearField(
        self, field_name: typing.Literal["constant", b"constant", "terms", b"terms"]
    ) -> None: ...

global___Linear = Linear