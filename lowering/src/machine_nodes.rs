use from_variants::FromVariants;
use serde::{Deserialize, Serialize};
use type_checker::{AtomicTypeEnum, Boolean, Integer};

pub type Name = String;
pub type Id = String;

#[derive(Clone, Debug, FromVariants, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum MachineType {
    AtomicType(AtomicType),
    TupleType(TupleType),
    FnType(FnType),
    UnionType(UnionType),
    NamedType(Name),
    Reference(Box<MachineType>),
    Lazy(Box<MachineType>),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicType(pub AtomicTypeEnum);

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct TupleType(pub Vec<MachineType>);
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct FnType(pub Vec<MachineType>, pub Box<MachineType>);
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnionType(pub Vec<Name>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypeDef {
    pub name: Name,
    pub constructors: Vec<(Name, Option<MachineType>)>,
}

#[derive(Clone, Debug, FromVariants, Serialize, Deserialize)]
pub enum Value {
    BuiltIn(BuiltIn),
    Memory(Memory),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Memory(pub Id);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub ret: (Value, MachineType),
}

#[derive(Clone, Debug, FromVariants, Serialize, Deserialize)]
pub enum BuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(Name, MachineType),
}

#[derive(Clone, Debug, FromVariants, Serialize, Deserialize)]
pub enum Expression {
    Block(Block),
    Value(Value),
    Wrap(Value, MachineType),
    Unwrap(Value),
    Reference(Value, MachineType),
    Dereference(Value),
    ElementAccess(ElementAccess),
    TupleExpression(TupleExpression),
    FnCall(FnCall),
    ConstructorCall(ConstructorCall),
    ClosureInstantiation(ClosureInstantiation),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ElementAccess {
    pub value: Value,
    pub idx: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TupleExpression(pub Vec<Value>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FnCall {
    pub fn_: Value,
    pub args: Vec<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConstructorCall {
    pub idx: usize,
    pub data: Option<(Name, Value)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClosureInstantiation {
    pub name: Name,
    pub env: Value,
}

#[derive(Clone, Debug, FromVariants, Serialize, Deserialize)]
pub enum Statement {
    Await(Await),
    Assignment(Assignment),
    IfStatement(IfStatement),
    MatchStatement(MatchStatement),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Await(pub Vec<Memory>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Assignment {
    pub allocation: Option<MachineType>,
    pub target: Memory,
    pub value: Expression,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IfStatement {
    pub condition: Value,
    pub branches: (Vec<Statement>, Vec<Statement>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchStatement {
    pub expression: (Value, UnionType),
    pub branches: Vec<MatchBranch>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchBranch {
    pub target: Option<Name>,
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryAllocation(pub Id, pub MachineType);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FnDef {
    pub name: Name,
    pub arguments: Vec<(Memory, MachineType)>,
    pub statements: Vec<Statement>,
    pub ret: (Value, MachineType),
    pub env: Option<MachineType>,
    pub allocations: Vec<MemoryAllocation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Program {
    pub type_defs: Vec<TypeDef>,
    pub globals: Vec<MemoryAllocation>,
    pub fn_defs: Vec<FnDef>,
}
