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

static LOCATION_ID: AtomicUsize = AtomicUsize::new(0);
#[derive(Clone, PartialEq, Ord, PartialOrd, Hash, Eq)]
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
    fn substitute(&self, substitution: &Substitution) -> IntermediateValue {
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
        }
    }
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
            IntermediateExpression::IntermediateLambda(lambda) => {
                let IntermediateLambda {
                    args,
                    statements,
                    ret: return_value,
                } = lambda;
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
    left_true_history: HashMap<Location, Location>,
    right_true_history: HashMap<Location, Location>,
    left_history: HashMap<Location, Location>,
    right_history: HashMap<Location, Location>,
}

impl ExpressionEqualityChecker {
    pub fn assert_equal(e1: &IntermediateExpression, e2: &IntermediateExpression) {
        let mut expression_equality_checker = Self::new();
        expression_equality_checker.assert_equal_expression(e1, e2)
    }
    fn new() -> Self {
        ExpressionEqualityChecker {
            left_true_history: HashMap::new(),
            right_true_history: HashMap::new(),
            left_history: HashMap::new(),
            right_history: HashMap::new(),
        }
    }
    fn assert_equal_memory(&mut self, m1: &IntermediateMemory, m2: &IntermediateMemory) {
        let IntermediateMemory {
            location: l1,
            type_: _,
        } = m1;
        let IntermediateMemory {
            location: l2,
            type_: _,
        } = m2;
        self.assert_equal_locations(l1, l2)
    }
    fn assert_equal_arg(&mut self, a1: &IntermediateArg, a2: &IntermediateArg) {
        let IntermediateArg {
            location: l1,
            type_: _,
        } = a1;
        let IntermediateArg {
            location: l2,
            type_: _,
        } = a2;
        self.assert_equal_locations(l1, l2)
    }
    fn assert_equal_args(&mut self, a1: &Vec<IntermediateArg>, a2: &Vec<IntermediateArg>) {
        assert_eq!(a1.len(), a2.len());
        for (a1, a2) in a1.iter().zip_eq(a2.iter()) {
            self.assert_equal_arg(a1, a2)
        }
    }
    fn assert_equal_locations(&mut self, l1: &Location, l2: &Location) {
        if self.left_history.get(&l1) == Some(&l2) {
            return;
        }
        assert!(!matches!(self.left_history.get(&l1), Some(_)));
        assert!(!matches!(self.right_history.get(&l2), Some(_)));
        self.left_history.insert(l1.clone(), l2.clone());
        self.right_history.insert(l2.clone(), l1.clone());
    }
    fn assert_equal_assignment(
        &mut self,
        m1: &IntermediateAssignment,
        m2: &IntermediateAssignment,
    ) {
        let IntermediateAssignment {
            expression: e1,
            location: l1,
        } = m1;
        let IntermediateAssignment {
            expression: e2,
            location: l2,
        } = m2;
        if self.left_true_history.get(&l1) == Some(&l2) {
            return;
        }
        if self.left_history.get(&l1) == Some(&l2) {
            self.left_true_history.insert(l1.clone(), l2.clone());
            self.right_true_history.insert(l2.clone(), l1.clone());
            self.assert_equal_expression(&e1, &e2)
        } else {
            assert!(!matches!(self.left_true_history.get(&l1), Some(_)));
            assert!(!matches!(self.right_true_history.get(&l2), Some(_)));
            assert!(!matches!(self.left_history.get(&l1), Some(_)));
            assert!(!matches!(self.right_history.get(&l2), Some(_)));
            self.left_history.insert(l1.clone(), l2.clone());
            self.right_history.insert(l2.clone(), l1.clone());
            self.left_true_history.insert(l1.clone(), l2.clone());
            self.right_true_history.insert(l2.clone(), l1.clone());
            self.assert_equal_expression(&e1, &e2)
        }
    }
    fn assert_equal_expression(
        &mut self,
        e1: &IntermediateExpression,
        e2: &IntermediateExpression,
    ) {
        match (e1, e2) {
            (
                IntermediateExpression::IntermediateValue(v1),
                IntermediateExpression::IntermediateValue(v2),
            ) => self.assert_equal_value(&v1, &v2),
            (
                IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                    value: v1,
                    idx: i1,
                }),
                IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                    value: v2,
                    idx: i2,
                }),
            ) => {
                assert_eq!(i1, i2);
                self.assert_equal_value(&v1, &v2)
            }
            (
                IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                    values1,
                )),
                IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                    values2,
                )),
            ) => self.assert_equal_values(&values1, &values2),
            (
                IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                    fn_: v1,
                    args: a1,
                }),
                IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                    fn_: v2,
                    args: a2,
                }),
            ) => {
                self.assert_equal_values(&a1, &a2);
                self.assert_equal_value(&v1, &v2)
            }
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
                assert_eq!(i1, i2);
                match (d1, d2) {
                    (None, None) => {}
                    (Some(d1), Some(d2)) => self.assert_equal_value(d1, d2),
                    _ => assert!(false),
                }
                assert_eq!(t1, t2)
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
                self.assert_equal_args(a1, a2);
                self.assert_equal_statements(&s1, &s2);
                self.assert_equal_value(&r1, &r2)
            }
            _ => assert!(false),
        }
    }
    fn assert_equal_value(&mut self, v1: &IntermediateValue, v2: &IntermediateValue) {
        match (v1, v2) {
            (
                IntermediateValue::IntermediateBuiltIn(b1),
                IntermediateValue::IntermediateBuiltIn(b2),
            ) => assert_eq!(b1, b2),
            (IntermediateValue::IntermediateArg(a1), IntermediateValue::IntermediateArg(a2)) => {
                self.assert_equal_arg(a1, a2);
            }
            (
                IntermediateValue::IntermediateMemory(m1),
                IntermediateValue::IntermediateMemory(m2),
            ) => self.assert_equal_memory(m1, m2),
            _ => {
                assert!(false);
            }
        }
    }
    fn assert_equal_values(
        &mut self,
        values1: &Vec<IntermediateValue>,
        values2: &Vec<IntermediateValue>,
    ) {
        assert_eq!(values1.len(), values2.len());
        for (v1, v2) in values1.iter().zip_eq(values2.iter()) {
            self.assert_equal_value(v1, v2)
        }
    }
    fn assert_equal_statements(
        &mut self,
        statements1: &Vec<IntermediateStatement>,
        statements2: &Vec<IntermediateStatement>,
    ) {
        assert_eq!(statements1.len(), statements2.len());
        for (s1, s2) in statements1.iter().zip_eq(statements2.iter()) {
            self.assert_equal_statement(s1, s2)
        }
    }
    fn assert_equal_statement(&mut self, s1: &IntermediateStatement, s2: &IntermediateStatement) {
        match (s1, s2) {
            (
                IntermediateStatement::IntermediateAssignment(m1),
                IntermediateStatement::IntermediateAssignment(m2),
            ) => self.assert_equal_assignment(m1, m2),
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
                self.assert_equal_value(c1, c2);
                self.assert_equal_statements(&b1.0, &b2.0);
                self.assert_equal_statements(&b1.1, &b2.1)
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
            ) => {
                self.assert_equal_value(s1, s2);
                self.assert_equal_branches(b1, b2)
            }
            _ => assert!(false),
        }
    }
    fn assert_equal_branch(
        &mut self,
        branch1: &IntermediateMatchBranch,
        branch2: &IntermediateMatchBranch,
    ) {
        let IntermediateMatchBranch {
            target: t1,
            statements: s1,
        } = branch1;
        let IntermediateMatchBranch {
            target: t2,
            statements: s2,
        } = branch2;
        (match (t1, t2) {
            (None, None) => {}
            (Some(a1), Some(a2)) => self.assert_equal_arg(a1, a2),
            _ => assert!(false),
        });
        self.assert_equal_statements(s1, s2)
    }
    fn assert_equal_branches(
        &mut self,
        branches1: &Vec<IntermediateMatchBranch>,
        branches2: &Vec<IntermediateMatchBranch>,
    ) {
        assert_eq!(branches1.len(), branches2.len());
        for (b1, b2) in branches1.iter().zip_eq(branches2.iter()) {
            self.assert_equal_branch(b1, b2)
        }
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
pub struct IntermediateLambda {
    pub args: Vec<IntermediateArg>,
    pub statements: Vec<IntermediateStatement>,
    pub ret: IntermediateValue,
}

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
            Box::new(self.ret.type_()),
        )
    }

    pub fn find_open_vars(&self) -> Vec<IntermediateMemory> {
        let mut targets: HashSet<Location> =
            HashSet::from_iter(IntermediateStatement::all_targets(&self.statements));
        let mut args: HashSet<IntermediateArg> =
            HashSet::from_iter(IntermediateStatement::all_arguments(&self.statements));
        args.extend(self.args.clone());
        targets.extend(args.into_iter().map(|arg| arg.location));
        let values = IntermediateExpression::from(self.clone()).values();
        values
            .into_iter()
            .unique()
            .into_iter()
            .filter_map(|value| match value {
                IntermediateValue::IntermediateBuiltIn(_) => None,
                IntermediateValue::IntermediateMemory(memory) => {
                    if !targets.contains(&memory.location) {
                        Some(memory)
                    } else {
                        None
                    }
                }
                IntermediateValue::IntermediateArg(arg) => Some(IntermediateMemory {
                    location: arg.location,
                    type_: arg.type_,
                }),
            })
            .collect()
    }
    pub fn substitute(&mut self, substitution: &Substitution) {
        IntermediateStatement::substitute_all(&mut self.statements, substitution);
        self.ret = self.ret.substitute(substitution);
    }
}

#[derive(Clone, PartialEq, FromVariants, Eq, Hash)]
pub enum IntermediateStatement {
    IntermediateAssignment(IntermediateAssignment),
    IntermediateIfStatement(IntermediateIfStatement),
    IntermediateMatchStatement(IntermediateMatchStatement),
}

impl fmt::Debug for IntermediateStatement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntermediateAssignment(assignment) => assignment.fmt(f),
            Self::IntermediateIfStatement(if_statement) => if_statement.fmt(f),
            Self::IntermediateMatchStatement(match_statement) => match_statement.fmt(f),
        }
    }
}

impl IntermediateStatement {
    pub fn values(&self) -> Vec<IntermediateValue> {
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
        }
    }
    fn arguments(&self) -> Vec<IntermediateArg> {
        match self {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression:
                    IntermediateExpression::IntermediateLambda(IntermediateLambda {
                        args: arguments,
                        statements,
                        ret: _,
                    }),
                location: _,
            }) => {
                let mut args = IntermediateStatement::all_arguments(statements);
                args.extend(arguments.clone());
                args
            }
            IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                condition: _,
                branches,
            }) => {
                let mut args = IntermediateStatement::all_arguments(&branches.0);
                args.extend(IntermediateStatement::all_arguments(&branches.1));
                args
            }
            IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                subject: _,
                branches,
            }) => branches
                .iter()
                .flat_map(|IntermediateMatchBranch { target, statements }| {
                    let mut args = IntermediateStatement::all_arguments(&statements);
                    if let Some(arg) = target {
                        args.push(arg.clone());
                    };
                    args
                })
                .collect(),
            _ => Vec::new(),
        }
    }
    pub fn all_targets(statements: &Vec<Self>) -> Vec<Location> {
        statements
            .iter()
            .flat_map(|statement| statement.targets())
            .collect()
    }
    pub fn all_arguments(statements: &Vec<Self>) -> Vec<IntermediateArg> {
        statements
            .iter()
            .flat_map(|statement| statement.arguments())
            .collect()
    }
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
    pub main: IntermediateLambda,
    pub types: Vec<Rc<RefCell<IntermediateType>>>,
}
