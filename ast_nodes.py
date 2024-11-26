from __future__ import annotations

import enum
from dataclasses import dataclass
from typing import ClassVar, Optional, TypeAlias, Union


class ASTNode: ...


Id: TypeAlias = str


@dataclass
class Import(ASTNode): ...


@dataclass
class FunctionType(ASTNode):
    argument_type: TypeInstance
    return_type: TypeInstance


@dataclass
class GenericType(ASTNode):
    type_variables: list[Id]
    type: TypeInstance


@dataclass
class TupleType(ASTNode):
    types: list[TypeInstance]


class AtomicTypeEnum(enum.IntEnum):
    INT = enum.auto()
    BOOL = enum.auto()


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
    type: TypeInstance


@dataclass
class UnionTypeDefinition(ASTNode):
    items: list[TypeItem]


@dataclass
class OpaqueTypeDefinition(ASTNode):
    type: TypeInstance


@dataclass
class EmptyTypeDefinition(ASTNode): ...


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
    args: list[Expression]


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
    type_variables: list[TypeInstance]


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
class FunctionDef(ASTNode):
    assignees: list[TypedAssignee]
    return_type: TypeInstance
    body: Block


Expression: TypeAlias = Union[
    FunctionCall,
    Integer,
    Boolean,
    ElementAccess,
    GenericVariable,
    IfExpression,
    MatchExpression,
    TupleExpression,
    FunctionDef,
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


def TypeVariable(name: Id) -> GenericVariable:
    return GenericTypeVariable(name, [])
