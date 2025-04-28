use std::collections::HashSet;

use itertools::Itertools;

use crate::{
    Assignment, Await, Enqueue, FnDef, IfStatement, MatchBranch, MatchStatement, Memory, Program,
    Statement, Value,
};

struct Enqueuer {}

impl Enqueuer {
    fn new() -> Self {
        Self {}
    }
    /// Add extra requirements to statements and order correctly.
    fn fix_statements(
        &self,
        mut reversed_statements: Vec<Statement>,
        required: impl Iterator<Item = Memory>,
    ) -> Vec<Statement> {
        reversed_statements.extend(required.map(|memory| Enqueue(memory).into()));
        reversed_statements.reverse();
        reversed_statements
    }
    /// Add enqueue instructions to statements, returning the updated statements in _reverse_ order.
    fn enqueue_statements(
        &self,
        statements: Vec<Statement>,
        mut required: HashSet<Memory>,
    ) -> (Vec<Statement>, HashSet<Memory>) {
        let statements = statements
            .into_iter()
            .rev()
            .flat_map(|statement| match statement {
                Statement::Await(Await(ref memory)) => {
                    required.extend(memory.clone());
                    vec![statement]
                }
                Statement::Assignment(Assignment {
                    ref target,
                    value: _,
                }) => {
                    if required.remove(target) {
                        vec![Enqueue(target.clone()).into(), statement]
                    } else {
                        vec![statement]
                    }
                }
                Statement::IfStatement(IfStatement {
                    condition,
                    branches: (true_branch, false_branch),
                }) => {
                    let (true_branch, true_required) =
                        self.enqueue_statements(true_branch, required.clone());
                    let (false_branch, false_required) =
                        self.enqueue_statements(false_branch, required.clone());
                    let intersection = true_required
                        .intersection(&false_required)
                        .cloned()
                        .collect::<HashSet<_>>();
                    let true_required = true_required.difference(&intersection).cloned();
                    let false_required = false_required.difference(&intersection).cloned();
                    let true_branch = self.fix_statements(true_branch, true_required);
                    let false_branch = self.fix_statements(false_branch, false_required);
                    required = intersection;
                    vec![IfStatement {
                        condition: condition.clone(),
                        branches: (true_branch, false_branch),
                    }
                    .into()]
                }
                Statement::MatchStatement(MatchStatement {
                    expression,
                    branches,
                    auxiliary_memory,
                }) => {
                    let ((statements, all_required), targets): ((Vec<_>, Vec<_>), Vec<_>) =
                        branches
                            .into_iter()
                            .map(|MatchBranch { statements, target }| {
                                (
                                    self.enqueue_statements(statements, required.clone()),
                                    target,
                                )
                            })
                            .unzip();
                    let mut intersection = None;
                    for required in &all_required {
                        intersection = match intersection {
                            None => Some(required.clone()),
                            Some(intersection) => {
                                Some(intersection.intersection(required).cloned().collect())
                            }
                        }
                    }
                    let intersection = intersection.unwrap();
                    let branches = statements
                        .into_iter()
                        .zip_eq(all_required.into_iter())
                        .zip_eq(targets.into_iter())
                        .map(|((statements, required), target)| {
                            let difference = required.difference(&intersection).cloned();
                            let statements = self.fix_statements(statements, difference);
                            MatchBranch { target, statements }
                        })
                        .collect_vec();
                    required = intersection;
                    vec![MatchStatement {
                        expression,
                        branches,
                        auxiliary_memory,
                    }
                    .into()]
                }
                statement => vec![statement],
            })
            .collect_vec();
        (statements, required)
    }

    fn enqueue_fn_def(&self, fn_def: FnDef) -> FnDef {
        let FnDef {
            name,
            arguments,
            statements,
            ret,
            env,
            is_recursive,
            size_bounds,
        } = fn_def;
        let mut required = HashSet::new();
        if let Value::Memory(ref memory) = ret.0 {
            required.insert(memory.clone());
        }
        let (statements, required) = self.enqueue_statements(statements, required);
        let statements = self.fix_statements(statements, required.into_iter());
        FnDef {
            name,
            arguments,
            statements,
            ret,
            env,
            is_recursive,
            size_bounds,
        }
    }
    /// Update program with enqueue statements.
    fn enqueue(program: Program) -> Program {
        let Program { type_defs, fn_defs } = program;
        let enqueuer = Enqueuer::new();
        let fn_defs = fn_defs
            .into_iter()
            .map(|fn_def| enqueuer.enqueue_fn_def(fn_def))
            .collect_vec();
        Program { type_defs, fn_defs }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        BuiltIn, Declaration, FnCall, FnDef, FnType, Id, IfStatement, MatchBranch, MatchStatement,
        Name, TypeDef, UnionType, Value,
    };

    use super::*;

    use lowering::{AtomicTypeEnum, Integer};
    use test_case::test_case;

    #[test_case(
        vec![
            Await(vec![Memory(Id::from("f"))]).into(),
            Assignment{
                target: Memory(Id::from("y0")),
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
            Await(vec![Memory(Id::from("f"))]).into(),
            Assignment{
                target: Memory(Id::from("y1")),
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
            Await(vec![Memory(Id::from("y0")),Memory(Id::from("y1"))]).into(),
        ],
        vec![
            Await(vec![Memory(Id::from("f"))]).into(),
            Assignment{
                target: Memory(Id::from("y0")),
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
            Enqueue(Memory(Id::from("y0"))).into(),
            Await(vec![Memory(Id::from("f"))]).into(),
            Assignment{
                target: Memory(Id::from("y1")),
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
            Enqueue(Memory(Id::from("y1"))).into(),
            Await(vec![Memory(Id::from("y0")),Memory(Id::from("y1"))]).into(),
        ],
        vec!["f"];
        "sequential code"
    )]
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("c"))]).into(),
            IfStatement {
                condition: Memory(Id::from("c")).into(),
                branches: (
                    vec![
                        Await(vec![Memory(Id::from("f"))]).into(),
                        Assignment{
                            target: Memory(Id::from("y0")),
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
                        Await(vec![Memory(Id::from("y0"))]).into(),
                    ],
                    vec![
                        Await(vec![Memory(Id::from("f"))]).into(),
                        Assignment{
                            target: Memory(Id::from("y1")),
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
                        Await(vec![Memory(Id::from("y1"))]).into(),
                    ]
                )
            }.into()
        ],
        vec![
            Await(vec![Memory(Id::from("c"))]).into(),
            IfStatement {
                condition: Memory(Id::from("c")).into(),
                branches: (
                    vec![
                        Await(vec![Memory(Id::from("f"))]).into(),
                        Assignment{
                            target: Memory(Id::from("y0")),
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
                        Enqueue(Memory(Id::from("y0"))).into(),
                        Await(vec![Memory(Id::from("y0"))]).into(),
                    ],
                    vec![
                        Await(vec![Memory(Id::from("f"))]).into(),
                        Assignment{
                            target: Memory(Id::from("y1")),
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
                        Enqueue(Memory(Id::from("y1"))).into(),
                        Await(vec![Memory(Id::from("y1"))]).into(),
                    ]
                )
            }.into()
        ],
        vec!["f", "c"];
        "if branches with overlap"
    )]
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("c"))]).into(),
            IfStatement {
                condition: Memory(Id::from("c")).into(),
                branches: (
                    vec![
                        Await(vec![Memory(Id::from("f"))]).into(),
                    ],
                    vec![
                        Await(vec![Memory(Id::from("g"))]).into(),
                    ]
                )
            }.into()
        ],
        vec![
            Await(vec![Memory(Id::from("c"))]).into(),
            IfStatement {
                condition: Memory(Id::from("c")).into(),
                branches: (
                    vec![
                        Enqueue(Memory(Id::from("f"))).into(),
                        Await(vec![Memory(Id::from("f"))]).into(),
                    ],
                    vec![
                        Enqueue(Memory(Id::from("g"))).into(),
                        Await(vec![Memory(Id::from("g"))]).into(),
                    ]
                )
            }.into()
        ],
        vec!["c"];
        "if branches no overlap"
    )]
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("s"))]).into(),
            MatchStatement {
                expression: (Memory(Id::from("s")).into(), UnionType(vec![Name::from("wrapper")])),
                auxiliary_memory: Memory(Id::from("aux")).into(),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("x"))),
                        statements: vec![
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
                        ]
                    }
                ]
            }.into()
        ],
        vec![
            Await(vec![Memory(Id::from("s"))]).into(),
            MatchStatement {
                expression: (Memory(Id::from("s")).into(), UnionType(vec![Name::from("wrapper")])),
                auxiliary_memory: Memory(Id::from("aux")).into(),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("x"))),
                        statements: vec![
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
                        ]
                    }
                ]
            }.into()
        ],
        vec!["f","s"];
        "match single branch"
    )]
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("s"))]).into(),
            MatchStatement {
                expression: (Memory(Id::from("s")).into(), UnionType(vec![Name::from("wrapper"), Name::from("two"), Name::from("three")])),
                auxiliary_memory: Memory(Id::from("aux")).into(),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("x"))),
                        statements: vec![
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
                        ]
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("j"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Await(vec![Memory(Id::from("j"))]).into(),
                        ]
                    }
                ]
            }.into()
        ],
        vec![
            Await(vec![Memory(Id::from("s"))]).into(),
            MatchStatement {
                expression: (Memory(Id::from("s")).into(), UnionType(vec![Name::from("wrapper"), Name::from("two"), Name::from("three")])),
                auxiliary_memory: Memory(Id::from("aux")).into(),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("x"))),
                        statements: vec![
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
                        ]
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("j"))),
                        statements: vec![
                            Enqueue(Memory(Id::from("j"))).into(),
                            Await(vec![Memory(Id::from("f"))]).into(),
                            Await(vec![Memory(Id::from("j"))]).into(),
                        ]
                    }
                ]
            }.into()
        ],
        vec!["f","s"];
        "match multiple branches"
    )]
    fn test_enqueue_statements(
        statements: Vec<Statement>,
        expected_statements: Vec<Statement>,
        expected_required_values: Vec<&str>,
    ) {
        let enqueuer = Enqueuer::new();
        let (mut enqueued_statements, required_values) =
            enqueuer.enqueue_statements(statements, HashSet::new());
        enqueued_statements.reverse();
        assert_eq!(expected_statements, enqueued_statements);
        assert_eq!(
            expected_required_values
                .into_iter()
                .map(|id| Memory(Id::from(id)))
                .collect::<HashSet<_>>(),
            required_values
        );
    }

    #[test_case(
        Program{
            type_defs: vec![
                TypeDef {
                    name: Name::from("Empty"),
                    constructors: vec![(Name::from("empty"), None)]
                }
            ],
            fn_defs: vec![
                FnDef{
                    name: Name::from("F0"),
                    arguments: vec![
                        (
                            Memory(Id::from("f")),
                            FnType(
                                vec![AtomicTypeEnum::INT.into()],
                                Box::new(AtomicTypeEnum::INT.into()),
                            ).into(),
                        ),
                        (
                            Memory(Id::from("x")),
                            AtomicTypeEnum::INT.into(),
                        ),
                    ],
                    ret: (Memory(Id::from("y")).into(), AtomicTypeEnum::INT.into()),
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
                                    Memory(Id::from("x")).into(),
                                ]
                            }.into(),
                        }.into(),
                    ],
                    env: Vec::new(),
                    is_recursive: true,
                    size_bounds: (50, 50)
                },
                FnDef{
                    name: Name::from("F1"),
                    arguments: vec![
                        (
                            Memory(Id::from("s")),
                            UnionType(vec![Name::from("empty")]).into(),
                        ),
                    ],
                    ret: (Memory(Id::from("y")).into(), AtomicTypeEnum::INT.into()),
                    statements: vec![
                        Declaration{
                            memory: Memory(Id::from("x")),
                            type_: AtomicTypeEnum::INT.into()
                        }.into(),
                        Await(vec![Memory(Id::from("s"))]).into(),
                        MatchStatement {
                            expression: (Memory(Id::from("s")).into(), UnionType(vec![Name::from("empty")])),
                            auxiliary_memory: Memory(Id::from("aux")).into(),
                            branches: vec![
                                MatchBranch {
                                    target: None,
                                    statements: vec![
                                        Assignment {
                                            target: Memory(Id::from("x")),
                                            value: Value::from(Integer{value: 0}).into()
                                        }.into()
                                    ]
                                }
                            ]
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("y")),
                            value: Memory(Id::from("x")).into()
                        }.into()
                    ],
                    env: Vec::new(),
                    is_recursive: true,
                    size_bounds: (70, 70)
                }
            ]
        },
        Program{
            type_defs: vec![
                TypeDef {
                    name: Name::from("Empty"),
                    constructors: vec![(Name::from("empty"), None)]
                }
            ],
            fn_defs: vec![
                FnDef{
                    name: Name::from("F0"),
                    arguments: vec![
                        (
                            Memory(Id::from("f")),
                            FnType(
                                vec![AtomicTypeEnum::INT.into()],
                                Box::new(AtomicTypeEnum::INT.into()),
                            ).into(),
                        ),
                        (
                            Memory(Id::from("x")),
                            AtomicTypeEnum::INT.into(),
                        ),
                    ],
                    ret: (Memory(Id::from("y")).into(), AtomicTypeEnum::INT.into()),
                    statements: vec![
                        Enqueue(Memory(Id::from("f"))).into(),
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
                                    Memory(Id::from("x")).into(),
                                ]
                            }.into(),
                        }.into(),
                        Enqueue(Memory(Id::from("y"))).into(),
                    ],
                    env: Vec::new(),
                    is_recursive: true,
                    size_bounds: (50, 50)
                },
                FnDef{
                    name: Name::from("F1"),
                    arguments: vec![
                        (
                            Memory(Id::from("s")),
                            UnionType(vec![Name::from("empty")]).into(),
                        ),
                    ],
                    ret: (Memory(Id::from("y")).into(), AtomicTypeEnum::INT.into()),
                    statements: vec![
                        Enqueue(Memory(Id::from("s"))).into(),
                        Declaration{
                            memory: Memory(Id::from("x")),
                            type_: AtomicTypeEnum::INT.into()
                        }.into(),
                        Await(vec![Memory(Id::from("s"))]).into(),
                        MatchStatement {
                            expression: (Memory(Id::from("s")).into(), UnionType(vec![Name::from("empty")])),
                            auxiliary_memory: Memory(Id::from("aux")).into(),
                            branches: vec![
                                MatchBranch {
                                    target: None,
                                    statements: vec![
                                        Assignment {
                                            target: Memory(Id::from("x")),
                                            value: Value::from(Integer{value: 0}).into()
                                        }.into()
                                    ]
                                }
                            ]
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("y")),
                            value: Memory(Id::from("x")).into()
                        }.into(),
                        Enqueue(Memory(Id::from("y"))).into()
                    ],
                    env: Vec::new(),
                    is_recursive: true,
                    size_bounds: (70, 70)
                }
            ]
        };
        "simple program"
    )]
    fn test_enqueue_program(program: Program, expected_program: Program) {
        let program = Enqueuer::enqueue(program);
        assert_eq!(expected_program, program);
    }
}
