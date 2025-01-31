use core::fmt;
use itertools::Itertools;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
};

use from_variants::FromVariants;
use type_checker::{AtomicTypeEnum, Boolean, Id, Integer};

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

struct TypeEqualityChecker {
    equal_references: HashMap<*mut IntermediateType, *mut IntermediateType>,
}

impl TypeEqualityChecker {
    fn new() -> Self {
        TypeEqualityChecker {
            equal_references: HashMap::new(),
        }
    }
    fn equal_type(&mut self, t1: &IntermediateType, t2: &IntermediateType) -> bool {
        match (t1, t2) {
            (IntermediateType::AtomicType(a1), IntermediateType::AtomicType(a2)) => a1 == a2,
            (
                IntermediateType::IntermediateTupleType(IntermediateTupleType(t1)),
                IntermediateType::IntermediateTupleType(IntermediateTupleType(t2)),
            ) => self.equal_types(t1, t2),
            (
                IntermediateType::IntermediateFnType(IntermediateFnType(a1, r1)),
                IntermediateType::IntermediateFnType(IntermediateFnType(a2, r2)),
            ) => self.equal_types(a1, a2) && self.equal_type(r1, r2),
            (
                IntermediateType::IntermediateUnionType(IntermediateUnionType(t1)),
                IntermediateType::IntermediateUnionType(IntermediateUnionType(t2)),
            ) => {
                t1.len() == t2.len()
                    && t1.iter().zip_eq(t2.iter()).all(|(t1, t2)| match (t1, t2) {
                        (None, None) => true,
                        (Some(t1), Some(t2)) => self.equal_type(t1, t2),
                        _ => false,
                    })
            }
            (IntermediateType::Reference(r1), IntermediateType::Reference(r2)) => {
                let p1 = r1.as_ptr();
                let p2 = r2.as_ptr();
                if self.equal_references.get(&p1) == Some(&p2) {
                    true
                } else if matches!(self.equal_references.get(&p1), Some(_))
                    || matches!(self.equal_references.get(&p2), Some(_))
                {
                    false
                } else {
                    self.equal_references.insert(p1, p2);
                    self.equal_references.insert(p2, p1);
                    self.equal_type(&r1.borrow().clone(), &r2.borrow().clone())
                }
            }
            _ => false,
        }
    }
    fn equal_types(&mut self, t1: &Vec<IntermediateType>, t2: &Vec<IntermediateType>) -> bool {
        t1.len() == t2.len()
            && t1
                .iter()
                .zip_eq(t2.iter())
                .all(|(t1, t2)| self.equal_type(t1, t2))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateTupleType(pub Vec<IntermediateType>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateFnType(pub Vec<IntermediateType>, pub Box<IntermediateType>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateUnionType(pub Vec<Option<IntermediateType>>);

#[derive(Clone, Eq)]
pub struct Location(Rc<RefCell<()>>);

impl Location {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(())))
    }
}

impl PartialEq for Location {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ptr() == other.0.as_ptr()
    }
}

impl PartialOrd for Location {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Location {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.as_ptr().cmp(&other.0.as_ptr())
    }
}

impl Hash for Location {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self.0.as_ptr())
    }
}

type IntermediateMemory = Location;

#[derive(Clone, FromVariants, PartialEq, Eq, Debug, Hash)]
pub enum IntermediateValue {
    IntermediateBuiltIn(IntermediateBuiltIn),
    IntermediateMemory(IntermediateMemory),
    IntermediateArg(IntermediateArg),
}

impl IntermediateValue {
    fn substitute(&self, substitution: &Substitution) -> IntermediateValue {
        substitution.get(&self).unwrap_or(&self).clone()
    }
    fn substitute_all(values: &mut Vec<Self>, substitution: &Substitution) {
        for value in values {
            *value = value.substitute(substitution);
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

#[derive(Clone, Debug, FromVariants, PartialEq, Eq, Hash)]
pub enum IntermediateBuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(BuiltInFn),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BuiltInFn(pub Id, pub IntermediateType);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateFnDef(pub IntermediateValue);

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

#[derive(Clone, PartialEq, Eq, FromVariants, Hash, Debug)]
pub enum IntermediateExpression {
    IntermediateValue(IntermediateValue),
    IntermediateElementAccess(IntermediateElementAccess),
    IntermediateTupleExpression(IntermediateTupleExpression),
    IntermediateFnCall(IntermediateFnCall),
    IntermediateCtorCall(IntermediateCtorCall),
    IntermediateLambda(IntermediateLambda),
}

impl IntermediateExpression {
    pub fn targets(&self) -> Vec<Location> {
        match self {
            IntermediateExpression::IntermediateLambda(IntermediateLambda {
                statements,
                args: _,
                ret: _,
            }) => IntermediateStatement::all_targets(statements),
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
            IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args,
                statements,
                ret: (return_value, _),
            }) => {
                let mut values: Vec<_> = IntermediateStatement::all_values(statements);
                values.push(return_value.clone());
                values
                    .into_iter()
                    .filter(|value| match value {
                        IntermediateValue::IntermediateArg(arg) => !args.contains(&arg),
                        _ => true,
                    })
                    .collect()
            }
        }
    }
    fn substitute(&mut self, substitution: &Substitution) {
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
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                idx: _,
                data,
                type_: _,
            }) => match data {
                None => (),
                Some(data) => *data = data.substitute(substitution),
            },
            IntermediateExpression::IntermediateLambda(fn_def) => fn_def.substitute(substitution),
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

pub struct ExpressionEqualityChecker {
    true_history: HashMap<Location, Location>,
    history: HashMap<Location, Location>,
}

impl ExpressionEqualityChecker {
    pub fn equal(e1: &IntermediateExpression, e2: &IntermediateExpression) -> bool {
        let mut expression_equality_checker = Self::new();
        expression_equality_checker.equal_expression(e1, e2)
    }
    fn new() -> Self {
        ExpressionEqualityChecker {
            true_history: HashMap::new(),
            history: HashMap::new(),
        }
    }
    fn equal_arg(&mut self, a1: &IntermediateArg, a2: &IntermediateArg) -> bool {
        let IntermediateArg {
            location: l1,
            type_: _,
        } = a1;
        let IntermediateArg {
            location: l2,
            type_: _,
        } = a2;
        self.equal_locations(l1, l2)
    }
    fn equal_args(&mut self, a1: &Vec<IntermediateArg>, a2: &Vec<IntermediateArg>) -> bool {
        a1.len() == a2.len()
            && a1
                .iter()
                .zip(a2.iter())
                .all(|(a1, a2)| self.equal_arg(a1, a2))
    }
    fn equal_locations(&mut self, l1: &Location, l2: &Location) -> bool {
        if self.history.get(&l1) == Some(&l2) {
            true
        } else if matches!(self.history.get(&l1), Some(_))
            || matches!(self.history.get(&l2), Some(_))
        {
            false
        } else {
            self.history.insert(l1.clone(), l2.clone());
            self.history.insert(l2.clone(), l1.clone());
            true
        }
    }
    fn equal_assignment(
        &mut self,
        m1: &IntermediateAssignment,
        m2: &IntermediateAssignment,
    ) -> bool {
        let IntermediateAssignment {
            expression: e1,
            location: l1,
        } = m1;
        let IntermediateAssignment {
            expression: e2,
            location: l2,
        } = m2;
        if self.true_history.get(&l1) == Some(&l2) {
            true
        } else if self.history.get(&l1) == Some(&l2) {
            self.true_history.insert(l1.clone(), l2.clone());
            self.true_history.insert(l2.clone(), l1.clone());
            self.equal_expression(&e1, &e2)
        } else if matches!(self.true_history.get(&l1), Some(_))
            || matches!(self.true_history.get(&l2), Some(_))
            || matches!(self.history.get(&l1), Some(_))
            || matches!(self.history.get(&l2), Some(_))
        {
            false
        } else {
            self.history.insert(l1.clone(), l2.clone());
            self.history.insert(l2.clone(), l1.clone());
            self.true_history.insert(l1.clone(), l2.clone());
            self.true_history.insert(l2.clone(), l1.clone());
            self.equal_expression(&e1, &e2)
        }
    }
    fn equal_expression(
        &mut self,
        e1: &IntermediateExpression,
        e2: &IntermediateExpression,
    ) -> bool {
        match (e1, e2) {
            (
                IntermediateExpression::IntermediateValue(v1),
                IntermediateExpression::IntermediateValue(v2),
            ) => self.equal_value(&v1, &v2),
            (
                IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                    value: v1,
                    idx: i1,
                }),
                IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                    value: v2,
                    idx: i2,
                }),
            ) => i1 == i2 && self.equal_value(&v1, &v2),
            (
                IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                    values1,
                )),
                IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                    values2,
                )),
            ) => self.equal_values(&values1, &values2),
            (
                IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                    fn_: v1,
                    args: a1,
                }),
                IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                    fn_: v2,
                    args: a2,
                }),
            ) => self.equal_values(&a1, &a2) && self.equal_value(&v1, &v2),
            (
                IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                    idx: i1,
                    data: d1,
                    type_: t1,
                }),
                IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                    idx: i2,
                    data: d2,
                    type_: t2,
                }),
            ) => {
                i1 == i2
                    && match (d1, d2) {
                        (None, None) => true,
                        (Some(d1), Some(d2)) => self.equal_value(d1, d2),
                        _ => false,
                    }
                    && t1 == t2
            }
            (
                IntermediateExpression::IntermediateLambda(IntermediateLambda {
                    args: a1,
                    statements: s1,
                    ret: r1,
                }),
                IntermediateExpression::IntermediateLambda(IntermediateLambda {
                    args: a2,
                    statements: s2,
                    ret: r2,
                }),
            ) => {
                self.equal_args(a1, a2)
                    && self.equal_statements(&s1, &s2)
                    && r1.1 == r2.1
                    && self.equal_value(&r1.0, &r2.0)
            }
            _ => false,
        }
    }
    fn equal_value(&mut self, v1: &IntermediateValue, v2: &IntermediateValue) -> bool {
        match (v1, v2) {
            (
                IntermediateValue::IntermediateBuiltIn(b1),
                IntermediateValue::IntermediateBuiltIn(b2),
            ) => b1 == b2,
            (IntermediateValue::IntermediateArg(a1), IntermediateValue::IntermediateArg(a2)) => {
                self.equal_arg(a1, a2)
            }
            (
                IntermediateValue::IntermediateMemory(m1),
                IntermediateValue::IntermediateMemory(m2),
            ) => self.equal_locations(m1, m2),
            _ => false,
        }
    }
    fn equal_values(
        &mut self,
        values1: &Vec<IntermediateValue>,
        values2: &Vec<IntermediateValue>,
    ) -> bool {
        values1.len() == values2.len()
            && values1
                .iter()
                .zip(values2.iter())
                .all(|(v1, v2)| self.equal_value(v1, v2))
    }
    fn equal_statements(
        &mut self,
        statements1: &Vec<IntermediateStatement>,
        statements2: &Vec<IntermediateStatement>,
    ) -> bool {
        statements1.len() == statements2.len()
            && statements1
                .iter()
                .zip(statements2.iter())
                .all(|(s1, s2)| self.equal_statement(s1, s2))
    }
    fn equal_statement(&mut self, s1: &IntermediateStatement, s2: &IntermediateStatement) -> bool {
        match (s1, s2) {
            (
                IntermediateStatement::IntermediateAssignment(m1),
                IntermediateStatement::IntermediateAssignment(m2),
            ) => self.equal_assignment(m1, m2),
            (
                IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                    condition: c1,
                    branches: b1,
                }),
                IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                    condition: c2,
                    branches: b2,
                }),
            ) => {
                self.equal_value(c1, c2)
                    && self.equal_statements(&b1.0, &b2.0)
                    && self.equal_statements(&b1.1, &b2.1)
            }
            (
                IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                    subject: s1,
                    branches: b1,
                }),
                IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                    subject: s2,
                    branches: b2,
                }),
            ) => self.equal_value(s1, s2) && self.equal_branches(b1, b2),
            _ => false,
        }
    }
    fn equal_branch(
        &mut self,
        branch1: &IntermediateMatchBranch,
        branch2: &IntermediateMatchBranch,
    ) -> bool {
        let IntermediateMatchBranch {
            target: t1,
            statements: s1,
        } = branch1;
        let IntermediateMatchBranch {
            target: t2,
            statements: s2,
        } = branch2;
        (match (t1, t2) {
            (None, None) => true,
            (Some(a1), Some(a2)) => self.equal_arg(a1, a2),
            _ => false,
        }) && self.equal_statements(s1, s2)
    }
    fn equal_branches(
        &mut self,
        branches1: &Vec<IntermediateMatchBranch>,
        branches2: &Vec<IntermediateMatchBranch>,
    ) -> bool {
        branches1.len() == branches2.len()
            && branches1
                .iter()
                .zip(branches2.iter())
                .all(|(b1, e2)| self.equal_branch(b1, e2))
    }
}

#[derive(Clone, Eq, Debug)]
pub struct IntermediateArg {
    pub type_: IntermediateType,
    pub location: Location,
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateTupleExpression(pub Vec<IntermediateValue>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateFnCall {
    pub fn_: IntermediateValue,
    pub args: Vec<IntermediateValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateCtorCall {
    pub idx: usize,
    pub data: Option<IntermediateValue>,
    pub type_: IntermediateUnionType,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateLambda {
    pub args: Vec<IntermediateArg>,
    pub statements: Vec<IntermediateStatement>,
    pub ret: (IntermediateValue, IntermediateType),
}

type Substitution = HashMap<IntermediateValue, IntermediateValue>;

impl IntermediateLambda {
    pub fn find_open_vars(&self) -> Vec<IntermediateValue> {
        let targets: HashSet<Location> =
            HashSet::from_iter(IntermediateStatement::all_targets(&self.statements));
        let values = IntermediateExpression::from(self.clone()).values();
        values
            .into_iter()
            .unique()
            .into_iter()
            .filter(|value| match value {
                IntermediateValue::IntermediateBuiltIn(_) => false,
                IntermediateValue::IntermediateMemory(location) => !targets.contains(location),
                IntermediateValue::IntermediateArg(_) => true,
            })
            .collect()
    }
    pub fn substitute(&mut self, substitution: &Substitution) {
        IntermediateStatement::substitute_all(&mut self.statements, substitution);
    }
}

#[derive(Clone, PartialEq, FromVariants, Eq, Hash, Debug)]
pub enum IntermediateStatement {
    IntermediateAssignment(IntermediateAssignment),
    IntermediateFnDef(IntermediateFnDef),
    IntermediateIfStatement(IntermediateIfStatement),
    IntermediateMatchStatement(IntermediateMatchStatement),
}

impl IntermediateStatement {
    fn values(&self) -> Vec<IntermediateValue> {
        match self {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location: _,
            }) => expression.values(),
            IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                condition,
                branches,
            }) => {
                let mut values = IntermediateStatement::all_values(&branches.0);
                values.extend(IntermediateStatement::all_values(&branches.1));
                values.push(condition.clone());
                values
            }
            IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                subject,
                branches,
            }) => {
                let mut values = branches
                    .iter()
                    .flat_map(|IntermediateMatchBranch { target, statements }| {
                        IntermediateStatement::all_values(&statements)
                            .into_iter()
                            .filter(|value| match value {
                                IntermediateValue::IntermediateArg(arg) => {
                                    Some(arg) != target.as_ref()
                                }
                                _ => true,
                            })
                    })
                    .collect_vec();
                values.push(subject.clone());
                values
            }
            IntermediateStatement::IntermediateFnDef(intermediate_fn_def) => todo!(),
        }
    }
    fn all_values(statements: &Vec<Self>) -> Vec<IntermediateValue> {
        statements
            .iter()
            .flat_map(|statement| statement.values())
            .collect()
    }
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
            IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                condition: _,
                branches,
            }) => {
                let mut targets = IntermediateStatement::all_targets(&branches.0);
                targets.extend(IntermediateStatement::all_targets(&branches.1));
                targets
            }
            IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                subject: _,
                branches,
            }) => branches
                .iter()
                .flat_map(|branch| IntermediateStatement::all_targets(&branch.statements))
                .collect(),
            IntermediateStatement::IntermediateFnDef(intermediate_fn_def) => todo!(),
        }
    }
    pub fn all_targets(statements: &Vec<Self>) -> Vec<Location> {
        statements
            .iter()
            .flat_map(|statement| statement.targets())
            .collect()
    }
    fn substitute(&mut self, substitution: &Substitution) {
        match self {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location: _,
            }) => expression.substitute(substitution),
            IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                condition,
                branches,
            }) => {
                *condition = condition.substitute(substitution);
                IntermediateStatement::substitute_all(&mut branches.0, substitution);
                IntermediateStatement::substitute_all(&mut branches.1, substitution);
            }
            IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                subject,
                branches,
            }) => {
                *subject = subject.substitute(substitution);
                for branch in branches {
                    IntermediateStatement::substitute_all(&mut branch.statements, substitution);
                }
            }
            IntermediateStatement::IntermediateFnDef(intermediate_fn_def) => todo!(),
        }
    }
    fn substitute_all(statements: &mut Vec<Self>, substitution: &Substitution) {
        for statement in statements {
            statement.substitute(substitution)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateIfStatement {
    pub condition: IntermediateValue,
    pub branches: (Vec<IntermediateStatement>, Vec<IntermediateStatement>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateMatchStatement {
    pub subject: IntermediateValue,
    pub branches: Vec<IntermediateMatchBranch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateMatchBranch {
    pub target: Option<IntermediateArg>,
    pub statements: Vec<IntermediateStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntermediateProgram {
    pub statements: Vec<IntermediateStatement>,
    pub main: IntermediateValue,
    pub types: Vec<Rc<RefCell<IntermediateType>>>,
}
