use std::collections::{HashMap, HashSet};

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
}
