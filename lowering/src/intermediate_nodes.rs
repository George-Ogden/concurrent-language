use core::fmt;
use itertools::Itertools;
use std::{cell::RefCell, collections::HashMap, hash::Hash, rc::Rc};

use from_variants::FromVariants;
use type_checker::{AtomicTypeEnum, Boolean, Integer};

use crate::{AtomicType, Name};

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
            Self::Reference(_) => f.debug_tuple("Reference").finish(),
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

pub type Location = Rc<RefCell<()>>;

#[derive(Clone, FromVariants, Eq)]
pub enum IntermediateValue {
    IntermediateBuiltIn(IntermediateBuiltIn),
    IntermediateMemory(Location),
    IntermediateArg(IntermediateArg),
}

impl fmt::Debug for IntermediateValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntermediateBuiltIn(arg0) => {
                f.debug_tuple("IntermediateBuiltIn").field(arg0).finish()
            }
            Self::IntermediateMemory(arg0) => f
                .debug_tuple("IntermediateMemory")
                .field(&arg0.as_ptr())
                .finish(),
            Self::IntermediateArg(arg0) => f.debug_tuple("IntermediateArg").field(arg0).finish(),
        }
    }
}

impl PartialEq for IntermediateValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::IntermediateBuiltIn(l0), Self::IntermediateBuiltIn(r0)) => l0 == r0,
            (Self::IntermediateMemory(l0), Self::IntermediateMemory(r0)) => {
                l0.as_ptr() == r0.as_ptr()
            }
            (Self::IntermediateArg(l0), Self::IntermediateArg(r0)) => l0 == r0,
            _ => false,
        }
    }
}

impl Hash for IntermediateValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            IntermediateValue::IntermediateBuiltIn(built_in) => {
                0.hash(state);
                built_in.hash(state);
            }
            IntermediateValue::IntermediateMemory(location) => {
                1.hash(state);
                location.as_ptr().hash(state);
            }
            IntermediateValue::IntermediateArg(arg) => {
                2.hash(state);
                arg.hash(state);
            }
        }
    }
}

impl From<Location> for IntermediateValue {
    fn from(value: Location) -> Self {
        Self::IntermediateMemory(value)
    }
}

#[derive(Clone, Debug, FromVariants, PartialEq, Eq, Hash)]
pub enum IntermediateBuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(Name, IntermediateType),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntermediateMemory {
    pub expression: Rc<RefCell<IntermediateExpression>>,
    pub location: Location,
}

impl Hash for IntermediateMemory {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.expression.as_ptr().hash(state);
        self.location.as_ptr().hash(state);
    }
}

impl From<IntermediateExpression> for IntermediateMemory {
    fn from(value: IntermediateExpression) -> Self {
        IntermediateMemory {
            expression: Rc::new(RefCell::new(value)),
            location: Rc::new(RefCell::new(())),
        }
    }
}

impl From<IntermediateArg> for IntermediateMemory {
    fn from(value: IntermediateArg) -> Self {
        IntermediateMemory {
            expression: Rc::new(RefCell::new(value.into())),
            location: Rc::new(RefCell::new(())),
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
    IntermediateFnDef(IntermediateFnDef),
}

impl IntermediateExpression {
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
            IntermediateExpression::IntermediateFnDef(_) => Vec::new(),
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
    true_history: HashMap<*mut (), *mut ()>,
    history: HashMap<*mut (), *mut ()>,
    args: HashMap<*mut IntermediateType, *mut IntermediateType>,
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
            args: HashMap::new(),
        }
    }
    fn equal_arg(&mut self, a1: &IntermediateArg, a2: &IntermediateArg) -> bool {
        let IntermediateArg(t1) = a1;
        let IntermediateArg(t2) = a2;
        if self.args.get(&t1.as_ptr()) == Some(&t2.as_ptr()) {
            true
        } else if matches!(self.args.get(&t1.as_ptr()), Some(_))
            || matches!(self.args.get(&t2.as_ptr()), Some(_))
            || t1 != t2
        {
            false
        } else {
            self.args.insert(t1.as_ptr(), t2.as_ptr());
            self.args.insert(t2.as_ptr(), t1.as_ptr());
            true
        }
    }
    fn equal_args(&mut self, a1: &Vec<IntermediateArg>, a2: &Vec<IntermediateArg>) -> bool {
        a1.len() == a2.len()
            && a1
                .iter()
                .zip(a2.iter())
                .all(|(a1, a2)| self.equal_arg(a1, a2))
    }
    fn equal_locations(&mut self, l1: &Location, l2: &Location) -> bool {
        if self.history.get(&l1.as_ptr()) == Some(&l2.as_ptr()) {
            true
        } else if matches!(self.history.get(&l1.as_ptr()), Some(_))
            || matches!(self.history.get(&l2.as_ptr()), Some(_))
        {
            false
        } else {
            self.history.insert(l1.as_ptr(), l2.as_ptr());
            self.history.insert(l2.as_ptr(), l1.as_ptr());
            true
        }
    }
    fn equal_memory(&mut self, m1: &IntermediateMemory, m2: &IntermediateMemory) -> bool {
        let IntermediateMemory {
            expression: e1,
            location: l1,
        } = m1;
        let IntermediateMemory {
            expression: e2,
            location: l2,
        } = m2;
        if self.true_history.get(&l1.as_ptr()) == Some(&l2.as_ptr()) {
            true
        } else if self.history.get(&l1.as_ptr()) == Some(&l2.as_ptr()) {
            self.true_history.insert(l1.as_ptr(), l2.as_ptr());
            self.true_history.insert(l2.as_ptr(), l1.as_ptr());
            self.equal_expression(&e1.borrow().clone(), &e2.borrow().clone())
        } else if matches!(self.true_history.get(&l1.as_ptr()), Some(_))
            || matches!(self.true_history.get(&l2.as_ptr()), Some(_))
            || matches!(self.history.get(&l1.as_ptr()), Some(_))
            || matches!(self.history.get(&l2.as_ptr()), Some(_))
        {
            false
        } else {
            self.history.insert(l1.as_ptr(), l2.as_ptr());
            self.history.insert(l2.as_ptr(), l1.as_ptr());
            self.true_history.insert(l1.as_ptr(), l2.as_ptr());
            self.true_history.insert(l2.as_ptr(), l1.as_ptr());
            self.equal_expression(&e1.borrow().clone(), &e2.borrow().clone())
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
                IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                    args: a1,
                    statements: s1,
                    return_value: r1,
                }),
                IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                    args: a2,
                    statements: s2,
                    return_value: r2,
                }),
            ) => {
                self.equal_args(a1, a2)
                    && self.equal_statements(&s1, &s2)
                    && self.equal_value(&r1, &r2)
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
            (IntermediateStatement::Assignment(m1), IntermediateStatement::Assignment(m2)) => {
                self.equal_memory(m1, m2)
            }
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

#[derive(Clone, Eq)]
pub struct IntermediateArg(pub Rc<RefCell<IntermediateType>>);

impl fmt::Debug for IntermediateArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("IntermediateArg")
            .field(&self.0.as_ptr())
            .finish()
    }
}

impl PartialEq for IntermediateArg {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ptr() == other.0.as_ptr()
    }
}

impl From<IntermediateType> for IntermediateArg {
    fn from(value: IntermediateType) -> Self {
        IntermediateArg(Rc::new(RefCell::new(value)))
    }
}

impl Hash for IntermediateArg {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.borrow().hash(state);
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
pub struct IntermediateFnDef {
    pub args: Vec<IntermediateArg>,
    pub statements: Vec<IntermediateStatement>,
    pub return_value: IntermediateValue,
}

#[derive(Clone, PartialEq, FromVariants, Eq, Hash)]
pub enum IntermediateStatement {
    Assignment(IntermediateMemory),
    IntermediateIfStatement(IntermediateIfStatement),
    IntermediateMatchStatement(IntermediateMatchStatement),
}

impl fmt::Debug for IntermediateStatement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Assignment(IntermediateMemory {
                expression,
                location,
            }) => write!(
                f,
                "{:#?} = {:?}",
                location.as_ptr(),
                expression.borrow().clone()
            ),
            Self::IntermediateIfStatement(arg0) => f
                .debug_tuple("IntermediateIfStatement")
                .field(arg0)
                .finish(),
            Self::IntermediateMatchStatement(arg0) => f
                .debug_tuple("IntermediateMatchStatement")
                .field(arg0)
                .finish(),
        }
    }
}

impl From<IntermediateMemory> for IntermediateStatement {
    fn from(value: IntermediateMemory) -> Self {
        IntermediateStatement::Assignment(value)
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateProgram {
    pub statements: Vec<IntermediateStatement>,
    pub main: IntermediateValue,
}
