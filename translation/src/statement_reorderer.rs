use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use itertools::Either::{self, Left, Right};
use itertools::Itertools;

use crate::{
    Assignment, Await, Expression, FnCall, IfStatement, MatchBranch, MatchStatement, Memory,
    Program, Statement, Value,
};

#[derive(Debug, Clone, PartialEq)]
struct Node {
    dependencies: HashSet<Memory>,
    dependents: Vec<Memory>,
    is_fn: bool,
    awaits: Vec<Memory>,
    memory: Memory,
    expression: Expression,
    fn_dependents: Option<usize>,
    fns_used: Option<usize>,
}

impl Node {
    /// Comparable heuristics for determining order (higher is better).
    fn project_heuristic(&self) -> (bool, usize, bool) {
        (
            self.is_fn,
            self.fn_dependents.unwrap(),
            self.fns_used.unwrap() == 0,
        )
    }
    /// Convert back to statements.
    fn to_statements(self) -> Vec<Statement> {
        let assignment = Assignment {
            value: self.expression,
            target: self.memory,
        };
        if self.awaits.len() == 0 {
            vec![assignment.into()]
        } else {
            let await_ = Await(self.awaits);
            vec![await_.into(), assignment.into()]
        }
    }
}

#[derive(Debug, Clone)]
struct HeuristicNode(pub Node);

impl PartialEq for HeuristicNode {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for HeuristicNode {}

impl PartialOrd for HeuristicNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeuristicNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.project_heuristic().cmp(&other.0.project_heuristic())
    }
}

type Graph = HashMap<Memory, Node>;

#[derive(Debug, Clone)]
pub struct StatementReorderer {
    fn_calls: HashSet<Memory>,
}

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
                                if let Some(Right(statements)) = &batch {
                                    if statements.len() == 1 {
                                        return Some(Left(statements[0].clone()));
                                    }
                                }
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
                            .collect(),
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
                        memory: target.clone(),
                        expression: value,
                        fn_dependents: None,
                        fns_used: None,
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

    fn compute_fn_dependents(&self, mut graph: Graph) -> Graph {
        for memory in graph.clone().keys() {
            self.compute_node_fn_dependents(&memory, &mut graph);
        }
        graph
    }
    fn compute_node_fn_dependents(&self, memory: &Memory, graph: &mut Graph) -> usize {
        if let Some(count) = graph.get(memory).unwrap().fn_dependents {
            return count;
        }

        let dependents = graph.get(memory).unwrap().dependents.clone();
        let is_fn = graph.get(memory).unwrap().is_fn;

        let mut fn_dependents = dependents
            .iter()
            .map(|memory| self.compute_node_fn_dependents(memory, graph))
            .sum();

        if is_fn {
            fn_dependents += 1;
        }

        graph.get_mut(memory).unwrap().fn_dependents = Some(fn_dependents);
        fn_dependents
    }

    fn compute_fns_used(&self, mut graph: Graph) -> Graph {
        for node in graph.values_mut() {
            node.fns_used = Some(
                node.dependencies
                    .iter()
                    .map(|dependency| {
                        if self.fn_calls.contains(dependency) {
                            1
                        } else {
                            0
                        }
                    })
                    .sum(),
            );
        }
        graph
    }

    /// Order nodes in the graph based on heuristics.
    fn find_order(&self, graph: Graph) -> Vec<Node> {
        let keys = HashSet::<Memory>::from_iter(graph.keys().cloned());
        let mut graph = graph;
        let mut free_nodes = BinaryHeap::new();
        // Only keep dependencies from the graph.
        for node in graph.values_mut() {
            node.dependencies = node
                .dependencies
                .iter()
                .filter(|&dependency| keys.contains(dependency))
                .cloned()
                .collect();
            if node.dependencies.len() == 0 {
                free_nodes.push(HeuristicNode(node.clone()));
            }
        }
        let mut order = Vec::new();
        // Pick next best statement.
        while let Some(HeuristicNode(node)) = free_nodes.pop() {
            for dependent in &node.dependents {
                let neighbor = graph.get_mut(dependent).unwrap();
                neighbor.dependencies.remove(&node.memory);
                if neighbor.dependencies.len() == 0 {
                    free_nodes.push(HeuristicNode(neighbor.clone()));
                }
            }
            order.push(node);
        }
        order
    }

    fn reorder_statements(&mut self, statements: Vec<Statement>) -> Vec<Statement> {
        self.collect_fn_calls(&statements);
        let statements = self
            .batch_statements(statements)
            .flat_map(|batch| match batch {
                Left(Statement::IfStatement(IfStatement {
                    condition,
                    branches: (true_branch, false_branch),
                })) => {
                    vec![IfStatement {
                        condition,
                        branches: (
                            self.reorder_statements(true_branch),
                            self.reorder_statements(false_branch),
                        ),
                    }
                    .into()]
                }
                Left(Statement::MatchStatement(MatchStatement {
                    expression,
                    branches,
                    auxiliary_memory,
                })) => {
                    vec![MatchStatement {
                        expression,
                        branches: branches
                            .into_iter()
                            .map(|MatchBranch { target, statements }| MatchBranch {
                                target,
                                statements: self.reorder_statements(statements),
                            })
                            .collect_vec(),
                        auxiliary_memory,
                    }
                    .into()]
                }
                Left(statement) => vec![statement],
                Right(statements) => {
                    let graph = self.construct_graph(statements);
                    let graph = self.compute_fn_dependents(graph);
                    let graph = self.compute_fns_used(graph);
                    let order = self.find_order(graph);
                    order
                        .into_iter()
                        .flat_map(Node::to_statements)
                        .collect_vec()
                }
            })
            .collect_vec();
        statements
    }
    /// Reorder statements in a program.
    pub fn reorder(mut program: Program) -> Program {
        for fn_def in program.fn_defs.iter_mut() {
            fn_def.statements =
                StatementReorderer::new().reorder_statements(fn_def.statements.clone());
        }
        program
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Assignment, Await, BuiltIn, ClosureInstantiation, Declaration, FnCall, FnDef, FnType, Id,
        IfStatement, MatchBranch, MatchStatement, Name, Statement, TupleExpression, TypeDef,
        UnionType,
    };

    use super::*;

    use itertools::Either;
    use lowering::{AtomicTypeEnum, Boolean, Integer};
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
                    dependencies: HashSet::from([Memory(Id::from("f")),Memory(Id::from("m1"))]),
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
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("t1")),
                }
            ),
            (
                Memory(Id::from("t2")),
                Node{
                    dependencies: HashSet::from([Memory(Id::from("f")),Memory(Id::from("m2"))]),
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
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("t2")),
                }
            ),
            (
                Memory(Id::from("t3")),
                Node{
                    dependencies: HashSet::from([Memory(Id::from("t1")),Memory(Id::from("t2"))]),
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
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("t3")),
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
            Left(
                Await(vec![Memory(Id::from("condition"))]).into(),
            ),
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
            Left(
                Assignment {
                    target: Memory(Id::from("m4")),
                    value: TupleExpression(vec![Memory(Id::from("m1")).into()]).into(),
                }.into()
            ),
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

    #[test_case(
        HashMap::from([
            (
                Memory(Id::from("a")),
                Node {
                    dependencies: HashSet::new(),
                    dependents: vec![
                        Memory(Id::from("f")),
                        Memory(Id::from("g")),
                    ],
                    is_fn: false,
                    awaits: Vec::new(),
                    expression: Value::from(Integer {value: 0}).into(),
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("a")),
                }
            ),
            (
                Memory(Id::from("f")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("a")),
                    ]),
                    dependents: vec![
                        Memory(Id::from("p")),
                        Memory(Id::from("m")),
                    ],
                    is_fn: true,
                    awaits: vec![Memory(Id::from("F"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("F")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("a")).into(),
                        ]
                    }.into(),
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("f")),
                }
            ),
            (
                Memory(Id::from("g")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("a")),
                    ]),
                    dependents: vec![
                        Memory(Id::from("p")),
                        Memory(Id::from("m")),
                    ],
                    is_fn: true,
                    awaits: vec![Memory(Id::from("G"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("G")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("a")).into(),
                        ]
                    }.into(),
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("g")),
                }
            ),
            (
                Memory(Id::from("p")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("f")),
                        Memory(Id::from("g")),
                    ]),
                    dependents: Vec::new(),
                    is_fn: false,
                    awaits: vec![Memory(Id::from("f")),Memory(Id::from("g"))],
                    expression: FnCall {
                        fn_: BuiltIn::BuiltInFn(Name::from("+")).into(),
                        fn_type: FnType(
                            vec![
                                AtomicTypeEnum::INT.into(),
                                AtomicTypeEnum::INT.into(),
                            ],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("f")).into(),
                            Memory(Id::from("g")).into(),
                        ]
                    }.into(),
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("p")),
                }
            ),
            (
                Memory(Id::from("m")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("f")),
                        Memory(Id::from("g")),
                    ]),
                    dependents: Vec::new(),
                    is_fn: false,
                    awaits: vec![Memory(Id::from("f")),Memory(Id::from("g"))],
                    expression: FnCall {
                        fn_: BuiltIn::BuiltInFn(Name::from("-")).into(),
                        fn_type: FnType(
                            vec![
                                AtomicTypeEnum::INT.into(),
                                AtomicTypeEnum::INT.into(),
                            ],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("f")).into(),
                            Memory(Id::from("g")).into(),
                        ]
                    }.into(),
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("m")),
                }
            ),
        ]),
        HashMap::from([
            (Id::from("a"), 2),
            (Id::from("f"), 1),
            (Id::from("g"), 1),
            (Id::from("p"), 0),
            (Id::from("m"), 0),
        ]);
        "two independent fns"
    )]
    #[test_case(
        HashMap::from([
            (
                Memory(Id::from("a")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("p")),
                        Memory(Id::from("A")),
                    ]),
                    dependents: vec![
                        Memory(Id::from("f")),
                    ],
                    is_fn: true,
                    awaits: vec![Memory(Id::from("A"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("A")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("p")).into(),
                        ]
                    }.into(),
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("a")),
                }
            ),
            (
                Memory(Id::from("f")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("a")),
                        Memory(Id::from("F")),
                    ]),
                    dependents: vec![
                        Memory(Id::from("g")),
                    ],
                    is_fn: true,
                    awaits: vec![Memory(Id::from("F"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("F")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("a")).into(),
                        ]
                    }.into(),
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("f")),
                }
            ),
            (
                Memory(Id::from("g")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("a")),
                    ]),
                    dependents: Vec::new(),
                    is_fn: true,
                    awaits: vec![Memory(Id::from("G"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("G")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("f")).into(),
                        ]
                    }.into(),
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("g")),
                }
            ),
        ]),
        HashMap::from([
            (Id::from("a"), 3),
            (Id::from("f"), 2),
            (Id::from("g"), 1),
        ]);
        "line dependencies"
    )]
    fn test_compute_fn_dependents(graph: Graph, expected_dependents: HashMap<Id, usize>) {
        let reorderer = StatementReorderer::new();
        let graph = reorderer.compute_fn_dependents(graph);
        assert_eq!(
            expected_dependents,
            graph
                .into_iter()
                .map(|(Memory(id), node)| (id, node.fn_dependents.unwrap()))
                .collect()
        );
    }

    #[test_case(
        vec!["p", "a", "f"],
        HashMap::from([
            (
                Memory(Id::from("a")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("p")),
                        Memory(Id::from("A")),
                    ]),
                    dependents: vec![
                        Memory(Id::from("f")),
                    ],
                    is_fn: true,
                    awaits: vec![Memory(Id::from("A"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("A")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("p")).into(),
                        ]
                    }.into(),
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("a")),
                }
            ),
            (
                Memory(Id::from("f")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("a")),
                        Memory(Id::from("F")),
                    ]),
                    dependents: vec![
                        Memory(Id::from("g")),
                    ],
                    is_fn: true,
                    awaits: vec![Memory(Id::from("F"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("F")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("a")).into(),
                        ]
                    }.into(),
                    fn_dependents: None,
                    fns_used: None,
                    memory: Memory(Id::from("f")),
                }
            ),
        ]),
        HashMap::from([
            (Id::from("a"), 1),
            (Id::from("f"), 1),
        ]);
        "consecutive fn use"
    )]
    #[test_case(
        vec!["a", "b"],
        HashMap::from([
            (
                Memory(Id::from("r")),
                Node {
                    memory: Memory(Id::from("r")),
                    dependencies: HashSet::from([
                        Memory(Id::from("a")),
                        Memory(Id::from("b")),
                    ]),
                    dependents: Vec::new(),
                    is_fn: false,
                    awaits: vec![Memory(Id::from("A"))],
                    expression: FnCall {
                        fn_: BuiltIn::BuiltInFn(Name::from("*")).into(),
                        fn_type: FnType(
                            vec![
                                AtomicTypeEnum::INT.into(),
                                AtomicTypeEnum::INT.into(),
                            ],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("a")).into(),
                            Memory(Id::from("b")).into(),
                        ]
                    }.into(),
                    fn_dependents: None,
                    fns_used: None,
                }
            ),
        ]),
        HashMap::from([
            (Id::from("r"), 2),
        ]);
        "double fn use"
    )]
    fn test_compute_fns_used(fns: Vec<&str>, graph: Graph, expected_fns_used: HashMap<Id, usize>) {
        let mut reorderer = StatementReorderer::new();
        reorderer.fn_calls = HashSet::from_iter(fns.into_iter().map(|id| Memory(Id::from(id))));
        let graph = reorderer.compute_fns_used(graph);
        assert_eq!(
            expected_fns_used,
            graph
                .into_iter()
                .map(|(Memory(id), node)| (id, node.fns_used.unwrap()))
                .collect()
        );
    }

    #[test_case(
        HashMap::from([
            (
                Memory(Id::from("a")),
                Node {
                    memory: Memory(Id::from("a")),
                    dependencies: HashSet::from([
                        Memory(Id::from("x")),
                        Memory(Id::from("F")),
                    ]),
                    dependents: Vec::new(),
                    is_fn: false,
                    awaits: vec![Memory(Id::from("F"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("F")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("x")).into(),
                        ]
                    }.into(),
                    fn_dependents: Some(0),
                    fns_used: Some(1),
                }
            ),
            (
                Memory(Id::from("b")),
                Node {
                    memory: Memory(Id::from("b")),
                    dependencies: HashSet::from([
                        Memory(Id::from("y")),
                        Memory(Id::from("F")),
                    ]),
                    dependents: Vec::new(),
                    is_fn: false,
                    awaits: vec![Memory(Id::from("F"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("F")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("y")).into(),
                        ]
                    }.into(),
                    fn_dependents: Some(1),
                    fns_used: Some(1),
                }
            ),
        ]),
        vec![vec!["b","a"]];
        "dependents preference"
    )]
    #[test_case(
        HashMap::from([
            (
                Memory(Id::from("a")),
                Node {
                    memory: Memory(Id::from("a")),
                    dependencies: HashSet::from([
                        Memory(Id::from("x")),
                        Memory(Id::from("F")),
                    ]),
                    dependents: Vec::new(),
                    is_fn: true,
                    awaits: vec![Memory(Id::from("F"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("F")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("x")).into(),
                        ]
                    }.into(),
                    fn_dependents: Some(1),
                    fns_used: Some(1),
                }
            ),
            (
                Memory(Id::from("b")),
                Node {
                    memory: Memory(Id::from("b")),
                    dependencies: HashSet::from([
                        Memory(Id::from("y")),
                        Memory(Id::from("F")),
                    ]),
                    dependents: Vec::new(),
                    is_fn: true,
                    awaits: vec![Memory(Id::from("F"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("F")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("y")).into(),
                        ]
                    }.into(),
                    fn_dependents: Some(1),
                    fns_used: Some(0),
                }
            ),
        ]),
        vec![vec!["b","a"]];
        "fn used preference tie breaker"
    )]
    #[test_case(
        HashMap::from([
            (
                Memory(Id::from("a")),
                Node {
                    memory: Memory(Id::from("a")),
                    dependencies: HashSet::from([
                        Memory(Id::from("x")),
                    ]),
                    dependents: vec![
                        Memory(Id::from("b"))
                    ],
                    is_fn: true,
                    awaits: Vec::new(),
                    expression: FnCall {
                        fn_: BuiltIn::BuiltInFn(Name::from("++")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("x")).into(),
                        ]
                    }.into(),
                    fn_dependents: Some(0),
                    fns_used: Some(0),
                }
            ),
            (
                Memory(Id::from("b")),
                Node {
                    memory: Memory(Id::from("b")),
                    dependencies: HashSet::from([
                        Memory(Id::from("a")),
                    ]),
                    dependents: Vec::new(),
                    is_fn: true,
                    awaits: Vec::new(),
                    expression: FnCall {
                        fn_: BuiltIn::BuiltInFn(Name::from("++")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("a")).into(),
                        ]
                    }.into(),
                    fn_dependents: Some(0),
                    fns_used: Some(0),
                }
            ),
        ]),
        vec![vec!["a","b"]];
        "dependency respected"
    )]
    #[test_case(
        HashMap::from([
            (
                Memory(Id::from("a")),
                Node {
                    dependencies: HashSet::new(),
                    dependents: vec![
                        Memory(Id::from("f")),
                        Memory(Id::from("g")),
                    ],
                    is_fn: false,
                    awaits: Vec::new(),
                    expression: Value::from(Integer {value: 0}).into(),
                    fn_dependents: Some(2),
                    fns_used: Some(0),
                    memory: Memory(Id::from("a")),
                }
            ),
            (
                Memory(Id::from("f")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("a")),
                    ]),
                    dependents: vec![
                        Memory(Id::from("p")),
                        Memory(Id::from("m")),
                    ],
                    is_fn: true,
                    awaits: vec![Memory(Id::from("F"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("F")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("a")).into(),
                        ]
                    }.into(),
                    fn_dependents: Some(1),
                    fns_used: Some(0),
                    memory: Memory(Id::from("f")),
                }
            ),
            (
                Memory(Id::from("g")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("a")),
                    ]),
                    dependents: vec![
                        Memory(Id::from("p")),
                        Memory(Id::from("m")),
                    ],
                    is_fn: true,
                    awaits: vec![Memory(Id::from("G"))],
                    expression: FnCall {
                        fn_: Memory(Id::from("G")).into(),
                        fn_type: FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("a")).into(),
                        ]
                    }.into(),
                    fn_dependents: Some(1),
                    fns_used: Some(0),
                    memory: Memory(Id::from("g")),
                }
            ),
            (
                Memory(Id::from("p")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("f")),
                        Memory(Id::from("g")),
                    ]),
                    dependents: Vec::new(),
                    is_fn: false,
                    awaits: vec![Memory(Id::from("f")),Memory(Id::from("g"))],
                    expression: FnCall {
                        fn_: BuiltIn::BuiltInFn(Name::from("+")).into(),
                        fn_type: FnType(
                            vec![
                                AtomicTypeEnum::INT.into(),
                                AtomicTypeEnum::INT.into(),
                            ],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("f")).into(),
                            Memory(Id::from("g")).into(),
                        ]
                    }.into(),
                    fn_dependents: Some(0),
                    fns_used: Some(2),
                    memory: Memory(Id::from("p")),
                }
            ),
            (
                Memory(Id::from("m")),
                Node {
                    dependencies: HashSet::from([
                        Memory(Id::from("f")),
                        Memory(Id::from("g")),
                    ]),
                    dependents: Vec::new(),
                    is_fn: false,
                    awaits: vec![Memory(Id::from("f")),Memory(Id::from("g"))],
                    expression: FnCall {
                        fn_: BuiltIn::BuiltInFn(Name::from("-")).into(),
                        fn_type: FnType(
                            vec![
                                AtomicTypeEnum::INT.into(),
                                AtomicTypeEnum::INT.into(),
                            ],
                            Box::new(AtomicTypeEnum::INT.into())
                        ),
                        args: vec![
                            Memory(Id::from("f")).into(),
                            Memory(Id::from("g")).into(),
                        ]
                    }.into(),
                    fn_dependents: Some(0),
                    fns_used: Some(2),
                    memory: Memory(Id::from("m")),
                }
            ),
        ]),
        vec![
            vec!["a","f","g","p","m"],
            vec!["a","f","g","m","p"],
            vec!["a","g","f","p","m"],
            vec!["a","g","f","m","p"],
        ];
        "two independent fns"
    )]
    fn test_node_reorder(graph: Graph, possible_orderings: Vec<Vec<&str>>) {
        let reorderer = StatementReorderer::new();
        let ordering = reorderer.find_order(graph);
        let possible_orderings = possible_orderings
            .into_iter()
            .map(|ordering| {
                ordering
                    .into_iter()
                    .map(|id| Memory(Id::from(id)))
                    .collect_vec()
            })
            .collect::<HashSet<Vec<Memory>>>();
        let ordering = ordering.into_iter().map(|node| node.memory).collect_vec();
        dbg!(&ordering);
        assert!(possible_orderings.contains(&ordering));
    }

    #[test_case(
        Program{
            type_defs: Vec::new(),
            fn_defs: vec![
                FnDef {
                    name: Name::from("F0"),
                    arguments: Vec::new(),
                    ret: (Memory(Id::from("result")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    is_recursive: false,
                    size_bounds: (30, 50),
                    statements: vec![
                        Await(vec![Memory(Id::from("condition"))]).into(),
                        Declaration{
                            memory: Memory(Id::from("result")),
                            type_: AtomicTypeEnum::INT.into()
                        }.into(),
                        IfStatement{
                            condition: Memory(Id::from("condition")).into(),
                            branches: (
                                vec![
                                    Await(vec![Memory(Id::from("f"))]).into(),
                                    Assignment{
                                        target: Memory(Id::from("y")),
                                        value: FnCall {
                                            fn_: Memory(Id::from("f")).into(),
                                            fn_type: FnType(
                                                vec![AtomicTypeEnum::INT.into()],
                                                Box::new(AtomicTypeEnum::INT.into())
                                            ),
                                            args: vec![
                                                Integer{value: 5}.into()
                                            ]
                                        }.into(),
                                    }.into(),
                                    Assignment{
                                        target: Memory(Id::from("result")),
                                        value: FnCall {
                                            fn_: BuiltIn::BuiltInFn(Name::from("++")).into(),
                                            fn_type: FnType(
                                                vec![AtomicTypeEnum::INT.into()],
                                                Box::new(AtomicTypeEnum::INT.into())
                                            ),
                                            args: vec![
                                                Memory(Id::from("y")).into()
                                            ]
                                        }.into(),
                                    }.into(),
                                    Await(vec![Memory(Id::from("f"))]).into(),
                                    Assignment{
                                        target: Memory(Id::from("_")),
                                        value: FnCall {
                                            fn_: Memory(Id::from("f")).into(),
                                            fn_type: FnType(
                                                vec![AtomicTypeEnum::INT.into()],
                                                Box::new(AtomicTypeEnum::INT.into())
                                            ),
                                            args: vec![
                                                Memory(Id::from("y")).into()
                                            ]
                                        }.into(),
                                    }.into(),
                                ],
                                vec![
                                    Assignment{
                                        target: Memory(Id::from("result")),
                                        value: Value::from(Integer{value: 0}).into(),
                                    }.into(),
                                ]
                            )
                        }.into(),
                    ]
                }
            ]
        },
        Program{
            type_defs: Vec::new(),
            fn_defs: vec![
                FnDef {
                    name: Name::from("F0"),
                    arguments: Vec::new(),
                    ret: (Memory(Id::from("result")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    is_recursive: false,
                    size_bounds: (30, 50),
                    statements: vec![
                        Await(vec![Memory(Id::from("condition"))]).into(),
                        Declaration{
                            memory: Memory(Id::from("result")),
                            type_: AtomicTypeEnum::INT.into()
                        }.into(),
                        IfStatement{
                            condition: Memory(Id::from("condition")).into(),
                            branches: (
                                vec![
                                    Await(vec![Memory(Id::from("f"))]).into(),
                                    Assignment{
                                        target: Memory(Id::from("y")),
                                        value: FnCall {
                                            fn_: Memory(Id::from("f")).into(),
                                            fn_type: FnType(
                                                vec![AtomicTypeEnum::INT.into()],
                                                Box::new(AtomicTypeEnum::INT.into())
                                            ),
                                            args: vec![
                                                Integer{value: 5}.into()
                                            ]
                                        }.into(),
                                    }.into(),
                                    Await(vec![Memory(Id::from("f"))]).into(),
                                    Assignment{
                                        target: Memory(Id::from("_")),
                                        value: FnCall {
                                            fn_: Memory(Id::from("f")).into(),
                                            fn_type: FnType(
                                                vec![AtomicTypeEnum::INT.into()],
                                                Box::new(AtomicTypeEnum::INT.into())
                                            ),
                                            args: vec![
                                                Memory(Id::from("y")).into()
                                            ]
                                        }.into(),
                                    }.into(),
                                    Assignment{
                                        target: Memory(Id::from("result")),
                                        value: FnCall {
                                            fn_: BuiltIn::BuiltInFn(Name::from("++")).into(),
                                            fn_type: FnType(
                                                vec![AtomicTypeEnum::INT.into()],
                                                Box::new(AtomicTypeEnum::INT.into())
                                            ),
                                            args: vec![
                                                Memory(Id::from("y")).into()
                                            ]
                                        }.into(),
                                    }.into(),
                                ],
                                vec![
                                    Assignment{
                                        target: Memory(Id::from("result")),
                                        value: Value::from(Integer{value: 0}).into(),
                                    }.into(),
                                ]
                            )
                        }.into(),
                    ]
                }
            ]
        };
        "program with if"
    )]
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
                    ret: (Memory(Id::from("result")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    is_recursive: false,
                    size_bounds: (30, 50),
                    statements: vec![
                        Await(vec![Memory(Id::from("subject"))]).into(),
                        Declaration{
                            memory: Memory(Id::from("result")),
                            type_: AtomicTypeEnum::INT.into()
                        }.into(),
                        MatchStatement{
                            expression: (Memory(Id::from("subject")).into(), UnionType(vec![Name::from("empty")])),
                            auxiliary_memory: Memory(Id::from("aux")),
                            branches: vec![
                                MatchBranch {
                                    target: None,
                                    statements: vec![
                                        Await(vec![Memory(Id::from("f"))]).into(),
                                        Assignment{
                                            target: Memory(Id::from("y")),
                                            value: FnCall {
                                                fn_: Memory(Id::from("f")).into(),
                                                fn_type: FnType(
                                                    vec![AtomicTypeEnum::INT.into()],
                                                    Box::new(AtomicTypeEnum::INT.into())
                                                ),
                                                args: vec![
                                                    Integer{value: 5}.into()
                                                ]
                                            }.into(),
                                        }.into(),
                                        Assignment{
                                            target: Memory(Id::from("result")),
                                            value: FnCall {
                                                fn_: BuiltIn::BuiltInFn(Name::from("++")).into(),
                                                fn_type: FnType(
                                                    vec![AtomicTypeEnum::INT.into()],
                                                    Box::new(AtomicTypeEnum::INT.into())
                                                ),
                                                args: vec![
                                                    Memory(Id::from("y")).into()
                                                ]
                                            }.into(),
                                        }.into(),
                                        Assignment{
                                            target: Memory(Id::from("_")),
                                            value: FnCall {
                                                fn_: BuiltIn::BuiltInFn(Name::from("--")).into(),
                                                fn_type: FnType(
                                                    vec![AtomicTypeEnum::INT.into()],
                                                    Box::new(AtomicTypeEnum::INT.into())
                                                ),
                                                args: vec![
                                                    Integer{value: 4}.into()
                                                ]
                                            }.into(),
                                        }.into(),
                                    ],
                                }
                            ]
                        }.into(),
                    ]
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
                    ret: (Memory(Id::from("result")).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
                    is_recursive: false,
                    size_bounds: (30, 50),
                    statements: vec![
                        Await(vec![Memory(Id::from("subject"))]).into(),
                        Declaration{
                            memory: Memory(Id::from("result")),
                            type_: AtomicTypeEnum::INT.into()
                        }.into(),
                        MatchStatement{
                            expression: (Memory(Id::from("subject")).into(), UnionType(vec![Name::from("empty")])),
                            auxiliary_memory: Memory(Id::from("aux")),
                            branches: vec![
                                MatchBranch {
                                    target: None,
                                    statements: vec![
                                        Await(vec![Memory(Id::from("f"))]).into(),
                                        Assignment{
                                            target: Memory(Id::from("y")),
                                            value: FnCall {
                                                fn_: Memory(Id::from("f")).into(),
                                                fn_type: FnType(
                                                    vec![AtomicTypeEnum::INT.into()],
                                                    Box::new(AtomicTypeEnum::INT.into())
                                                ),
                                                args: vec![
                                                    Integer{value: 5}.into()
                                                ]
                                            }.into(),
                                        }.into(),
                                        Assignment{
                                            target: Memory(Id::from("_")),
                                            value: FnCall {
                                                fn_: BuiltIn::BuiltInFn(Name::from("--")).into(),
                                                fn_type: FnType(
                                                    vec![AtomicTypeEnum::INT.into()],
                                                    Box::new(AtomicTypeEnum::INT.into())
                                                ),
                                                args: vec![
                                                    Integer{value: 4}.into()
                                                ]
                                            }.into(),
                                        }.into(),
                                        Assignment{
                                            target: Memory(Id::from("result")),
                                            value: FnCall {
                                                fn_: BuiltIn::BuiltInFn(Name::from("++")).into(),
                                                fn_type: FnType(
                                                    vec![AtomicTypeEnum::INT.into()],
                                                    Box::new(AtomicTypeEnum::INT.into())
                                                ),
                                                args: vec![
                                                    Memory(Id::from("y")).into()
                                                ]
                                            }.into(),
                                        }.into(),
                                    ],
                                }
                            ]
                        }.into(),
                    ]
                }
            ]
        };
        "program with match"
    )]
    fn test_reorder_program(program: Program, expected_program: Program) {
        let reordered_program = StatementReorderer::reorder(program);
        assert_eq!(expected_program, reordered_program)
    }
}
