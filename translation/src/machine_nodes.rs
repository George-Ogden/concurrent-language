use from_variants::FromVariants;
use itertools::Itertools;
use lowering::{AtomicTypeEnum, Boolean, Integer};

pub type Name = String;
pub type Id = String;

#[derive(Clone, Debug, FromVariants, Hash, PartialEq, Eq)]
pub enum MachineType {
    AtomicType(AtomicType),
    TupleType(TupleType),
    FnType(FnType),
    WeakFnType(FnType),
    UnionType(UnionType),
    NamedType(Name),
}

impl From<AtomicTypeEnum> for MachineType {
    fn from(value: AtomicTypeEnum) -> Self {
        AtomicType(value).into()
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AtomicType(pub AtomicTypeEnum);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TupleType(pub Vec<MachineType>);
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct FnType(pub Vec<MachineType>, pub Box<MachineType>);
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct UnionType(pub Vec<Name>);

#[derive(Clone, Debug, PartialEq)]
pub struct TypeDef {
    pub name: Name,
    pub constructors: Vec<(Name, Option<MachineType>)>,
}

impl TypeDef {
    /// Find the names of all types that are used in all definitions.
    pub fn directly_used_types(&self) -> Vec<Name> {
        self.constructors
            .iter()
            .flat_map(|(_, type_)| match type_ {
                None => Vec::new(),
                Some(type_) => self.used_types(type_),
            })
            .collect_vec()
    }
    /// Find the names types that are used in a specific type.
    fn used_types(&self, type_: &MachineType) -> Vec<Name> {
        match type_ {
            MachineType::AtomicType(_) => Vec::new(),
            MachineType::TupleType(TupleType(types)) => self.all_used_types(types),
            MachineType::FnType(FnType(args, ret)) | MachineType::WeakFnType(FnType(args, ret)) => {
                let mut types = self.all_used_types(args);
                types.extend(self.used_types(&*ret));
                types
            }
            MachineType::UnionType(UnionType(names)) => names.clone(),
            MachineType::NamedType(name) => vec![name.clone()],
        }
    }
    fn all_used_types(&self, types: &Vec<MachineType>) -> Vec<Name> {
        types
            .iter()
            .flat_map(|type_| self.used_types(type_))
            .collect_vec()
    }
}

#[derive(Clone, Debug, FromVariants, PartialEq, Eq)]
pub enum Value {
    BuiltIn(BuiltIn),
    Memory(Memory),
}

impl Value {
    pub fn filter_memory(&self) -> Option<Memory> {
        match self {
            Value::BuiltIn(_) => None,
            Value::Memory(memory) => Some(memory.clone()),
        }
    }
}

impl From<Integer> for Value {
    fn from(value: Integer) -> Self {
        BuiltIn::from(value).into()
    }
}

impl From<Boolean> for Value {
    fn from(value: Boolean) -> Self {
        BuiltIn::from(value).into()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Memory(pub Id);

#[derive(Clone, Debug, FromVariants, PartialEq, Eq)]
pub enum BuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(Name),
}

#[derive(Clone, Debug, FromVariants, PartialEq, Eq)]
pub enum Expression {
    Value(Value),
    ElementAccess(ElementAccess),
    TupleExpression(TupleExpression),
    FnCall(FnCall),
    ConstructorCall(ConstructorCall),
    ClosureInstantiation(ClosureInstantiation),
}

impl Expression {
    pub fn values(&self) -> Vec<Value> {
        match self {
            Expression::Value(value) => vec![value.clone()],
            Expression::ElementAccess(ElementAccess { value, idx: _ }) => vec![value.clone()],
            Expression::TupleExpression(TupleExpression(values)) => values.clone(),
            Expression::FnCall(FnCall {
                fn_,
                fn_type: _,
                args,
            }) => {
                let mut values = vec![fn_.clone()];
                values.extend(args.clone());
                values
            }
            Expression::ConstructorCall(ConstructorCall {
                type_: _,
                idx: _,
                data,
            }) => data
                .as_ref()
                .map(|(_, value)| vec![value.clone()])
                .unwrap_or_default(),
            Expression::ClosureInstantiation(ClosureInstantiation { name: _, env }) => env
                .as_ref()
                .map(|value| vec![value.clone()])
                .unwrap_or_default(),
        }
    }
}

impl From<Memory> for Expression {
    fn from(value: Memory) -> Self {
        Value::from(value).into()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementAccess {
    pub value: Value,
    pub idx: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TupleExpression(pub Vec<Value>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnCall {
    pub fn_: Value,
    pub fn_type: FnType,
    pub args: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstructorCall {
    pub type_: Name,
    pub idx: usize,
    pub data: Option<(Name, Value)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureInstantiation {
    pub name: Name,
    pub env: Option<Value>,
}

#[derive(Clone, Debug, FromVariants, PartialEq)]
pub enum Statement {
    Await(Await),
    Declaration(Declaration),
    Allocation(Allocation),
    Assignment(Assignment),
    IfStatement(IfStatement),
    MatchStatement(MatchStatement),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Await(pub Vec<Memory>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Declaration {
    pub type_: MachineType,
    pub memory: Memory,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Allocation {
    pub name: Name,
    pub fns: Vec<(Memory, Name)>,
    pub target: Memory,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub target: Memory,
    pub value: Expression,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfStatement {
    pub condition: Value,
    pub branches: (Vec<Statement>, Vec<Statement>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchStatement {
    pub expression: (Value, UnionType),
    pub branches: Vec<MatchBranch>,
    pub auxiliary_memory: Memory,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchBranch {
    pub target: Option<Memory>,
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug)]
pub struct FnDef {
    pub name: Name,
    pub arguments: Vec<(Memory, MachineType)>,
    pub statements: Vec<Statement>,
    pub ret: (Value, MachineType),
    pub env: Vec<MachineType>,
    pub is_recursive: bool,
    pub size_bounds: (usize, usize),
}

impl PartialEq for FnDef {
    fn eq(&self, other: &Self) -> bool {
        // Don't require that code sizes are equal (for easier testing).
        self.name == other.name
            && self.arguments == other.arguments
            && self.statements == other.statements
            && self.ret == other.ret
            && self.env == other.env
            && self.is_recursive == other.is_recursive
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub type_defs: Vec<TypeDef>,
    pub fn_defs: Vec<FnDef>,
}
