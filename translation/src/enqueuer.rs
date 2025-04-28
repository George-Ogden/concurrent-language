use std::collections::HashSet;

use itertools::Itertools;

use crate::{Assignment, Await, Enqueue, Memory, Statement};

struct Enqueuer {}

impl Enqueuer {
    fn new() -> Self {
        Self {}
    }
    fn enqueue(&self, statements: Vec<Statement>) -> (Vec<Statement>, HashSet<Memory>) {
        let mut required = HashSet::new();
        let mut statements = statements
            .into_iter()
            .rev()
            .flat_map(|statement| match &statement {
                Statement::Await(Await(memory)) => {
                    required.extend(memory.clone());
                    vec![statement]
                }
                Statement::Assignment(Assignment { target, value: _ }) => {
                    if required.remove(target) {
                        vec![Enqueue(target.clone()).into(), statement]
                    } else {
                        vec![statement]
                    }
                }
                Statement::IfStatement(if_) => todo!(),
                Statement::MatchStatement(match_) => todo!(),
                _ => vec![statement],
            })
            .collect_vec();
        statements.reverse();
        (statements, required)
    }
}

#[cfg(test)]
mod tests {
    use crate::{FnCall, FnType, Id};

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
    fn test_enqueue_statements(
        statements: Vec<Statement>,
        expected_statements: Vec<Statement>,
        expected_required_values: Vec<&str>,
    ) {
        let enqueuer = Enqueuer::new();
        let (enqueued_statements, required_values) = enqueuer.enqueue(statements);
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
