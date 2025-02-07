use crate::{
    ast_nodes::{
        Assignee, Assignment, Definition, ExpressionBlock, FunctionDefinition, IfExpression,
        TypedAssignee, Var, VariableAssignee, ATOMIC_TYPE_BOOL,
    },
    Boolean, Id,
};

pub fn prefix() -> Vec<Definition> {
    vec![
        Assignment {
            assignee: VariableAssignee("&&"),
            expression: Box::new(
                FunctionDefinition {
                    parameters: vec![
                        TypedAssignee {
                            assignee: Assignee { id: Id::from("a") },
                            type_: ATOMIC_TYPE_BOOL.into(),
                        },
                        TypedAssignee {
                            assignee: Assignee { id: Id::from("b") },
                            type_: ATOMIC_TYPE_BOOL.into(),
                        },
                    ],
                    return_type: ATOMIC_TYPE_BOOL.into(),
                    body: ExpressionBlock(
                        IfExpression {
                            condition: Box::new(Var("a").into()),
                            true_block: ExpressionBlock(Var("b").into()),
                            false_block: ExpressionBlock(Boolean { value: false }.into()),
                        }
                        .into(),
                    ),
                }
                .into(),
            ),
        }
        .into(),
        Assignment {
            assignee: VariableAssignee("||"),
            expression: Box::new(
                FunctionDefinition {
                    parameters: vec![
                        TypedAssignee {
                            assignee: Assignee { id: Id::from("a") },
                            type_: ATOMIC_TYPE_BOOL.into(),
                        },
                        TypedAssignee {
                            assignee: Assignee { id: Id::from("b") },
                            type_: ATOMIC_TYPE_BOOL.into(),
                        },
                    ],
                    return_type: ATOMIC_TYPE_BOOL.into(),
                    body: ExpressionBlock(
                        IfExpression {
                            condition: Box::new(Var("a").into()),
                            true_block: ExpressionBlock(Boolean { value: false }.into()),
                            false_block: ExpressionBlock(Var("b").into()),
                        }
                        .into(),
                    ),
                }
                .into(),
            ),
        }
        .into(),
    ]
}
