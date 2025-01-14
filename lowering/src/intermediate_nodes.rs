use core::fmt;
use itertools::Itertools;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
};

use from_variants::FromVariants;
use type_checker::{AtomicTypeEnum, Boolean, Integer};

use crate::{AtomicType, Name};

#[derive(Clone, PartialEq, FromVariants, Eq)]
pub enum IntermediateType {
    AtomicType(AtomicType),
    IntermediateTupleType(IntermediateTupleType),
    IntermediateFnType(IntermediateFnType),
    IntermediateUnionType(IntermediateUnionType),
    IntermediateReferenceType(Rc<RefCell<IntermediateType>>),
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
            Self::IntermediateReferenceType(_) => {
                f.debug_tuple("IntermediateReferenceType").finish()
            }
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateTupleType(pub Vec<IntermediateType>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateFnType(pub Vec<IntermediateType>, pub Box<IntermediateType>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntermediateUnionType(pub Vec<Option<IntermediateType>>);

impl Hash for IntermediateUnionType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeDef(pub Rc<RefCell<Vec<Option<IntermediateType>>>>);

impl Hash for TypeDef {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, FromVariants, Eq, Hash)]
pub enum IntermediateValue {
    IntermediateBuiltIn(IntermediateBuiltIn),
    IntermediateMemory(IntermediateMemory),
    IntermediateArgument(IntermediateArgument),
}

impl From<IntermediateExpression> for IntermediateValue {
    fn from(value: IntermediateExpression) -> Self {
        IntermediateValue::IntermediateMemory(value.into())
    }
}

#[derive(Clone, Debug, FromVariants, PartialEq, Eq, Hash)]
pub enum IntermediateBuiltIn {
    Integer(Integer),
    Boolean(Boolean),
    BuiltInFn(Name, IntermediateType),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntermediateMemory(pub Rc<RefCell<IntermediateExpression>>);

impl Hash for IntermediateMemory {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}

impl From<IntermediateExpression> for IntermediateMemory {
    fn from(value: IntermediateExpression) -> Self {
        IntermediateMemory(Rc::new(RefCell::new(value)))
    }
}

impl From<IntermediateArgument> for IntermediateMemory {
    fn from(value: IntermediateArgument) -> Self {
        IntermediateMemory(Rc::new(RefCell::new(value.into())))
    }
}

#[derive(Clone, Eq, FromVariants, Hash)]
pub enum IntermediateExpression {
    IntermediateValue(IntermediateValue),
    IntermediateElementAccess(IntermediateElementAccess),
    IntermediateTupleExpression(IntermediateTupleExpression),
    IntermediateFnCall(IntermediateFnCall),
    IntermediateCtorCall(IntermediateCtorCall),
    IntermediateFnDef(IntermediateFnDef),
}

impl fmt::Debug for IntermediateExpression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut expression_formatter = ExpressionFormatter::new();
        expression_formatter.format(f, self)
    }
}

impl From<IntermediateArgument> for IntermediateExpression {
    fn from(value: IntermediateArgument) -> Self {
        IntermediateExpression::IntermediateValue(value.into())
    }
}

impl PartialEq for IntermediateExpression {
    fn eq(&self, other: &Self) -> bool {
        let mut expression_equality_checker = ExpressionEqualityChecker::new();
        expression_equality_checker.equal_expression(self, other)
    }
}

struct ExpressionEqualityChecker {
    history: HashMap<*mut IntermediateExpression, *mut IntermediateExpression>,
}
impl ExpressionEqualityChecker {
    fn new() -> Self {
        ExpressionEqualityChecker {
            history: HashMap::new(),
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
                }),
                IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                    idx: i2,
                    data: d2,
                }),
            ) => i1 == i2 && d1 == d2,
            (
                IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                    arguments: a1,
                    statements: s1,
                    return_value: r1,
                }),
                IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                    arguments: a2,
                    statements: s2,
                    return_value: r2,
                }),
            ) => a1 == a2 && self.eq_statements(&s1, &s2) && self.equal_value(&r1, &r2),
            _ => false,
        }
    }
    fn equal_value(&mut self, v1: &IntermediateValue, v2: &IntermediateValue) -> bool {
        match (v1, v2) {
            (
                IntermediateValue::IntermediateBuiltIn(b1),
                IntermediateValue::IntermediateBuiltIn(b2),
            ) => b1 == b2,
            (
                IntermediateValue::IntermediateArgument(a1),
                IntermediateValue::IntermediateArgument(a2),
            ) => a1 == a2,
            (
                IntermediateValue::IntermediateMemory(m1),
                IntermediateValue::IntermediateMemory(m2),
            ) => self.equal_memory(m1, m2),
            _ => false,
        }
    }
    fn equal_memory(&mut self, m1: &IntermediateMemory, m2: &IntermediateMemory) -> bool {
        let IntermediateMemory(m1) = m1;
        let IntermediateMemory(m2) = m2;
        if self.history.get(&m1.as_ptr()) == self.history.get(&m2.as_ptr()) {
            true
        } else if matches!(self.history.get(&m1.as_ptr()), Some(_))
            || matches!(self.history.get(&m2.as_ptr()), Some(_))
        {
            false
        } else {
            self.history.insert(m1.as_ptr(), m2.as_ptr());
            self.history.insert(m2.as_ptr(), m1.as_ptr());
            self.equal_expression(&m1.borrow().clone(), &m2.borrow().clone())
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
    fn eq_statements(
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
                    && self.eq_statements(&b1.0, &b2.0)
                    && self.eq_statements(&b1.1, &b2.1)
            }
            (
                IntermediateStatement::IntermediateMatchStatement(_),
                IntermediateStatement::IntermediateMatchStatement(_),
            ) => todo!(),
            _ => false,
        }
    }
}

struct ExpressionFormatter {
    history: HashSet<*mut IntermediateExpression>,
}
impl ExpressionFormatter {
    fn new() -> Self {
        Self {
            history: HashSet::new(),
        }
    }
    fn format(
        &mut self,
        f: &mut std::fmt::Formatter<'_>,
        expression: &IntermediateExpression,
    ) -> std::fmt::Result {
        write!(f, "{}", self.format_expression(expression))
    }
    fn format_expression(&mut self, expression: &IntermediateExpression) -> String {
        match &expression {
            IntermediateExpression::IntermediateValue(value) => {
                format!("Value({})", self.format_value(value))
            }
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx,
            }) => {
                format!("ElementAccess({},{})", self.format_value(value), idx)
            }
            IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                values,
            )) => {
                format!("TupleExpression({})", self.format_values(values))
            }
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args }) => {
                format!(
                    "FnCall({},{})",
                    self.format_value(fn_),
                    self.format_values(args)
                )
            }
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall { idx, data }) => {
                format!("CtorCall({},{:?})", idx, data)
            }
            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                arguments: _,
                statements,
                return_value,
            }) => {
                format!(
                    "FnDef({},{})",
                    self.format_statements(statements),
                    self.format_value(return_value)
                )
            }
        }
    }
    fn format_value(&mut self, value: &IntermediateValue) -> String {
        match value {
            IntermediateValue::IntermediateBuiltIn(intermediate_built_in) => {
                format!("{:?}", intermediate_built_in)
            }
            IntermediateValue::IntermediateMemory(IntermediateMemory(expression)) => {
                if self.history.contains(&expression.as_ptr()) {
                    format!("{:#?}", expression.as_ptr())
                } else {
                    self.history.insert(expression.as_ptr());
                    self.format_expression(&expression.borrow().clone())
                }
            }
            IntermediateValue::IntermediateArgument(IntermediateArgument(type_)) => {
                format!("Arg({:?})", type_)
            }
        }
    }
    fn format_values(&mut self, values: &Vec<IntermediateValue>) -> String {
        values
            .iter()
            .map(|value| self.format_value(value))
            .join(",")
    }
    fn format_statement(&mut self, statement: &IntermediateStatement) -> String {
        match statement {
            IntermediateStatement::Assignment(IntermediateMemory(memory)) => {
                format!(
                    "{:#?} = {}",
                    memory.as_ptr(),
                    self.format_expression(&memory.borrow().clone())
                )
            }
            IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                condition,
                branches,
            }) => {
                format!(
                    "If({},{},{})",
                    self.format_value(condition),
                    self.format_statements(&branches.0),
                    self.format_statements(&branches.1)
                )
            }
            IntermediateStatement::IntermediateMatchStatement(_) => {
                todo!()
            }
        }
    }
    fn format_statements(&mut self, statements: &Vec<IntermediateStatement>) -> String {
        statements
            .iter()
            .map(|statement| self.format_statement(statement))
            .join(";")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateArgument(pub IntermediateType);

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntermediateCtorCall {
    pub idx: usize,
    pub data: Rc<RefCell<Vec<Option<IntermediateType>>>>,
}

impl Hash for IntermediateCtorCall {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.idx.hash(state);
        self.data.as_ptr().hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateFnDef {
    pub arguments: Vec<IntermediateArgument>,
    pub statements: Vec<IntermediateStatement>,
    pub return_value: IntermediateValue,
}

#[derive(Clone, Debug, PartialEq, FromVariants, Eq, Hash)]
pub enum IntermediateStatement {
    Assignment(IntermediateMemory),
    IntermediateIfStatement(IntermediateIfStatement),
    IntermediateMatchStatement(IntermediateMatchStatement),
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
    pub expression: IntermediateValue,
    pub branches: Vec<IntermediateMatchBranch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateMatchBranch {
    pub target: Option<IntermediateArgument>,
    pub statements: Vec<IntermediateMatchBranch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntermediateProgram {
    pub statements: Vec<IntermediateStatement>,
    pub main: IntermediateValue,
}
