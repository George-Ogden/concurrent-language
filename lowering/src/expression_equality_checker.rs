use std::collections::HashMap;

use itertools::Itertools;

use crate::{
    IntermediateArg, IntermediateAssignment, IntermediateBlock, IntermediateCtorCall,
    IntermediateElementAccess, IntermediateExpression, IntermediateFnCall, IntermediateIf,
    IntermediateLambda, IntermediateMatch, IntermediateMatchBranch, IntermediateMemory,
    IntermediateStatement, IntermediateTupleExpression, IntermediateValue, Location,
};

/// Check whether two expressions are equal, storing verified and unverified references.
pub struct ExpressionEqualityChecker {
    // The true histories store bidirectional maps of equal assignment targets.
    left_true_history: HashMap<Location, Location>,
    right_true_history: HashMap<Location, Location>,
    // The histories store bidirectional maps of equal locations (potentially excluding assignment targets).
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
            // Locations have already been deemed equal.
            return;
        }
        // Check that the locations have not been found unequal.
        assert!(!matches!(self.left_history.get(&l1), Some(_)));
        assert!(!matches!(self.right_history.get(&l2), Some(_)));
        // Assume locations are equal.
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
            // If two locations have been assumed as equal, keep this assumption.
            self.left_true_history.insert(l1.clone(), l2.clone());
            self.right_true_history.insert(l2.clone(), l1.clone());
            self.assert_equal_expression(&e1, &e2)
        } else {
            // Ensure there are no existing assumptions about equality.
            assert!(!matches!(self.left_true_history.get(&l1), Some(_)));
            assert!(!matches!(self.right_true_history.get(&l2), Some(_)));
            assert!(!matches!(self.left_history.get(&l1), Some(_)));
            assert!(!matches!(self.right_history.get(&l2), Some(_)));
            // Assume that the locations are equal from here onwards.
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
                    block: b1,
                }),
                IntermediateExpression::IntermediateLambda(IntermediateLambda {
                    args: a2,
                    block: b2,
                }),
            ) => {
                self.assert_equal_args(a1, a2);
                self.assert_equal_block(&b1, &b2);
            }
            (
                IntermediateExpression::IntermediateIf(IntermediateIf {
                    condition: c1,
                    branches: b1,
                }),
                IntermediateExpression::IntermediateIf(IntermediateIf {
                    condition: c2,
                    branches: b2,
                }),
            ) => {
                self.assert_equal_value(c1, c2);
                self.assert_equal_block(&b1.0, &b2.0);
                self.assert_equal_block(&b1.1, &b2.1);
            }
            (
                IntermediateExpression::IntermediateMatch(IntermediateMatch {
                    subject: s1,
                    branches: b1,
                }),
                IntermediateExpression::IntermediateMatch(IntermediateMatch {
                    subject: s2,
                    branches: b2,
                }),
            ) => {
                self.assert_equal_value(s1, s2);
                self.assert_equal_branches(b1, b2);
            }
            _ => assert!(false),
        }
    }
    fn assert_equal_block(&mut self, b1: &IntermediateBlock, b2: &IntermediateBlock) {
        self.assert_equal_statements(&b1.statements, &b2.statements);
        self.assert_equal_value(&b1.ret, &b2.ret)
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
        }
    }
    fn assert_equal_branch(
        &mut self,
        branch1: &IntermediateMatchBranch,
        branch2: &IntermediateMatchBranch,
    ) {
        let IntermediateMatchBranch {
            target: t1,
            block: b1,
        } = branch1;
        let IntermediateMatchBranch {
            target: t2,
            block: b2,
        } = branch2;
        (match (t1, t2) {
            (None, None) => {}
            (Some(a1), Some(a2)) => self.assert_equal_arg(a1, a2),
            _ => assert!(false),
        });
        self.assert_equal_block(b1, b2)
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
