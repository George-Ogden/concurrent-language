use from_variants::FromVariants;
use type_checker::{AtomicTypeEnum, Boolean, Integer};

pub type Name = String;
pub type Id = String;

#[derive(Debug, Clone, FromVariants, Hash, PartialEq, Eq)]
pub enum MachineType {
    AtomicType(AtomicType),
    TupleType(TupleType),
    FnType(FnType),
    UnionType(UnionType),
    NamedType(Name),
    Reference(Box<MachineType>),
    Lazy(Box<MachineType>),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct AtomicType(pub AtomicTypeEnum);

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TupleType(pub Vec<MachineType>);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct FnType(pub Vec<MachineType>, pub Box<MachineType>);
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct UnionType(pub Vec<Name>);

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: Name,
    pub constructors: Vec<(Name, Option<MachineType>)>,
}

#[derive(Debug, Clone, FromVariants)]
pub enum Value {
    BuiltIn(BuiltIn),
    Memory(Memory),
}

#[derive(Debug, Clone)]
pub struct Memory(pub Id);

#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub ret: (Value, MachineType),
}

#[derive(Debug, Clone, FromVariants)]
pub enum BuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(Name, MachineType),
}

#[derive(Debug, Clone, FromVariants)]
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

#[derive(Debug, Clone)]
pub struct ElementAccess {
    pub value: Value,
    pub idx: usize,
}

#[derive(Clone, Debug)]
pub struct TupleExpression(pub Vec<Value>);

#[derive(Clone, Debug)]
pub struct FnCall {
    pub fn_: Value,
    pub args: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct ConstructorCall {
    pub idx: usize,
    pub data: Option<(Name, Value)>,
}

#[derive(Clone, Debug)]
pub struct ClosureInstantiation {
    pub name: Name,
    pub env: Value,
}

#[derive(Clone, Debug, FromVariants)]
pub enum Statement {
    Await(Await),
    Assignment(Assignment),
    IfStatement(IfStatement),
    MatchStatement(MatchStatement),
}

#[derive(Clone, Debug)]
pub struct Await(pub Vec<Memory>);

#[derive(Clone, Debug)]
pub struct Assignment {
    pub allocation: Option<MachineType>,
    pub target: Memory,
    pub value: Expression,
}

#[derive(Clone, Debug)]
pub struct IfStatement {
    pub condition: Value,
    pub branches: (Vec<Statement>, Vec<Statement>),
}

#[derive(Clone, Debug)]
pub struct MatchStatement {
    pub expression: (Value, UnionType),
    pub branches: Vec<MatchBranch>,
}

#[derive(Clone, Debug)]
pub struct MatchBranch {
    pub target: Option<Name>,
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug)]
pub struct MemoryAllocation(pub Id, pub MachineType);

#[derive(Clone, Debug)]
pub struct FnDef {
    pub name: Name,
    pub arguments: Vec<(Memory, MachineType)>,
    pub statements: Vec<Statement>,
    pub ret: (Value, MachineType),
    pub env: Option<MachineType>,
    pub allocations: Vec<MemoryAllocation>,
}

pub struct Program {
    pub type_defs: Vec<TypeDef>,
    pub globals: Vec<MemoryAllocation>,
    pub fn_defs: Vec<FnDef>,
}
