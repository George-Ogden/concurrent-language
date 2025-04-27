use std::collections::HashSet;

use itertools::Itertools;

use crate::{Assignment, Await, Expression, Id, IfStatement, Memory, Statement};

#[derive(Clone, Debug)]
struct AwaitDeduplicator {
    awaited_ids: HashSet<Memory>,
}

impl AwaitDeduplicator {
    fn new() -> Self {
        Self {
            awaited_ids: HashSet::new(),
        }
    }
    fn deduplicate_statements(&mut self, statements: Vec<Statement>) -> Vec<Statement> {
        statements
            .into_iter()
            .filter_map(|statement| match statement {
                Statement::Await(await_) => self.deduplicate_await(await_).map(Statement::from),
                Statement::IfStatement(if_) => Some(self.deduplicate_if(if_).into()),
                Statement::MatchStatement(match_statement) => todo!(),
                statement => Some(statement),
            })
            .collect_vec()
    }
    fn deduplicate_await(&mut self, Await(ids): Await) -> Option<Await> {
        {
            let fresh_ids = ids
                .into_iter()
                .filter(|id| !self.awaited_ids.contains(id))
                .collect_vec();
            if fresh_ids.is_empty() {
                None
            } else {
                self.awaited_ids.extend(fresh_ids.clone());
                Some(Await(fresh_ids))
            }
        }
    }
    fn deduplicate_if(
        &mut self,
        IfStatement {
            condition,
            branches: (true_branch, false_branch),
        }: IfStatement,
    ) -> IfStatement {
        let mut true_deduplicator = self.clone();
        let true_branch = true_deduplicator.deduplicate_statements(true_branch);
        let false_branch = self.deduplicate_statements(false_branch);
        self.awaited_ids = true_deduplicator
            .awaited_ids
            .intersection(&self.awaited_ids)
            .cloned()
            .collect::<HashSet<_>>();
        IfStatement {
            condition,
            branches: (true_branch, false_branch),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Assignment, Await, Declaration, FnCall, FnType, Id, IfStatement, Memory, Statement, Value,
    };

    use super::*;

    use lowering::{AtomicTypeEnum, Boolean};
    use test_case::test_case;

    #[test_case(
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Assignment{
                target: Memory(Id::from("m2")),
                value: FnCall {
                    fn_: Memory(Id::from("m0")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m1")).into(),
                    ]
                }.into(),
            }.into(),
            Await(vec![Memory(Id::from("m0"))]).into(),
            Assignment{
                target: Memory(Id::from("m4")),
                value: FnCall {
                    fn_: Memory(Id::from("m0")).into(),
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
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Assignment{
                target: Memory(Id::from("m2")),
                value: FnCall {
                    fn_: Memory(Id::from("m0")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m1")).into(),
                    ]
                }.into(),
            }.into(),
            Assignment{
                target: Memory(Id::from("m4")),
                value: FnCall {
                    fn_: Memory(Id::from("m0")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m3")).into(),
                    ]
                }.into(),
            }.into()
        ];
        "multiple awaits"
    )]
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Assignment{
                target: Memory(Id::from("m2")),
                value: FnCall {
                    fn_: Memory(Id::from("m0")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m1")).into(),
                    ]
                }.into(),
            }.into(),
            Declaration{
                memory: Memory(Id::from("m3")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            IfStatement{
                condition: Value::from(Boolean{value: false}).into(),
                branches: (
                    vec![
                        Await(vec![Memory(Id::from("m0"))]).into(),
                        Assignment{
                            target: Memory(Id::from("m5")),
                            value: FnCall {
                                fn_: Memory(Id::from("m0")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("m4")).into(),
                                ]
                            }.into(),
                        }.into(),
                        Assignment{
                            target: Memory(Id::from("m3")),
                            value: Memory(Id::from("m5")).into(),
                        }.into()
                    ],
                    vec![
                        Assignment{
                            target: Memory(Id::from("m3")),
                            value: Value::from(Boolean{value: true}).into(),
                        }.into(),
                    ]
                )
            }.into(),
            Assignment{
                target: Memory(Id::from("m6")),
                value: Memory(Id::from("m3")).into(),
            }.into()
        ],
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Assignment{
                target: Memory(Id::from("m2")),
                value: FnCall {
                    fn_: Memory(Id::from("m0")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m1")).into(),
                    ]
                }.into(),
            }.into(),
            Declaration{
                memory: Memory(Id::from("m3")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            IfStatement{
                condition: Value::from(Boolean{value: false}).into(),
                branches: (
                    vec![
                        Assignment{
                            target: Memory(Id::from("m5")),
                            value: FnCall {
                                fn_: Memory(Id::from("m0")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("m4")).into(),
                                ]
                            }.into(),
                        }.into(),
                        Assignment{
                            target: Memory(Id::from("m3")),
                            value: Memory(Id::from("m5")).into(),
                        }.into()
                    ],
                    vec![
                        Assignment{
                            target: Memory(Id::from("m3")),
                            value: Value::from(Boolean{value: true}).into(),
                        }.into(),
                    ]
                )
            }.into(),
            Assignment{
                target: Memory(Id::from("m6")),
                value: Memory(Id::from("m3")).into(),
            }.into()
        ];
        "await in if statement"
    )]
    #[test_case(
        vec![
            Declaration{
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            IfStatement{
                condition: Value::from(Boolean{value: false}).into(),
                branches: (
                    vec![
                        Await(vec![Memory(Id::from("m1"))]).into(),
                        Assignment{
                            target: Memory(Id::from("m0")),
                            value: FnCall {
                                fn_: Memory(Id::from("m1")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("m2")).into(),
                                ]
                            }.into(),
                        }.into(),
                    ],
                    vec![
                        Await(vec![Memory(Id::from("m1"))]).into(),
                        Assignment{
                            target: Memory(Id::from("m0")),
                            value: FnCall {
                                fn_: Memory(Id::from("m1")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("m4")).into(),
                                ]
                            }.into(),
                        }.into(),
                    ],
                )
            }.into(),
            Await(vec![Memory(Id::from("m1"))]).into(),
            Assignment{
                target: Memory(Id::from("m8")),
                value: FnCall {
                    fn_: Memory(Id::from("m1")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m7")).into(),
                    ]
                }.into(),
            }.into(),
        ],
        vec![
            Declaration{
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            IfStatement{
                condition: Value::from(Boolean{value: false}).into(),
                branches: (
                    vec![
                        Await(vec![Memory(Id::from("m1"))]).into(),
                        Assignment{
                            target: Memory(Id::from("m0")),
                            value: FnCall {
                                fn_: Memory(Id::from("m1")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("m2")).into(),
                                ]
                            }.into(),
                        }.into(),
                    ],
                    vec![
                        Await(vec![Memory(Id::from("m1"))]).into(),
                        Assignment{
                            target: Memory(Id::from("m0")),
                            value: FnCall {
                                fn_: Memory(Id::from("m1")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("m4")).into(),
                                ]
                            }.into(),
                        }.into(),
                    ],
                )
            }.into(),
            Assignment{
                target: Memory(Id::from("m8")),
                value: FnCall {
                    fn_: Memory(Id::from("m1")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m7")).into(),
                    ]
                }.into(),
            }.into(),
        ];
        "awaits in both if statement branches"
    )]
    #[test_case(
        vec![
            Declaration{
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            IfStatement{
                condition: Value::from(Boolean{value: false}).into(),
                branches: (
                    vec![
                        Await(vec![Memory(Id::from("m1"))]).into(),
                        Assignment{
                            target: Memory(Id::from("m0")),
                            value: FnCall {
                                fn_: Memory(Id::from("m1")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("m2")).into(),
                                ]
                            }.into(),
                        }.into(),
                    ],
                    vec![
                        Await(vec![Memory(Id::from("m4"))]).into(),
                        Assignment{
                            target: Memory(Id::from("m0")),
                            value: FnCall {
                                fn_: Memory(Id::from("m4")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("m5")).into(),
                                ]
                            }.into(),
                        }.into(),
                    ],
                )
            }.into(),
            Await(vec![Memory(Id::from("m1"))]).into(),
            Assignment{
                target: Memory(Id::from("m8")),
                value: FnCall {
                    fn_: Memory(Id::from("m1")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m7")).into(),
                    ]
                }.into(),
            }.into(),
        ],
        vec![
            Declaration{
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            IfStatement{
                condition: Value::from(Boolean{value: false}).into(),
                branches: (
                    vec![
                        Await(vec![Memory(Id::from("m1"))]).into(),
                        Assignment{
                            target: Memory(Id::from("m0")),
                            value: FnCall {
                                fn_: Memory(Id::from("m1")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("m2")).into(),
                                ]
                            }.into(),
                        }.into(),
                    ],
                    vec![
                        Await(vec![Memory(Id::from("m4"))]).into(),
                        Assignment{
                            target: Memory(Id::from("m0")),
                            value: FnCall {
                                fn_: Memory(Id::from("m4")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("m5")).into(),
                                ]
                            }.into(),
                        }.into(),
                    ],
                )
            }.into(),
            Await(vec![Memory(Id::from("m1"))]).into(),
            Assignment{
                target: Memory(Id::from("m8")),
                value: FnCall {
                    fn_: Memory(Id::from("m1")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("m7")).into(),
                    ]
                }.into(),
            }.into(),
        ];
        "awaits in single if statement branch"
    )]
    fn test_deduplicate_statements(
        duplicated_statements: Vec<Statement>,
        expected_statements: Vec<Statement>,
    ) {
        let mut deduplicator = AwaitDeduplicator::new();
        let deduplicated_statements = deduplicator.deduplicate_statements(duplicated_statements);
        assert_eq!(expected_statements, deduplicated_statements)
    }
}
