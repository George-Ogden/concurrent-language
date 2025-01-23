use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use from_variants::FromVariants;
use lowering::{AtomicTypeEnum, Boolean, Integer};

pub type Name = String;
pub type Id = String;

#[derive(Clone, Debug, FromVariants, Hash, PartialEq, Eq)]
pub enum MachineType {
    AtomicType(AtomicType),
    TupleType(TupleType),
    FnType(FnType),
    UnionType(UnionType),
    NamedType(Name),
    Reference(Box<MachineType>),
    Lazy(Box<MachineType>),
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

#[derive(Clone, Debug, FromVariants, PartialEq)]
pub enum Value {
    BuiltIn(BuiltIn),
    Memory(Memory),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Memory(pub Id);

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub ret: (Value, MachineType),
}

#[derive(Clone, Debug, FromVariants, PartialEq)]
pub enum BuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(Name),
}

#[derive(Clone, Debug, FromVariants, PartialEq)]
pub enum Expression {
    Block(Block),
    Value(Value),
    Wrap(Value, MachineType),
    Unwrap(Value),
    ElementAccess(ElementAccess),
    TupleExpression(TupleExpression),
    FnCall(FnCall),
    ConstructorCall(ConstructorCall),
    ClosureInstantiation(ClosureInstantiation),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ElementAccess {
    pub value: Value,
    pub idx: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TupleExpression(pub Vec<Value>);

#[derive(Clone, Debug, PartialEq)]
pub struct FnCall {
    pub fn_: Value,
    pub fn_type: FnType,
    pub args: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorCall {
    pub idx: usize,
    pub data: Option<(Name, Value)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClosureInstantiation {
    pub name: Name,
    pub env: Option<Value>,
}

#[derive(Clone, Debug, FromVariants, PartialEq)]
pub enum Statement {
    Await(Await),
    Declaration(Declaration),
    Assignment(Assignment),
    IfStatement(IfStatement),
    MatchStatement(MatchStatement),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AllocationState {
    Undeclared(Option<MachineType>),
    Declared(MachineType),
}

impl PartialOrd for AllocationState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(match (self, other) {
            (AllocationState::Undeclared(_), AllocationState::Undeclared(_))
            | (AllocationState::Declared(_), AllocationState::Declared(_)) => Ordering::Equal,
            (AllocationState::Undeclared(_), AllocationState::Declared(_)) => Ordering::Greater,
            (AllocationState::Declared(_), AllocationState::Undeclared(_)) => Ordering::Less,
        })
    }
}

impl Ord for AllocationState {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

type Declarations = HashMap<Memory, AllocationState>;

impl Statement {
    fn merge_declarations_serial(
        mut declarations1: Declarations,
        declarations2: Declarations,
    ) -> Declarations {
        for (key, value2) in declarations2 {
            declarations1
                .entry(key)
                .and_modify(|value1| {
                    if value2.partial_cmp(value1) == Some(Ordering::Less) {
                        *value1 = value2.clone();
                    }
                })
                .or_insert(value2);
        }
        declarations1
    }
    pub fn merge_declarations_parallel(
        declarations1: Declarations,
        declarations2: Declarations,
    ) -> Declarations {
        let shared_memory = declarations1
            .keys()
            .filter(|k| declarations2.contains_key(k))
            .cloned()
            .collect::<HashSet<_>>();
        shared_memory
            .into_iter()
            .map(|memory| {
                (
                    memory.clone(),
                    match (&declarations1[&memory], &declarations2[&memory]) {
                        (AllocationState::Undeclared(t1), AllocationState::Undeclared(t2)) => {
                            AllocationState::Undeclared(t1.clone().or(t2.clone()))
                        }
                        (AllocationState::Undeclared(_), AllocationState::Declared(t))
                        | (AllocationState::Declared(t), AllocationState::Undeclared(_)) => {
                            AllocationState::Undeclared(Some(t.clone()))
                        }
                        (AllocationState::Declared(t1), AllocationState::Declared(_)) => {
                            AllocationState::Declared(t1.clone())
                        }
                    },
                )
            })
            .collect::<HashMap<_, _>>()
    }
    fn get_declarations(&self) -> Declarations {
        match self {
            Statement::Await(_) => HashMap::new(),
            Statement::Assignment(Assignment {
                target,
                value:
                    Expression::FnCall(FnCall {
                        fn_: _,
                        fn_type: FnType(_, r),
                        args: _,
                    }),
                check_null: _,
            }) => HashMap::from([(
                target.clone(),
                AllocationState::Undeclared(Some(*r.clone())),
            )]),
            Statement::Assignment(Assignment {
                target,
                value: _,
                check_null: _,
            }) => HashMap::from([(target.clone(), AllocationState::Undeclared(None))]),
            Statement::Declaration(Declaration { type_, memory }) => {
                HashMap::from([(memory.clone(), AllocationState::Declared(type_.clone()))])
            }
            Statement::IfStatement(IfStatement {
                condition: _,
                branches: (true_branch, false_branch),
            }) => Self::merge_declarations_serial(
                Self::declarations(true_branch),
                Self::declarations(false_branch),
            ),
            Statement::MatchStatement(MatchStatement {
                expression: _,
                branches,
            }) => {
                let mut declarations = HashMap::new();
                for branch in branches {
                    declarations = Self::merge_declarations_serial(
                        declarations,
                        Self::declarations(&branch.statements),
                    );
                }
                declarations
            }
        }
    }
    pub fn declarations(statements: &Vec<Statement>) -> Declarations {
        let mut declarations = HashMap::new();
        for statement in statements {
            declarations =
                Self::merge_declarations_serial(declarations, statement.get_declarations());
        }
        declarations
    }
    pub fn from_declarations(declarations: Declarations) -> Vec<Statement> {
        declarations
            .into_iter()
            .filter_map(|(memory, state)| match state {
                AllocationState::Undeclared(_) => None,
                AllocationState::Declared(type_) => Some(Declaration { memory, type_ }.into()),
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Await(pub Vec<Memory>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Declaration {
    pub type_: MachineType,
    pub memory: Memory,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub target: Memory,
    pub value: Expression,
    pub check_null: bool,
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchBranch {
    pub target: Option<Memory>,
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FnDef {
    pub name: Name,
    pub arguments: Vec<(Memory, MachineType)>,
    pub statements: Vec<Statement>,
    pub ret: (Value, MachineType),
    pub env: Option<MachineType>,
    pub allocations: Vec<Declaration>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub type_defs: Vec<TypeDef>,
    pub fn_defs: Vec<FnDef>,
}
