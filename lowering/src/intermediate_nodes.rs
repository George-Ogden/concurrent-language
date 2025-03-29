use core::fmt;
use itertools::Itertools;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use from_variants::FromVariants;
use type_checker::{AtomicTypeEnum, Boolean, Id, Integer};

use crate::type_equality_checker::TypeEqualityChecker;

#[derive(Clone, FromVariants, Eq)]
pub enum IntermediateType {
    AtomicType(AtomicType),
    IntermediateTupleType(IntermediateTupleType),
    IntermediateFnType(IntermediateFnType),
    IntermediateUnionType(IntermediateUnionType),
    Reference(Rc<RefCell<IntermediateType>>),
}

impl fmt::Debug for IntermediateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AtomicType(arg0) => f.debug_tuple("AtomicType").field(arg0).finish(),
            Self::IntermediateTupleType(arg0) => {
                f.debug_tuple("IntermediateTupleType").field(arg0).finish()
            }
            Self::IntermediateFnType(arg0) => {
                f.debug_tuple("IntermediateFnType").field(arg0).finish()
            }
            Self::IntermediateUnionType(arg0) => {
                f.debug_tuple("IntermediateUnionType").field(arg0).finish()
            }
            Self::Reference(r) => f.debug_tuple("Reference").field(&r.as_ptr()).finish(),
        }
    }
}

impl From<AtomicTypeEnum> for IntermediateType {
    fn from(value: AtomicTypeEnum) -> Self {
        Self::AtomicType(AtomicType(value))
    }
}

impl Hash for IntermediateType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

impl PartialEq for IntermediateType {
    fn eq(&self, other: &Self) -> bool {
        let mut equality_checker = TypeEqualityChecker::new();
        equality_checker.equal_type(self, other)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AtomicType(pub AtomicTypeEnum);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateTupleType(pub Vec<IntermediateType>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateFnType(pub Vec<IntermediateType>, pub Box<IntermediateType>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateUnionType(pub Vec<Option<IntermediateType>>);

static LOCATION_ID: AtomicUsize = AtomicUsize::new(0);
#[derive(Clone, Ord, Hash, Eq, PartialEq, PartialOrd)]
pub struct Location(usize);

impl Location {
    pub fn new() -> Self {
        Self(LOCATION_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl IntermediateValue {
    pub fn substitute(&self, substitution: &Substitution) -> IntermediateValue {
        match self {
            IntermediateValue::IntermediateBuiltIn(built_in) => built_in.clone().into(),
            IntermediateValue::IntermediateMemory(memory) => IntermediateMemory {
                type_: memory.type_.clone(),
                location: substitution
                    .get(&memory.location)
                    .unwrap_or(&memory.location)
                    .clone(),
            }
            .into(),
            IntermediateValue::IntermediateArg(arg) => match substitution.get(&arg.location) {
                None => arg.clone().into(),
                Some(location) => IntermediateMemory {
                    location: location.clone(),
                    type_: arg.type_.clone(),
                }
                .into(),
            },
        }
    }
    fn substitute_all(values: &mut Vec<Self>, substitution: &Substitution) {
        for value in values {
            *value = value.substitute(substitution);
        }
    }
    pub fn type_(&self) -> IntermediateType {
        match self {
            IntermediateValue::IntermediateBuiltIn(built_in) => built_in.type_(),
            IntermediateValue::IntermediateMemory(memory) => memory.type_(),
            IntermediateValue::IntermediateArg(arg) => arg.type_(),
        }
    }
    pub fn location(&self) -> Option<Location> {
        match self {
            IntermediateValue::IntermediateBuiltIn(_) => None,
            IntermediateValue::IntermediateMemory(memory) => Some(memory.location.clone()),
            IntermediateValue::IntermediateArg(arg) => Some(arg.location.clone()),
        }
    }
    fn types(values: &Vec<Self>) -> Vec<IntermediateType> {
        values.iter().map(Self::type_).collect()
    }
    pub fn filter_memory_location(&self) -> Option<Location> {
        if let IntermediateValue::IntermediateMemory(IntermediateMemory { type_: _, location }) =
            self
        {
            Some(location.clone())
        } else {
            None
        }
    }
}

impl From<Integer> for IntermediateValue {
    fn from(value: Integer) -> IntermediateValue {
        IntermediateBuiltIn::from(value).into()
    }
}

impl From<Boolean> for IntermediateValue {
    fn from(value: Boolean) -> IntermediateValue {
        IntermediateBuiltIn::from(value).into()
    }
}

impl From<BuiltInFn> for IntermediateValue {
    fn from(value: BuiltInFn) -> IntermediateValue {
        IntermediateBuiltIn::from(value).into()
    }
}

impl From<IntermediateAssignment> for IntermediateValue {
    fn from(value: IntermediateAssignment) -> IntermediateValue {
        IntermediateMemory {
            location: value.location,
            type_: value.expression.type_(),
        }
        .into()
    }
}

#[derive(Clone, FromVariants, PartialEq, Eq, Hash)]
pub enum IntermediateBuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(BuiltInFn),
}

impl IntermediateBuiltIn {
    pub fn type_(&self) -> IntermediateType {
        match self {
            IntermediateBuiltIn::Integer(_) => AtomicTypeEnum::INT.into(),
            IntermediateBuiltIn::Boolean(_) => AtomicTypeEnum::BOOL.into(),
            IntermediateBuiltIn::BuiltInFn(BuiltInFn(_, type_)) => type_.clone().into(),
        }
    }
}

impl fmt::Debug for IntermediateBuiltIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integer(Integer { value }) => f.debug_tuple("Integer").field(value).finish(),
            Self::Boolean(Boolean { value }) => f.debug_tuple("Boolean").field(value).finish(),
            Self::BuiltInFn(BuiltInFn(name, _)) => f.debug_tuple("BuiltInFn").field(name).finish(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BuiltInFn(pub Id, pub IntermediateFnType);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateAssignment {
    pub expression: IntermediateExpression,
    pub location: Location,
}

impl From<IntermediateExpression> for IntermediateAssignment {
    fn from(value: IntermediateExpression) -> Self {
        IntermediateAssignment {
            expression: value,
            location: Location::new(),
        }
    }
}

impl From<IntermediateValue> for IntermediateAssignment {
    fn from(value: IntermediateValue) -> Self {
        IntermediateAssignment {
            expression: value.into(),
            location: Location::new(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, FromVariants, Hash)]
pub enum IntermediateExpression {
    IntermediateValue(IntermediateValue),
    IntermediateElementAccess(IntermediateElementAccess),
    IntermediateTupleExpression(IntermediateTupleExpression),
    IntermediateFnCall(IntermediateFnCall),
    IntermediateCtorCall(IntermediateCtorCall),
    IntermediateLambda(IntermediateLambda),
    IntermediateIf(IntermediateIf),
    IntermediateMatch(IntermediateMatch),
}

impl fmt::Debug for IntermediateExpression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntermediateValue(value) => value.fmt(f),
            Self::IntermediateElementAccess(element_access) => element_access.fmt(f),
            Self::IntermediateTupleExpression(tuple) => tuple.fmt(f),
            Self::IntermediateFnCall(fn_call) => fn_call.fmt(f),
            Self::IntermediateCtorCall(ctor_call) => ctor_call.fmt(f),
            Self::IntermediateLambda(lambda) => lambda.fmt(f),
            Self::IntermediateIf(if_) => if_.fmt(f),
            Self::IntermediateMatch(match_) => match_.fmt(f),
        }
    }
}

impl IntermediateExpression {
    pub fn targets(&self) -> Vec<Location> {
        match self {
            IntermediateExpression::IntermediateLambda(IntermediateLambda { block, args: _ }) => {
                block.targets()
            }
            IntermediateExpression::IntermediateIf(IntermediateIf {
                condition: _,
                branches,
            }) => {
                let mut targets = branches.0.targets();
                targets.extend(branches.1.targets());
                targets
            }
            IntermediateExpression::IntermediateMatch(IntermediateMatch {
                subject: _,
                branches,
            }) => branches
                .iter()
                .flat_map(|branch| branch.block.targets())
                .collect(),
            _ => Vec::new(),
        }
    }
    pub fn values(&self) -> Vec<IntermediateValue> {
        match self {
            IntermediateExpression::IntermediateValue(value) => vec![value.clone()],
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx: _,
            }) => vec![value.clone()],
            IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                values,
            )) => values.clone(),
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args }) => {
                let mut values = args.clone();
                values.push(fn_.clone());
                values
            }
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                idx: _,
                data,
                type_: _,
            }) => match data {
                None => Vec::new(),
                Some(v) => vec![v.clone()],
            },
            IntermediateExpression::IntermediateLambda(lambda) => lambda.find_open_vars(),
            IntermediateExpression::IntermediateIf(IntermediateIf {
                condition,
                branches,
            }) => {
                let mut values = branches.0.values();
                values.extend(branches.1.values());
                values.push(condition.clone());
                values
            }
            IntermediateExpression::IntermediateMatch(IntermediateMatch { subject, branches }) => {
                let mut values = branches
                    .iter()
                    .flat_map(|IntermediateMatchBranch { target, block }| {
                        block.values().into_iter().filter(|value| match value {
                            IntermediateValue::IntermediateArg(arg) => Some(arg) != target.as_ref(),
                            _ => true,
                        })
                    })
                    .collect_vec();
                values.push(subject.clone());
                values
            }
        }
    }
    pub fn substitute(&mut self, substitution: &Substitution) {
        match self {
            IntermediateExpression::IntermediateValue(value) => {
                *value = value.substitute(substitution);
            }
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx: _,
            }) => {
                *value = value.substitute(substitution);
            }
            IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                values,
            )) => {
                IntermediateValue::substitute_all(values, substitution);
            }
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args }) => {
                *fn_ = fn_.substitute(substitution);
                IntermediateValue::substitute_all(args, substitution)
            }
            IntermediateExpression::IntermediateIf(IntermediateIf {
                condition,
                branches,
            }) => {
                *condition = condition.substitute(substitution);
                branches.0.substitute(substitution);
                branches.1.substitute(substitution);
            }
            IntermediateExpression::IntermediateMatch(IntermediateMatch { subject, branches }) => {
                *subject = subject.substitute(substitution);
                for branch in branches {
                    branch.block.substitute(substitution);
                }
            }
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                idx: _,
                data,
                type_: _,
            }) => match data {
                None => (),
                Some(data) => *data = data.substitute(substitution),
            },
            IntermediateExpression::IntermediateLambda(lambda) => lambda.substitute(substitution),
        }
    }
    pub fn type_(&self) -> IntermediateType {
        match self {
            IntermediateExpression::IntermediateValue(value) => value.type_(),
            IntermediateExpression::IntermediateElementAccess(element) => element.type_(),
            IntermediateExpression::IntermediateTupleExpression(tuple) => tuple.type_().into(),
            IntermediateExpression::IntermediateFnCall(fn_call) => fn_call.type_(),
            IntermediateExpression::IntermediateCtorCall(ctor_call) => ctor_call.type_().into(),
            IntermediateExpression::IntermediateLambda(lambda) => lambda.type_().into(),
            IntermediateExpression::IntermediateIf(if_) => if_.type_().into(),
            IntermediateExpression::IntermediateMatch(match_) => match_.type_().into(),
        }
    }
}

impl From<IntermediateArg> for IntermediateExpression {
    fn from(value: IntermediateArg) -> Self {
        IntermediateExpression::IntermediateValue(value.into())
    }
}

impl From<IntermediateBuiltIn> for IntermediateExpression {
    fn from(value: IntermediateBuiltIn) -> Self {
        IntermediateExpression::IntermediateValue(value.into())
    }
}

#[derive(Clone, FromVariants, PartialEq, Eq, Debug, Hash)]
pub enum IntermediateValue {
    IntermediateBuiltIn(IntermediateBuiltIn),
    IntermediateMemory(IntermediateMemory),
    IntermediateArg(IntermediateArg),
}

#[derive(Clone, Eq)]
pub struct IntermediateMemory {
    pub type_: IntermediateType,
    pub location: Location,
}

impl fmt::Debug for IntermediateMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Memory").field(&self.location).finish()
    }
}

impl IntermediateMemory {
    pub fn type_(&self) -> IntermediateType {
        self.type_.clone()
    }
}

impl PartialEq for IntermediateMemory {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location
    }
}

impl Hash for IntermediateMemory {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.location.hash(state);
    }
}

impl From<IntermediateType> for IntermediateMemory {
    fn from(value: IntermediateType) -> Self {
        IntermediateMemory {
            type_: value,
            location: Location::new(),
        }
    }
}

#[derive(Clone, Eq)]
pub struct IntermediateArg {
    pub type_: IntermediateType,
    pub location: Location,
}

impl fmt::Debug for IntermediateArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Arg").field(&self.location).finish()
    }
}

impl IntermediateArg {
    pub fn type_(&self) -> IntermediateType {
        self.type_.clone()
    }
}

impl PartialEq for IntermediateArg {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location
    }
}

impl From<IntermediateType> for IntermediateArg {
    fn from(value: IntermediateType) -> Self {
        IntermediateArg {
            type_: value,
            location: Location::new(),
        }
    }
}

impl Hash for IntermediateArg {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.location.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateElementAccess {
    pub value: IntermediateValue,
    pub idx: usize,
}

impl IntermediateElementAccess {
    pub fn type_(&self) -> IntermediateType {
        let IntermediateType::IntermediateTupleType(IntermediateTupleType(types)) =
            self.value.type_()
        else {
            panic!("Accessing non-tuple");
        };
        types[self.idx].clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateTupleExpression(pub Vec<IntermediateValue>);

impl IntermediateTupleExpression {
    pub fn type_(&self) -> IntermediateTupleType {
        IntermediateTupleType(IntermediateValue::types(&self.0))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateFnCall {
    pub fn_: IntermediateValue,
    pub args: Vec<IntermediateValue>,
}

impl IntermediateFnCall {
    pub fn type_(&self) -> IntermediateType {
        let IntermediateType::IntermediateFnType(IntermediateFnType(_, ret)) = self.fn_.type_()
        else {
            panic!("Calling non-function")
        };
        *ret
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateCtorCall {
    pub idx: usize,
    pub data: Option<IntermediateValue>,
    pub type_: IntermediateUnionType,
}

impl IntermediateCtorCall {
    pub fn type_(&self) -> IntermediateUnionType {
        self.type_.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateBlock {
    pub statements: Vec<IntermediateStatement>,
    pub ret: IntermediateValue,
}

impl IntermediateBlock {
    fn targets(&self) -> Vec<Location> {
        IntermediateStatement::all_targets(&self.statements)
    }

    pub fn values(&self) -> Vec<IntermediateValue> {
        let mut values = IntermediateStatement::all_values(&self.statements);
        values.push(self.ret.clone());
        values
    }
    fn substitute(&mut self, substitution: &Substitution) {
        IntermediateStatement::substitute_all(&mut self.statements, substitution);
        self.ret = self.ret.substitute(substitution);
    }
    pub fn type_(&self) -> IntermediateType {
        self.ret.type_()
    }
}

impl From<IntermediateValue> for IntermediateBlock {
    fn from(value: IntermediateValue) -> Self {
        IntermediateBlock {
            statements: Vec::new(),
            ret: value,
        }
    }
}

impl From<(Vec<IntermediateStatement>, IntermediateValue)> for IntermediateBlock {
    fn from(value: (Vec<IntermediateStatement>, IntermediateValue)) -> Self {
        IntermediateBlock {
            statements: value.0,
            ret: value.1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateLambda {
    pub args: Vec<IntermediateArg>,
    pub block: IntermediateBlock,
}

/// Typealias for a location substitution.
type Substitution = HashMap<Location, Location>;

impl IntermediateLambda {
    pub fn type_(&self) -> IntermediateFnType {
        IntermediateFnType(
            IntermediateValue::types(
                &self
                    .args
                    .iter()
                    .cloned()
                    .map(IntermediateValue::from)
                    .collect(),
            ),
            Box::new(self.block.type_()),
        )
    }

    pub fn find_open_vars(&self) -> Vec<IntermediateValue> {
        let mut targets: HashSet<Location> = HashSet::from_iter(self.block.targets());
        targets.extend(self.args.iter().map(|arg| arg.location.clone()));
        let values = self.block.values();
        values
            .into_iter()
            .unique()
            .filter_map(|value| match value {
                IntermediateValue::IntermediateBuiltIn(_) => None,
                IntermediateValue::IntermediateMemory(memory) => {
                    if !targets.contains(&memory.location) {
                        Some(memory.into())
                    } else {
                        None
                    }
                }
                IntermediateValue::IntermediateArg(arg) => {
                    if !targets.contains(&arg.location) {
                        Some(arg.into())
                    } else {
                        None
                    }
                }
            })
            .collect()
    }
    pub fn substitute(&mut self, substitution: &Substitution) {
        self.block.substitute(substitution)
    }
}

#[derive(Clone, PartialEq, FromVariants, Eq, Hash)]
pub enum IntermediateStatement {
    IntermediateAssignment(IntermediateAssignment),
}

impl fmt::Debug for IntermediateStatement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntermediateAssignment(assignment) => assignment.fmt(f),
        }
    }
}

impl IntermediateStatement {
    /// Find all values used in a statement.
    pub fn values(&self) -> Vec<IntermediateValue> {
        match self {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location: _,
            }) => expression.values(),
        }
    }
    /// Find all values used in multiple statements.
    fn all_values(statements: &Vec<Self>) -> Vec<IntermediateValue> {
        statements
            .iter()
            .flat_map(|statement| statement.values())
            .collect()
    }
    /// Find all targets assigned to in a statement.
    fn targets(&self) -> Vec<Location> {
        match self {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location,
            }) => {
                let mut targets = expression.targets();
                targets.push(location.clone());
                targets
            }
        }
    }
    /// Find all targets assigned to in multiple statements.
    pub fn all_targets(statements: &Vec<Self>) -> Vec<Location> {
        statements
            .iter()
            .flat_map(|statement| statement.targets())
            .collect()
    }
    /// Perform a substitution on a statement.
    fn substitute(&mut self, substitution: &Substitution) {
        match self {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location,
            }) => {
                if let Some(new_location) = substitution.get(location) {
                    *location = new_location.clone();
                }
                expression.substitute(substitution);
            }
        }
    }
    /// Perform a substitution on multiple statements.
    fn substitute_all(statements: &mut Vec<Self>, substitution: &Substitution) {
        for statement in statements {
            statement.substitute(substitution)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateIf {
    pub condition: IntermediateValue,
    pub branches: (IntermediateBlock, IntermediateBlock),
}

impl IntermediateIf {
    pub fn type_(&self) -> IntermediateType {
        self.branches.0.type_()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateMatch {
    pub subject: IntermediateValue,
    pub branches: Vec<IntermediateMatchBranch>,
}

impl IntermediateMatch {
    pub fn type_(&self) -> IntermediateType {
        self.branches[0].block.type_()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateMatchBranch {
    pub target: Option<IntermediateArg>,
    pub block: IntermediateBlock,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntermediateProgram {
    pub main: IntermediateLambda,
    pub types: Vec<Rc<RefCell<IntermediateType>>>,
}
