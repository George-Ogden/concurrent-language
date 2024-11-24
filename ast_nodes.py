from __future__ import annotations

from dataclasses import dataclass
from typing import TypeAlias, Union


class ASTNode: ...


Id: TypeAlias = str


@dataclass
class Import(ASTNode): ...


@dataclass
class FnType(ASTNode):
    arg_types: list[TypeInstance]
    return_type: TypeInstance


@dataclass
class GenericType(ASTNode):
    type_variables: list[Id]
    type: TypeInstance


@dataclass
class TupleType(ASTNode):
    types: list[TypeInstance]


@dataclass
class NamedType(ASTNode):
    name: Id


TypeInstance: TypeAlias = Union[FnType, GenericType, TupleType, NamedType]


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


@dataclass
class TypedAssignee(ASTNode):
    assignee: Assignee
    type: TypeInstance


@dataclass
class FnCall(ASTNode):
    fn: Id
    type_variables: list[TypeInstance]
    args: list[Expression]


@dataclass
class Integer(ASTNode):
    value: int


@dataclass
class ElementAccess(ASTNode):
    expression: Expression
    index: int


@dataclass
class Variable(ASTNode):
    name: Id


@dataclass
class IfExpression:
    condition: Expression
    true_block: Block
    false_block: Block


@dataclass
class MatchBlock(ASTNode):
    matches: list[tuple[Id, Assignee]]
    block: Block


@dataclass
class MatchExpression:
    condition: Expression
    blocks: list[MatchBlock]


@dataclass
class TupleExpression(ASTNode):
    expressions: list[Expression]


@dataclass
class FnDef(ASTNode):
    assignees: list[TypedAssignee]
    return_type: TypeInstance
    body: Block


Expression: TypeAlias = Union[
    FnCall, Integer, ElementAccess, Variable, IfExpression, MatchExpression, TupleExpression, FnDef
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
class TransparentTypeDefinition(ASTNode):
    id: Id
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
