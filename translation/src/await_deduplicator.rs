use std::collections::HashSet;

use itertools::Itertools;

use crate::{Assignment, Await, Expression, Id, Memory, Statement};

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
                Statement::Await(Await(ids)) => {
                    let fresh_ids = ids
                        .into_iter()
                        .filter(|id| !self.awaited_ids.contains(id))
                        .collect_vec();
                    if fresh_ids.is_empty() {
                        None
                    } else {
                        self.awaited_ids.extend(fresh_ids.clone());
                        Some(Await(fresh_ids).into())
                    }
                }
                Statement::IfStatement(if_statement) => todo!(),
                Statement::MatchStatement(match_statement) => todo!(),
                statement => Some(statement),
            })
            .collect_vec()
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
    fn test_deduplicate_statements(
        duplicated_statements: Vec<Statement>,
        expected_statements: Vec<Statement>,
    ) {
        let mut deduplicator = AwaitDeduplicator::new();
        let deduplicated_statements = deduplicator.deduplicate_statements(duplicated_statements);
        assert_eq!(expected_statements, deduplicated_statements)
    }
}
