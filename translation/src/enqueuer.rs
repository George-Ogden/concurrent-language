use std::collections::HashSet;

use itertools::Itertools;

use crate::{Assignment, Await, Enqueue, IfStatement, Memory, Statement};

struct Enqueuer {}

impl Enqueuer {
    fn new() -> Self {
        Self {}
    }
    /// Add enqueue instructions to statements, returning the updated statements in _reverse_ order.
    fn enqueue_statements(&self, statements: Vec<Statement>) -> (Vec<Statement>, HashSet<Memory>) {
        let mut required = HashSet::new();
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
                    let (mut true_branch, true_required) = self.enqueue_statements(true_branch);
                    let (mut false_branch, false_required) = self.enqueue_statements(false_branch);
                    let intersection = true_required
                        .intersection(&false_required)
                        .cloned()
                        .collect::<HashSet<_>>();
                    let true_required = true_required.difference(&intersection).cloned();
                    let false_required = false_required.difference(&intersection).cloned();
                    true_branch.extend(true_required.map(|memory| Enqueue(memory).into()));
                    true_branch.reverse();
                    false_branch.extend(false_required.map(|memory| Enqueue(memory).into()));
                    false_branch.reverse();
                    required.extend(intersection);
                    vec![IfStatement {
                        condition: condition.clone(),
                        branches: (true_branch, false_branch),
                    }
                    .into()]
                }
                Statement::MatchStatement(match_) => todo!(),
                statement => vec![statement],
            })
            .collect_vec();
        (statements, required)
    }
}

#[cfg(test)]
mod tests {
    use crate::{FnCall, FnType, Id, IfStatement, MatchBranch, MatchStatement, Name, UnionType};

    use super::*;

    use lowering::AtomicTypeEnum;
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
    fn test_enqueue_statements(
        statements: Vec<Statement>,
        expected_statements: Vec<Statement>,
        expected_required_values: Vec<&str>,
    ) {
        let enqueuer = Enqueuer::new();
        let (mut enqueued_statements, required_values) = enqueuer.enqueue_statements(statements);
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
}
