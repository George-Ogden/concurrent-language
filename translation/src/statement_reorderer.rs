use std::collections::HashSet;

use crate::{Assignment, Expression, FnCall, Memory, Statement, Value};

struct StatementReorderer {
    fn_calls: HashSet<Memory>,
}

impl StatementReorderer {
    fn new() -> Self {
        Self {
            fn_calls: HashSet::new(),
        }
    }

    fn collect_fn_calls(&mut self, statements: &Vec<Statement>) {
        for statement in statements {
            match statement {
                Statement::Assignment(Assignment {
                    target,
                    value:
                        Expression::FnCall(FnCall {
                            fn_: Value::Memory(_),
                            fn_type: _,
                            args: _,
                        }),
                }) => {
                    self.fn_calls.insert(target.clone());
                }
                Statement::IfStatement(if_) => {
                    self.collect_fn_calls(&if_.branches.0);
                    self.collect_fn_calls(&if_.branches.1);
                }
                Statement::MatchStatement(match_) => {
                    for branch in &match_.branches {
                        self.collect_fn_calls(&branch.statements);
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Assignment, Await, BuiltIn, Declaration, FnCall, FnType, Id, IfStatement, MatchBranch,
        MatchStatement, Name, Statement, UnionType,
    };

    use super::*;

    use lowering::{AtomicTypeEnum, Boolean};
    use test_case::test_case;

    #[test_case(
        vec![
            Await(vec![Memory(Id::from("f"))]).into(),
            Assignment{
                target: Memory(Id::from("t1")),
                value: FnCall {
                    fn_: Memory(Id::from("f")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m1")).into(),
                    ]
                }.into(),
            }.into(),
            Await(vec![Memory(Id::from("f"))]).into(),
            Assignment{
                target: Memory(Id::from("t2")),
                value: FnCall {
                    fn_: Memory(Id::from("f")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m2")).into(),
                    ]
                }.into(),
            }.into(),
            Assignment{
                target: Memory(Id::from("t2")),
                value: FnCall {
                    fn_: BuiltIn::BuiltInFn(Name::from("Plus__BuiltIn")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m3")).into(),
                    ]
                }.into(),
            }.into()
        ],
        vec!["t1", "t2"];
        "consecutive statements"
    )]
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("f"))]).into(),
            Assignment{
                target: Memory(Id::from("t1")),
                value: FnCall {
                    fn_: Memory(Id::from("f")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("x0")).into(),
                    ]
                }.into(),
            }.into(),
            Declaration{
                memory: Memory(Id::from("shared")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            IfStatement{
                condition: Value::from(Boolean{value: false}).into(),
                branches: (
                    vec![
                        Await(vec![Memory(Id::from("f"))]).into(),
                        Assignment{
                            target: Memory(Id::from("t2")),
                            value: FnCall {
                                fn_: Memory(Id::from("f")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("x1")).into(),
                                ]
                            }.into(),
                        }.into(),
                        Assignment{
                            target: Memory(Id::from("shared")),
                            value: Memory(Id::from("t2")).into(),
                        }.into()
                    ],
                    vec![
                        Await(vec![Memory(Id::from("f"))]).into(),
                        Assignment{
                            target: Memory(Id::from("t3")),
                            value: FnCall {
                                fn_: Memory(Id::from("f")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("x2")).into(),
                                ]
                            }.into(),
                        }.into(),
                        Assignment{
                            target: Memory(Id::from("shared")),
                            value: Memory(Id::from("t3")).into(),
                        }.into()
                    ],
                )
            }.into(),
        ],
        vec!["t1","t2","t3"];
        "statements in if"
    )]
    #[test_case(
        vec![
            Declaration{
                memory: Memory(Id::from("shared")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            MatchStatement{
                expression: (
                    Memory(Id::from("subject")).into(),
                    UnionType(vec![Name::from("A"), Name::from("B"), Name::from("C")])
                ),
                auxiliary_memory: Memory(Id::from("aux")),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("a"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Assignment{
                                target: Memory(Id::from("t0")),
                                value: FnCall {
                                    fn_: Memory(Id::from("f")).into(),
                                    fn_type: FnType(
                                        vec![AtomicTypeEnum::INT.into()],
                                        Box::new(AtomicTypeEnum::BOOL.into())
                                    ),
                                    args: vec![
                                        Memory(Id::from("a")).into(),
                                    ]
                                }.into(),
                            }.into(),
                            Assignment{
                                target: Memory(Id::from("shared")),
                                value: Memory(Id::from("t0")).into(),
                            }.into(),
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("b"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Assignment{
                                target: Memory(Id::from("t1")),
                                value: FnCall {
                                    fn_: Memory(Id::from("f")).into(),
                                    fn_type: FnType(
                                        vec![AtomicTypeEnum::INT.into()],
                                        Box::new(AtomicTypeEnum::BOOL.into())
                                    ),
                                    args: vec![
                                        Memory(Id::from("b")).into(),
                                    ]
                                }.into(),
                            }.into(),
                            Assignment{
                                target: Memory(Id::from("shared")),
                                value: Memory(Id::from("t1")).into(),
                            }.into(),
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("c"))),
                        statements: vec![
                            Assignment{
                                target: Memory(Id::from("shared")),
                                value: Memory(Id::from("c")).into(),
                            }.into(),
                        ],
                    },
                ]
            }.into(),
            Await(vec![Memory(Id::from("f"))]).into(),
            Assignment{
                target: Memory(Id::from("y")),
                value: FnCall {
                    fn_: Memory(Id::from("f")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("x2")).into(),
                    ]
                }.into(),
            }.into(),
        ],
        vec!["t0", "t1", "y"];
        "statements in match"
    )]
    fn test_collect_fn_calls(statements: Vec<Statement>, fn_calls: Vec<&str>) {
        let mut reorderer = StatementReorderer::new();
        reorderer.collect_fn_calls(&statements);
        assert_eq!(
            HashSet::from_iter(fn_calls.into_iter().map(|id| Memory(Id::from(id)))),
            reorderer.fn_calls
        )
    }
}
