use std::collections::HashSet;

use itertools::Itertools;

use crate::{Await, FnDef, IfStatement, MatchBranch, MatchStatement, Memory, Program, Statement};

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
    /// Remove duplicate awaits from a program.
    fn deduplicate(program: Program) -> Program {
        let Program { type_defs, fn_defs } = program;
        let fn_defs = fn_defs
            .into_iter()
            .map(|fn_def| AwaitDeduplicator::new().deduplicate_fn_def(fn_def))
            .collect_vec();
        Program { type_defs, fn_defs }
    }
    fn deduplicate_fn_def(
        &mut self,
        FnDef {
            name,
            arguments,
            statements,
            ret,
            env,
            is_recursive,
            size_bounds,
        }: FnDef,
    ) -> FnDef {
        FnDef {
            name,
            arguments,
            statements: self.deduplicate_statements(statements),
            ret,
            env,
            is_recursive,
            size_bounds,
        }
    }
    fn deduplicate_statements(&mut self, statements: Vec<Statement>) -> Vec<Statement> {
        statements
            .into_iter()
            .filter_map(|statement| match statement {
                Statement::Await(await_) => self.deduplicate_await(await_).map(Statement::from),
                Statement::IfStatement(if_) => Some(self.deduplicate_if(if_).into()),
                Statement::MatchStatement(match_) => Some(self.deduplicate_match(match_).into()),
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
    fn deduplicate_match(
        &mut self,
        MatchStatement {
            expression,
            auxiliary_memory,
            branches,
        }: MatchStatement,
    ) -> MatchStatement {
        let mut awaited_ids = None;
        let branches = branches
            .into_iter()
            .map(|MatchBranch { target, statements }| {
                let mut deduplicator = self.clone();
                let statements = deduplicator.deduplicate_statements(statements);
                awaited_ids = match &awaited_ids {
                    None => Some(deduplicator.awaited_ids),
                    Some(ids) => Some(
                        deduplicator
                            .awaited_ids
                            .intersection(ids)
                            .cloned()
                            .collect(),
                    ),
                };
                MatchBranch { target, statements }
            })
            .collect_vec();
        self.awaited_ids = awaited_ids.unwrap_or_default();
        MatchStatement {
            expression,
            branches,
            auxiliary_memory,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Assignment, Await, Declaration, FnCall, FnDef, FnType, Id, IfStatement, MatchBranch,
        MatchStatement, Memory, Name, Program, Statement, TypeDef, UnionType, Value,
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
    #[test_case(
        vec![
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
                        Memory(Id::from("x0")).into(),
                    ]
                }.into(),
            }.into(),
            Declaration{
                memory: Memory(Id::from("shared")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            MatchStatement{
                expression: (
                    Memory(Id::from("subject")).into(),
                    UnionType(vec![Name::from("Left"), Name::from("Right")])
                ),
                auxiliary_memory: Memory(Id::from("aux")),
                branches: vec![
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Assignment{
                                target: Memory(Id::from("shared")),
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
                        ],
                    },
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment{
                                target: Memory(Id::from("shared")),
                                value: Value::from(Boolean{value: true}).into(),
                            }.into(),
                        ],
                    }
                ]
            }.into(),
        ],
        vec![
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
                        Memory(Id::from("x0")).into(),
                    ]
                }.into(),
            }.into(),
            Declaration{
                memory: Memory(Id::from("shared")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            MatchStatement{
                expression: (
                    Memory(Id::from("subject")).into(),
                    UnionType(vec![Name::from("Left"), Name::from("Right")])
                ),
                auxiliary_memory: Memory(Id::from("aux")),
                branches: vec![
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment{
                                target: Memory(Id::from("shared")),
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
                        ],
                    },
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment{
                                target: Memory(Id::from("shared")),
                                value: Value::from(Boolean{value: true}).into(),
                            }.into(),
                        ],
                    }
                ]
            }.into(),
        ];
        "await in match statement"
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
                    UnionType(vec![Name::from("Wrapper")])
                ),
                auxiliary_memory: Memory(Id::from("aux")),
                branches: vec![
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Assignment{
                                target: Memory(Id::from("shared")),
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
                        Memory(Id::from("x1")).into(),
                    ]
                }.into(),
            }.into(),
        ],
        vec![
            Declaration{
                memory: Memory(Id::from("shared")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            MatchStatement{
                expression: (
                    Memory(Id::from("subject")).into(),
                    UnionType(vec![Name::from("Wrapper")])
                ),
                auxiliary_memory: Memory(Id::from("aux")),
                branches: vec![
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Assignment{
                                target: Memory(Id::from("shared")),
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
                        ],
                    },
                ]
            }.into(),
            Assignment{
                target: Memory(Id::from("y")),
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
        ];
        "await in match statement single branch"
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
                                target: Memory(Id::from("shared")),
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
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("b"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Assignment{
                                target: Memory(Id::from("shared")),
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
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("c"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Assignment{
                                target: Memory(Id::from("shared")),
                                value: FnCall {
                                    fn_: Memory(Id::from("f")).into(),
                                    fn_type: FnType(
                                        vec![AtomicTypeEnum::INT.into()],
                                        Box::new(AtomicTypeEnum::BOOL.into())
                                    ),
                                    args: vec![
                                        Memory(Id::from("c")).into(),
                                    ]
                                }.into(),
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
                        Memory(Id::from("x")).into(),
                    ]
                }.into(),
            }.into(),
        ],
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
                                target: Memory(Id::from("shared")),
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
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("b"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Assignment{
                                target: Memory(Id::from("shared")),
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
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("c"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Assignment{
                                target: Memory(Id::from("shared")),
                                value: FnCall {
                                    fn_: Memory(Id::from("f")).into(),
                                    fn_type: FnType(
                                        vec![AtomicTypeEnum::INT.into()],
                                        Box::new(AtomicTypeEnum::BOOL.into())
                                    ),
                                    args: vec![
                                        Memory(Id::from("c")).into(),
                                    ]
                                }.into(),
                            }.into(),
                        ],
                    },
                ]
            }.into(),
            Assignment{
                target: Memory(Id::from("y")),
                value: FnCall {
                    fn_: Memory(Id::from("f")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("x")).into(),
                    ]
                }.into(),
            }.into(),
        ];
        "await in match statement all branches"
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
                                target: Memory(Id::from("shared")),
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
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("b"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Assignment{
                                target: Memory(Id::from("shared")),
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
                        Memory(Id::from("x")).into(),
                    ]
                }.into(),
            }.into(),
        ],
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
                                target: Memory(Id::from("shared")),
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
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("b"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Assignment{
                                target: Memory(Id::from("shared")),
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
                        Memory(Id::from("x")).into(),
                    ]
                }.into(),
            }.into(),
        ];
        "await in match statement multiple branches"
    )]
    fn test_deduplicate_statements(
        duplicated_statements: Vec<Statement>,
        expected_statements: Vec<Statement>,
    ) {
        let mut deduplicator = AwaitDeduplicator::new();
        let deduplicated_statements = deduplicator.deduplicate_statements(duplicated_statements);
        assert_eq!(expected_statements, deduplicated_statements)
    }

    #[test_case(
        Program{
            type_defs: vec![
                TypeDef{
                    name: Name::from("Empty"),
                    constructors: vec![(Name::from("empty"), None)]
                }
            ],
            fn_defs: vec![
                FnDef {
                    name: Name::from("F0"),
                    arguments: Vec::new(),
                    ret: (Memory(Id::from("m3")).into(), AtomicTypeEnum::BOOL.into()),
                    env: Vec::new(),
                    is_recursive: false,
                    size_bounds: (30, 30),
                    statements: vec![
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
                },
                FnDef {
                    name: Name::from("F1"),
                    arguments: Vec::new(),
                    ret: (Memory(Id::from("m3")).into(), AtomicTypeEnum::BOOL.into()),
                    env: Vec::new(),
                    is_recursive: false,
                    size_bounds: (10, 50),
                    statements: vec![
                        Await(vec![Memory(Id::from("m0"))]).into(),
                        Assignment{
                            target: Memory(Id::from("n2")),
                            value: FnCall {
                                fn_: Memory(Id::from("m0")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("n1")).into(),
                                ]
                            }.into(),
                        }.into(),
                        Declaration{
                            memory: Memory(Id::from("n3")),
                            type_: AtomicTypeEnum::BOOL.into()
                        }.into(),
                        IfStatement{
                            condition: Value::from(Boolean{value: false}).into(),
                            branches: (
                                vec![
                                    Await(vec![Memory(Id::from("m0"))]).into(),
                                    Assignment{
                                        target: Memory(Id::from("n5")),
                                        value: FnCall {
                                            fn_: Memory(Id::from("m0")).into(),
                                            fn_type: FnType(
                                                vec![AtomicTypeEnum::INT.into()],
                                                Box::new(AtomicTypeEnum::BOOL.into())
                                            ),
                                            args: vec![
                                                Memory(Id::from("n4")).into(),
                                            ]
                                        }.into(),
                                    }.into(),
                                    Assignment{
                                        target: Memory(Id::from("n3")),
                                        value: Memory(Id::from("n5")).into(),
                                    }.into()
                                ],
                                vec![
                                    Assignment{
                                        target: Memory(Id::from("n3")),
                                        value: Value::from(Boolean{value: true}).into(),
                                    }.into(),
                                ]
                            )
                        }.into(),
                    ],
                }
            ]
        },
        Program{
            type_defs: vec![
                TypeDef{
                    name: Name::from("Empty"),
                    constructors: vec![(Name::from("empty"), None)]
                }
            ],
            fn_defs: vec![
                FnDef {
                    name: Name::from("F0"),
                    arguments: Vec::new(),
                    ret: (Memory(Id::from("m3")).into(), AtomicTypeEnum::BOOL.into()),
                    env: Vec::new(),
                    is_recursive: false,
                    size_bounds: (30, 30),
                    statements: vec![
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
                    ],
                },
                FnDef {
                    name: Name::from("F1"),
                    arguments: Vec::new(),
                    ret: (Memory(Id::from("m3")).into(), AtomicTypeEnum::BOOL.into()),
                    env: Vec::new(),
                    is_recursive: false,
                    size_bounds: (10, 50),
                    statements: vec![
                        Await(vec![Memory(Id::from("m0"))]).into(),
                        Assignment{
                            target: Memory(Id::from("n2")),
                            value: FnCall {
                                fn_: Memory(Id::from("m0")).into(),
                                fn_type: FnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::BOOL.into())
                                ),
                                args: vec![
                                    Memory(Id::from("n1")).into(),
                                ]
                            }.into(),
                        }.into(),
                        Declaration{
                            memory: Memory(Id::from("n3")),
                            type_: AtomicTypeEnum::BOOL.into()
                        }.into(),
                        IfStatement{
                            condition: Value::from(Boolean{value: false}).into(),
                            branches: (
                                vec![
                                    Assignment{
                                        target: Memory(Id::from("n5")),
                                        value: FnCall {
                                            fn_: Memory(Id::from("m0")).into(),
                                            fn_type: FnType(
                                                vec![AtomicTypeEnum::INT.into()],
                                                Box::new(AtomicTypeEnum::BOOL.into())
                                            ),
                                            args: vec![
                                                Memory(Id::from("n4")).into(),
                                            ]
                                        }.into(),
                                    }.into(),
                                    Assignment{
                                        target: Memory(Id::from("n3")),
                                        value: Memory(Id::from("n5")).into(),
                                    }.into()
                                ],
                                vec![
                                    Assignment{
                                        target: Memory(Id::from("n3")),
                                        value: Value::from(Boolean{value: true}).into(),
                                    }.into(),
                                ]
                            )
                        }.into(),
                    ],
                }
            ]
        };
        "program"
    )]
    fn test_deduplicate_program(program: Program, expected_program: Program) {
        let deduplicated_program = AwaitDeduplicator::deduplicate(program);
        assert_eq!(expected_program, deduplicated_program)
    }
}
