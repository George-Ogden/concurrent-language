use std::collections::HashSet;

use from_variants::FromVariants;
use itertools::Itertools;
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

impl From<AtomicTypeEnum> for MachineType {
    fn from(value: AtomicTypeEnum) -> Self {
        AtomicType(value).into()
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicType(pub AtomicTypeEnum);

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct TupleType(pub Vec<MachineType>);
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct FnType(pub Vec<MachineType>, pub Box<MachineType>);
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnionType(pub Vec<Name>);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TypeDef {
    pub name: Name,
    pub constructors: Vec<(Name, Option<MachineType>)>,
}

#[derive(Clone, Debug, FromVariants, Serialize, Deserialize, PartialEq)]
pub enum Value {
    BuiltIn(BuiltIn),
    Memory(Memory),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Memory(pub Id);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub ret: (Value, MachineType),
}

#[derive(Clone, Debug, FromVariants, Serialize, Deserialize, PartialEq)]
pub enum BuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(Name, MachineType),
}

#[derive(Clone, Debug, FromVariants, Serialize, Deserialize, PartialEq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ElementAccess {
    pub value: Value,
    pub idx: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TupleExpression(pub Vec<Value>);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FnCall {
    pub fn_: Value,
    pub args: Vec<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConstructorCall {
    pub idx: usize,
    pub data: Option<(Name, Value)>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ClosureInstantiation {
    pub name: Name,
    pub env: Value,
}

#[derive(Clone, Debug, FromVariants, Serialize, Deserialize, PartialEq)]
pub enum Statement {
    Await(Await),
    Declaration(Declaration),
    Assignment(Assignment),
    IfStatement(IfStatement),
    MatchStatement(MatchStatement),
}

impl Statement {
    fn get_declarations(&self) -> HashSet<Declaration> {
        match self {
            Statement::Await(_) | Statement::Assignment(_) => HashSet::new(),
            Statement::Declaration(declaration) => HashSet::from([declaration.clone()]),
            Statement::IfStatement(IfStatement {
                condition: _,
                branches: (true_branch, false_branch),
            }) => true_branch
                .iter()
                .chain(false_branch.iter())
                .flat_map(|stmt| stmt.get_declarations())
                .collect(),
            Statement::MatchStatement(MatchStatement {
                expression: _,
                branches,
            }) => branches
                .iter()
                .flat_map(|branch| {
                    branch
                        .statements
                        .iter()
                        .flat_map(|statement| statement.get_declarations())
                })
                .collect(),
        }
    }
    fn maybe_remove_declaration(self, declarations: &HashSet<Memory>) -> Option<Self> {
        match self {
            Statement::Await(await_) => Some(await_.into()),
            Statement::Assignment(assignment) => Some(assignment.into()),
            Statement::Declaration(Declaration { type_: _, memory })
                if declarations.contains(&memory) =>
            {
                None
            }
            Statement::Declaration(Declaration { type_, memory }) => {
                Some(Declaration { type_, memory }.into())
            }
            Statement::IfStatement(IfStatement {
                condition,
                branches,
            }) => Some(
                IfStatement {
                    condition,
                    branches: (
                        Self::remove_declarations(branches.0, declarations),
                        Self::remove_declarations(branches.1, declarations),
                    ),
                }
                .into(),
            ),
            Statement::MatchStatement(MatchStatement {
                expression,
                branches,
            }) => Some(
                MatchStatement {
                    expression,
                    branches: branches
                        .into_iter()
                        .map(|MatchBranch { target, statements }| MatchBranch {
                            target,
                            statements: Self::remove_declarations(statements, declarations),
                        })
                        .collect_vec(),
                }
                .into(),
            ),
        }
    }
    pub fn declarations(statements: &Vec<Statement>) -> HashSet<Declaration> {
        statements
            .iter()
            .map(|statement| statement.get_declarations())
            .flatten()
            .collect()
    }
    pub fn remove_declarations(
        statements: Vec<Statement>,
        declarations: &HashSet<Memory>,
    ) -> Vec<Statement> {
        statements
            .into_iter()
            .filter_map(|statement| statement.maybe_remove_declaration(declarations))
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Await(pub Vec<Memory>);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Declaration {
    pub type_: MachineType,
    pub memory: Memory,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Assignment {
    pub target: Memory,
    pub value: Expression,
    pub check_null: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IfStatement {
    pub condition: Value,
    pub branches: (Vec<Statement>, Vec<Statement>),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MatchStatement {
    pub expression: (Value, UnionType),
    pub branches: Vec<MatchBranch>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MatchBranch {
    pub target: Option<Name>,
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MemoryAllocation(pub Id, pub MachineType);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
