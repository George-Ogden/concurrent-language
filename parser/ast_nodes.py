from __future__ import annotations

import enum
import inspect
import typing
from dataclasses import dataclass
from types import NoneType
from typing import Any, ClassVar, Optional, Type, TypeAlias, Union


class ASTNode:
    SUBSTITUTIONS: ClassVar[dict[str, str]] = {"type": "type_"}

    def to_json(self) -> Any:
        annotations = inspect.get_annotations(type(self), eval_str=True)
        attrs = (
            (
                self.SUBSTITUTIONS.get(attr, attr),
                self.convert_to_json(getattr(self, attr), type_=annotations[attr]),
            )
            for attr in self.__match_args__
        )
        return {key: value for key, value in attrs}

    @classmethod
    def convert_to_json(cls, value: Any, type_: Optional[Type] = None) -> Optional[Any]:
        value_type = type(value)
        if type_ is None:
            type_ = value_type
        if isinstance(value, ASTNode):
            if typing.get_origin(type_) == Union and value_type in typing.get_args(type_):
                return {value_type.__name__: cls.convert_to_json(value, value_type)}
            else:
                return value.to_json()
        elif isinstance(value, list):
            node_type = typing.get_args(type_)
            if len(node_type) == 1:
                [type_] = node_type
            else:
                type_ = None
            return [cls.convert_to_json(node, type_=type_) for node in value]
        elif isinstance(value, (Id, NoneType, int)):
            return value


Id: TypeAlias = str


@dataclass
class Import(ASTNode): ...


@dataclass
class FunctionType(ASTNode):
    argument_type: TypeInstance
    return_type: TypeInstance


@dataclass
class GenericType(ASTNode):
    id: Id
    type_variables: list[TypeInstance]


@dataclass
class TupleType(ASTNode):
    types: list[TypeInstance]


class AtomicTypeEnum(ASTNode, enum.IntEnum):
    INT = enum.auto()
    BOOL = enum.auto()

    def to_json(self) -> Any:
        return self.name


@dataclass
class AtomicType(ASTNode):
    type: AtomicTypeEnum
    INT: ClassVar[AtomicType]
    BOOL: ClassVar[AtomicType]


AtomicType.INT = AtomicType(AtomicTypeEnum.INT)
AtomicType.BOOL = AtomicType(AtomicTypeEnum.BOOL)

TypeInstance: TypeAlias = Union[FunctionType, GenericType, TupleType, AtomicType]


@dataclass
class TypeItem(ASTNode):
    id: Id
    type: Optional[TypeInstance]


@dataclass
class UnionTypeDefinition(ASTNode):
    variable: GenericTypeVariable
    items: list[TypeItem]


@dataclass
class OpaqueTypeDefinition(ASTNode):
    variable: GenericTypeVariable
    type: TypeInstance


@dataclass
class EmptyTypeDefinition(ASTNode):
    id: Id


@dataclass
class Assignee(ASTNode):
    id: Id
    generic_variables: list[Id]


@dataclass
class TypedAssignee(ASTNode):
    assignee: Assignee
    type: TypeInstance


@dataclass
class FunctionCall(ASTNode):
    function: Expression
    arguments: list[Expression]


@dataclass
class Integer(ASTNode):
    value: int


@dataclass
class Boolean(ASTNode):
    value: bool


@dataclass
class ElementAccess(ASTNode):
    expression: Expression
    index: int


@dataclass
class GenericVariable(ASTNode):
    name: Id
    type_instances: list[TypeInstance]


@dataclass
class IfExpression(ASTNode):
    condition: Expression
    true_block: Block
    false_block: Block


@dataclass
class MatchItem(ASTNode):
    type_name: str
    assignee: Optional[Assignee]


@dataclass
class MatchBlock(ASTNode):
    matches: list[MatchItem]
    block: Block


@dataclass
class MatchExpression(ASTNode):
    subject: Expression
    blocks: list[MatchBlock]


@dataclass
class TupleExpression(ASTNode):
    expressions: list[Expression]


@dataclass
class FunctionDefinition(ASTNode):
    parameters: list[TypedAssignee]
    return_type: TypeInstance
    body: Block


@dataclass
class GenericConstructor(ASTNode):
    name: Id
    type_instances: list[TypeInstance]


@dataclass
class ConstructorCall(ASTNode):
    constructor: GenericConstructor
    arguments: list[Expression]


Expression: TypeAlias = Union[
    FunctionCall,
    Integer,
    Boolean,
    ElementAccess,
    GenericVariable,
    IfExpression,
    MatchExpression,
    TupleExpression,
    FunctionDefinition,
    ConstructorCall,
]


@dataclass
class Assignment(ASTNode):
    assignee: Assignee
    expression: Expression


@dataclass
class Block(ASTNode):
    assignments: list[Assignment]
    expression: Expression


@dataclass
class GenericTypeVariable(ASTNode):
    id: Id
    generic_variables: list[Id]


@dataclass
class TransparentTypeDefinition(ASTNode):
    variable: GenericTypeVariable
    type: TypeInstance


Definition: TypeAlias = Union[
    UnionTypeDefinition,
    OpaqueTypeDefinition,
    EmptyTypeDefinition,
    TransparentTypeDefinition,
    Assignment,
]


@dataclass
class Program(ASTNode):
    imports: list[Import]
    definitions: list[Definition]


def Variable(name: Id) -> GenericVariable:
    return GenericVariable(name, [])


def Typename(name: Id) -> GenericType:
    return GenericType(name, [])


def TypeVariable(name: Id) -> GenericTypeVariable:
    return GenericTypeVariable(name, [])


def Constructor(name: Id) -> GenericConstructor:
    return GenericConstructor(name, [])
