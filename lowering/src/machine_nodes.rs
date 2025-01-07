use from_variants::FromVariants;
use type_checker::{AtomicTypeEnum, Boolean, Integer};

pub type Name = String;
pub type Id = String;

#[derive(Debug, Clone, FromVariants)]
pub enum MachineType {
    AtomicType(AtomicType),
    TupleType(TupleType),
    FnType(FnType),
    UnionType(UnionType),
    NamedType(Name),
    Reference(Box<MachineType>),
    Lazy(Box<MachineType>),
}

#[derive(Debug, Clone)]
pub struct AtomicType(pub AtomicTypeEnum);

#[derive(Debug, Clone)]
pub struct TupleType(pub Vec<MachineType>);
#[derive(Debug, Clone)]
pub struct FnType(pub Vec<MachineType>, pub Box<MachineType>);
#[derive(Debug, Clone)]
pub struct UnionType(pub Vec<Name>);

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: Name,
    pub constructors: Vec<(Name, Option<MachineType>)>,
}

#[derive(Debug, Clone, FromVariants)]
pub enum Value {
    BuiltIn(BuiltIn),
    Store(Store),
    Block(Block),
}

impl Value {
    pub fn type_(&self) -> MachineType {
        match self {
            Self::BuiltIn(BuiltIn::Integer(_)) => AtomicType(AtomicTypeEnum::INT).into(),
            Self::BuiltIn(BuiltIn::Boolean(_)) => AtomicType(AtomicTypeEnum::BOOL).into(),
            Self::BuiltIn(BuiltIn::BuiltInFn(_, type_)) => type_.clone(),
            Self::Store(store) => store.type_(),
            Self::Block(Block { statements: _, ret }) => {
                FnType(Vec::new(), Box::new(ret.type_())).into()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Store {
    Memory(Id, MachineType),
    Register(Id, MachineType),
}

#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub ret: Box<Value>,
}

impl Store {
    pub fn id(&self) -> Id {
        match &self {
            Self::Memory(id, _) | Self::Register(id, _) => id.clone(),
        }
    }
    pub fn type_(&self) -> MachineType {
        match &self {
            Self::Memory(_, type_) | Self::Register(_, type_) => type_.clone(),
        }
    }
}

#[derive(Debug, Clone, FromVariants)]
pub enum BuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(Name, MachineType),
}

#[derive(Debug, Clone, FromVariants)]
pub enum Expression {
    Value(Value),
    Wrap(Value),
    Unwrap(Store),
    Reference(Store),
    Dereference(Store),
    ElementAccess(ElementAccess),
    TupleExpression(TupleExpression),
    FnCall(FnCall),
    ConstructorCall(ConstructorCall),
}

#[derive(Debug, Clone)]
pub struct ElementAccess {
    pub value: Store,
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

#[derive(Clone, Debug, FromVariants)]
pub enum Statement {
    Await(Await),
    Assignment(Assignment),
    IfStatement(IfStatement),
    MatchStatement(MatchStatement),
}

#[derive(Clone, Debug)]
pub struct Await(pub Vec<Store>);

#[derive(Clone, Debug)]
pub struct Assignment {
    pub target: Store,
    pub value: Expression,
}

#[derive(Clone, Debug)]
pub struct IfStatement {
    pub condition: Store,
    pub branches: (Vec<Statement>, Vec<Statement>),
}

#[derive(Clone, Debug)]
pub struct MatchStatement {
    pub expression: Store,
    pub branches: Vec<MatchBranch>,
}

#[derive(Clone, Debug)]
pub struct MatchBranch {
    pub target: Option<Name>,
    pub statements: Vec<Statement>,
}
