use std::collections::{HashMap, HashSet};

use itertools::Either::{self, Left, Right};
use itertools::Itertools;

use crate::{Assignment, Await, Expression, FnCall, Memory, Statement, Value};

#[derive(Debug, Clone)]
struct StatementReorderer {
    fn_calls: HashSet<Memory>,
}

#[derive(Debug, Clone, PartialEq)]
struct Node {
    dependencies: Vec<Memory>,
    dependents: Vec<Memory>,
    is_fn: bool,
    awaits: Vec<Memory>,
    expression: Expression,
}

type Graph = HashMap<Memory, Node>;

impl StatementReorderer {
    fn new() -> Self {
        Self {
            fn_calls: HashSet::new(),
        }
    }

    /// Record fn calls in statements.
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

    /// Batch statements into blocks that can be independently reordered.
    fn batch_statements(
        &self,
        statements: Vec<Statement>,
    ) -> impl Iterator<Item = Either<Statement, Vec<Statement>>> {
        // Store the next batch.
        let mut next_batch: Option<Either<Statement, Vec<Statement>>> = None;
        statements.into_iter().batching(move |it| {
            // Iterate through the vector, returning the next complete batch.
            loop {
                match it.next() {
                    Some(statement) => match &statement {
                        Statement::Await(_) | Statement::Assignment(_)
                            if !matches!(
                                &statement,
                                Statement::Assignment(Assignment {
                                    target: _,
                                    value: Expression::ClosureInstantiation(_)
                                })
                            ) =>
                        {
                            let batch;
                            (batch, next_batch) = match next_batch.clone() {
                                Some(Left(stmt)) => {
                                    (Some(Left(stmt)), Some(Right(vec![statement])))
                                }
                                Some(Right(mut statements)) => {
                                    statements.push(statement);
                                    (None, Some(Right(statements)))
                                }
                                None => (None, Some(Right(vec![statement]))),
                            };
                            if batch.is_some() {
                                return batch;
                            }
                        }
                        _ => {
                            let batch = next_batch.clone();
                            next_batch = Some(Left(statement));
                            if batch.is_some() {
                                return batch;
                            }
                        }
                    },
                    None => {
                        let batch = next_batch.clone();
                        next_batch = None;
                        return batch;
                    }
                }
            }
        })
    }
    /// Construct dependency graph on subset of statements.
    fn construct_graph(&self, statements: Vec<Statement>) -> Graph {
        let mut graph = Graph::new();
        let mut last_await = None;
        for statement in statements {
            match statement {
                Statement::Await(Await(memory)) => last_await = Some(memory),
                Statement::Assignment(Assignment { target, value }) => {
                    let node = Node {
                        dependencies: value
                            .values()
                            .iter()
                            .filter_map(Value::filter_memory)
                            .collect_vec(),
                        dependents: Vec::new(),
                        is_fn: self.fn_calls.contains(&target),
                        awaits: {
                            if let Some(awaits) = last_await {
                                last_await = None;
                                awaits
                            } else {
                                Vec::new()
                            }
                        },
                        expression: value,
                    };
                    graph.insert(target, node);
                }
                _ => panic!(),
            }
        }
        for (memory, node) in graph.clone().into_iter() {
            for dependency in &node.dependencies {
                if let Some(dependency) = graph.get_mut(dependency) {
                    dependency.dependents.push(memory.clone());
                }
            }
        }
        graph
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Assignment, Await, BuiltIn, ClosureInstantiation, Declaration, FnCall, FnType, Id,
        IfStatement, MatchBranch, MatchStatement, Name, Statement, TupleExpression, UnionType,
    };

    use super::*;

    use itertools::Either;
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
                target: Memory(Id::from("t3")),
                value: FnCall {
                    fn_: BuiltIn::BuiltInFn(Name::from("Increment__BuiltIn")).into(),
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

    #[test_case(
        vec!["t1", "t2"],
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
            Await(vec![Memory(Id::from("t1")), Memory(Id::from("t2"))]).into(),
            Assignment{
                target: Memory(Id::from("t3")),
                value: FnCall {
                    fn_: BuiltIn::BuiltInFn(Name::from("Plus__BuiltIn")).into(),
                    fn_type: FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ),
                    args: vec![
                        Memory(Id::from("t1")).into(),
                        Memory(Id::from("t2")).into(),
                    ]
                }.into(),
            }.into()
        ],
        HashMap::from([
            (
                Memory(Id::from("t1")),
                Node{
                    dependencies: vec![Memory(Id::from("f")),Memory(Id::from("m1"))],
                    dependents: vec![Memory(Id::from("t3"))],
                    expression: FnCall{
                        fn_: Memory(Id::from("f")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::BOOL.into())
                        ),
                        args: vec![
                            Memory(Id::from("m1")).into(),
                        ]
                    }.into(),
                    is_fn: true,
                    awaits: vec![Memory(Id::from("f"))],
                }
            ),
            (
                Memory(Id::from("t2")),
                Node{
                    dependencies: vec![Memory(Id::from("f")),Memory(Id::from("m2"))],
                    dependents: vec![Memory(Id::from("t3"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("f")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::BOOL.into())
                        ),
                        args: vec![
                            Memory(Id::from("m2")).into(),
                        ]
                    }.into(),
                    is_fn: true,
                    awaits: vec![Memory(Id::from("f"))],
                }
            ),
            (
                Memory(Id::from("t3")),
                Node{
                    dependencies: vec![Memory(Id::from("t1")),Memory(Id::from("t2"))],
                    dependents: Vec::new(),
                    expression: FnCall {
                        fn_: BuiltIn::BuiltInFn(Name::from("Plus__BuiltIn")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::BOOL.into())
                        ),
                        args: vec![
                            Memory(Id::from("t1")).into(),
                            Memory(Id::from("t2")).into(),
                        ]
                    }.into(),
                    is_fn: false,
                    awaits: vec![Memory(Id::from("t1")),Memory(Id::from("t2"))],
                }
            ),
        ]);
        "statements with addition"
    )]
    fn test_construct_graph(
        fn_calls: Vec<&str>,
        statements: Vec<Statement>,
        expected_graph: Graph,
    ) {
        let fn_calls = fn_calls
            .into_iter()
            .map(|id| Memory(Id::from(id)))
            .collect::<HashSet<_>>();
        let mut reorderer = StatementReorderer::new();
        reorderer.fn_calls = fn_calls;
        let graph = reorderer.construct_graph(statements);
        assert_eq!(expected_graph, graph);
    }

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
        ],
        vec![Right(vec![
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
        ])];
        "flat batches"
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
            Await(vec![Memory(Id::from("condition"))]).into(),
            IfStatement{
                condition: Memory(Id::from("condition")).into(),
                branches: (
                    Vec::new(),
                    Vec::new(),
                )
            }.into(),
        ],
        vec![
            Right(vec![
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
            ]),
            Left(
                Declaration{
                    memory: Memory(Id::from("shared")),
                    type_: AtomicTypeEnum::BOOL.into()
                }.into(),
            ),
            Right(vec![
                Await(vec![Memory(Id::from("condition"))]).into(),
            ]),
            Left(IfStatement{
                condition: Memory(Id::from("condition")).into(),
                branches: (
                    Vec::new(),
                    Vec::new(),
                )
            }.into()),
        ];
        "if statement batches"
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
                        statements: Vec::new(),
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("b"))),
                        statements: Vec::new(),
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("c"))),
                        statements: Vec::new(),
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
            Left(Declaration{
                memory: Memory(Id::from("shared")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into()),
            Left(MatchStatement{
                expression: (
                    Memory(Id::from("subject")).into(),
                    UnionType(vec![Name::from("A"), Name::from("B"), Name::from("C")])
                ),
                auxiliary_memory: Memory(Id::from("aux")),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("a"))),
                        statements: Vec::new(),
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("b"))),
                        statements: Vec::new(),
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("c"))),
                        statements: Vec::new(),
                    },
                ]
            }.into()),
            Right(vec![
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
            ])
        ];
        "match statement batches"
    )]
    #[test_case(
        vec![
            Declaration {
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                ).into(),
                memory: Memory(Id::from("m1"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: ClosureInstantiation { name: Name::from("F0"), env: None }.into(),
            }.into(),
            Assignment {
                target: Memory(Id::from("m4")),
                value: TupleExpression(vec![Memory(Id::from("m1")).into()]).into(),
            }.into(),
            Declaration {
                type_: FnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
                memory: Memory(Id::from("m5"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m5")),
                value: ClosureInstantiation {
                    name: Name::from("F1"),
                    env: Some(Memory(Id::from("m4")).into())
                }.into(),
            }.into(),
            Await(vec![Memory(Id::from("m5"))]).into(),
            Assignment {
                target: Memory(Id::from("m6")),
                value: FnCall {
                    fn_: Memory(Id::from("m5")).into(),
                    fn_type: FnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
                    args: Vec::new()
                }.into(),
            }.into()
        ],
        vec![
            Left(Declaration {
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                ).into(),
                memory: Memory(Id::from("m1"))
            }.into()),
            Left(Assignment {
                target: Memory(Id::from("m1")),
                value: ClosureInstantiation { name: Name::from("F0"), env: None }.into(),
            }.into()),
            Right(vec![
                Assignment {
                    target: Memory(Id::from("m4")),
                    value: TupleExpression(vec![Memory(Id::from("m1")).into()]).into(),
                }.into()
            ]),
            Left(Declaration {
                type_: FnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
                memory: Memory(Id::from("m5"))
            }.into()),
            Left(Assignment {
                target: Memory(Id::from("m5")),
                value: ClosureInstantiation {
                    name: Name::from("F1"),
                    env: Some(Memory(Id::from("m4")).into())
                }.into(),
            }.into()),
            Right(vec![
                Await(vec![Memory(Id::from("m5"))]).into(),
                Assignment {
                    target: Memory(Id::from("m6")),
                    value: FnCall {
                        fn_: Memory(Id::from("m5")).into(),
                        fn_type: FnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into(),
                        args: Vec::new()
                    }.into(),
                }.into()
            ])
        ];
        "closure instantiation"
    )]
    fn test_batch_statements(
        statements: Vec<Statement>,
        expected_batches: Vec<Either<Statement, Vec<Statement>>>,
    ) {
        let reorderer = StatementReorderer::new();
        let batches = reorderer.batch_statements(statements).collect_vec();
        assert_eq!(expected_batches, batches);
        for batch in batches {
            if let Right(statements) = batch {
                reorderer.construct_graph(statements);
            }
        }
    }
}
