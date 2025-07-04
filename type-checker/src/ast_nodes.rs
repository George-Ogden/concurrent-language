use serde::{Deserialize, Serialize};
use std::{convert::From, fmt};
use strum_macros::EnumIter;

pub type Id = String;
use from_variants::FromVariants;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, EnumIter, Copy, Hash, Eq)]
pub enum AtomicTypeEnum {
    INT,
    BOOL,
}

impl fmt::Display for AtomicTypeEnum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AtomicType {
    pub type_: AtomicTypeEnum,
}

#[allow(dead_code)]
pub const ATOMIC_TYPE_INT: AtomicType = AtomicType {
    type_: AtomicTypeEnum::INT,
};
#[allow(dead_code)]
pub const ATOMIC_TYPE_BOOL: AtomicType = AtomicType {
    type_: AtomicTypeEnum::BOOL,
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GenericType {
    pub id: Id,
    pub type_variables: Vec<TypeInstance>,
}

#[allow(non_snake_case, dead_code)]
pub fn Typename(id: &str) -> GenericType {
    GenericType {
        id: Id::from(id),
        type_variables: Vec::new(),
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TupleType {
    pub types: Vec<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FunctionType {
    pub argument_types: Vec<TypeInstance>,
    pub return_type: Box<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, FromVariants, Clone)]
pub enum TypeInstance {
    FunctionType(FunctionType),
    AtomicType(AtomicType),
    TupleType(TupleType),
    GenericType(GenericType),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TypeItem {
    pub id: Id,
    pub type_: Option<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GenericTypeVariable {
    pub id: Id,
    pub generic_variables: Vec<Id>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UnionTypeDefinition {
    pub variable: GenericTypeVariable,
    pub items: Vec<TypeItem>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct OpaqueTypeDefinition {
    pub variable: GenericTypeVariable,
    pub type_: TypeInstance,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct EmptyTypeDefinition {
    pub id: Id,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TransparentTypeDefinition {
    pub variable: GenericTypeVariable,
    pub type_: TypeInstance,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, FromVariants, Clone)]
pub enum Definition {
    UnionTypeDefinition(UnionTypeDefinition),
    OpaqueTypeDefinition(OpaqueTypeDefinition),
    TransparentTypeDefinition(TransparentTypeDefinition),
    EmptyTypeDefinition(EmptyTypeDefinition),
    Assignment(Assignment),
}

impl Definition {
    pub fn get_id(&self) -> &Id {
        match self {
            Self::UnionTypeDefinition(UnionTypeDefinition {
                variable:
                    GenericTypeVariable {
                        id,
                        generic_variables: _,
                    },
                items: _,
            })
            | Self::EmptyTypeDefinition(EmptyTypeDefinition { id })
            | Self::TransparentTypeDefinition(TransparentTypeDefinition {
                variable:
                    GenericTypeVariable {
                        id,
                        generic_variables: _,
                    },
                type_: _,
            })
            | Self::OpaqueTypeDefinition(OpaqueTypeDefinition {
                variable:
                    GenericTypeVariable {
                        id,
                        generic_variables: _,
                    },
                type_: _,
            })
            | Self::Assignment(Assignment {
                assignee:
                    ParametricAssignee {
                        assignee: Assignee { id },
                        generic_variables: _,
                    },
                expression: _,
            }) => id,
        }
    }
    pub fn get_parameters(&self) -> Vec<String> {
        match self {
            Self::UnionTypeDefinition(UnionTypeDefinition {
                variable:
                    GenericTypeVariable {
                        id: _,
                        generic_variables,
                    },
                items: _,
            })
            | Self::TransparentTypeDefinition(TransparentTypeDefinition {
                variable:
                    GenericTypeVariable {
                        id: _,
                        generic_variables,
                    },
                type_: _,
            })
            | Self::OpaqueTypeDefinition(OpaqueTypeDefinition {
                variable:
                    GenericTypeVariable {
                        id: _,
                        generic_variables,
                    },
                type_: _,
            })
            | Self::Assignment(Assignment {
                assignee:
                    ParametricAssignee {
                        assignee: _,
                        generic_variables,
                    },
                expression: _,
            }) => generic_variables.clone(),
            Self::EmptyTypeDefinition(EmptyTypeDefinition { id: _ }) => Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Hash, Eq)]
pub struct Integer {
    pub value: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Hash, Eq)]
pub struct Boolean {
    pub value: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TupleExpression {
    pub expressions: Vec<Expression>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GenericVariable {
    pub id: Id,
    pub type_instances: Vec<TypeInstance>,
}

#[allow(non_snake_case, dead_code)]
pub fn Var(id: &str) -> GenericVariable {
    GenericVariable {
        id: Id::from(id),
        type_instances: Vec::new(),
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ElementAccess {
    pub expression: Box<Expression>,
    pub index: usize,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct IfExpression {
    pub condition: Box<Expression>,
    pub true_block: Block,
    pub false_block: Block,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MatchItem {
    pub type_name: Id,
    pub assignee: Option<Assignee>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MatchBlock {
    pub matches: Vec<MatchItem>,
    pub block: Block,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MatchExpression {
    pub subject: Box<Expression>,
    pub blocks: Vec<MatchBlock>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TypedAssignee {
    pub assignee: Assignee,
    pub type_: TypeInstance,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FunctionDefinition {
    pub parameters: Vec<TypedAssignee>,
    pub return_type: TypeInstance,
    pub body: Block,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FunctionCall {
    pub function: Box<Expression>,
    pub arguments: Vec<Expression>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ConstructorCall {
    pub constructor: GenericConstructor,
    pub arguments: Vec<Expression>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GenericConstructor {
    pub id: Id,
    pub type_instances: Vec<TypeInstance>,
}

#[allow(non_snake_case, dead_code)]
pub fn Constructor(id: &str) -> GenericConstructor {
    GenericConstructor {
        id: Id::from(id),
        type_instances: Vec::new(),
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, FromVariants, Clone)]
pub enum Expression {
    Integer(Integer),
    Boolean(Boolean),
    TupleExpression(TupleExpression),
    GenericVariable(GenericVariable),
    ElementAccess(ElementAccess),
    IfExpression(IfExpression),
    MatchExpression(MatchExpression),
    FunctionDefinition(FunctionDefinition),
    FunctionCall(FunctionCall),
    ConstructorCall(ConstructorCall),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Assignee {
    pub id: Id,
}

impl From<Id> for Assignee {
    fn from(value: Id) -> Self {
        Assignee { id: value }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ParametricAssignee {
    pub assignee: Assignee,
    pub generic_variables: Vec<Id>,
}

impl ParametricAssignee {
    pub fn id(&self) -> Id {
        self.assignee.id.clone()
    }
}

#[allow(non_snake_case, dead_code)]
pub fn VariableAssignee(id: &str) -> ParametricAssignee {
    ParametricAssignee {
        assignee: Assignee::from(Id::from(id)),
        generic_variables: Vec::new(),
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Assignment {
    pub assignee: ParametricAssignee,
    pub expression: Box<Expression>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Block {
    pub assignments: Vec<Assignment>,
    pub expression: Box<Expression>,
}

#[allow(non_snake_case, dead_code)]
pub fn ExpressionBlock(expression: Expression) -> Block {
    return Block {
        assignments: Vec::new(),
        expression: Box::new(expression),
    };
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Program {
    pub definitions: Vec<Definition>,
}

#[cfg(test)]
mod tests {

    use super::*;

    use test_case::test_case;

    #[test_case(
        r#""INT""#,
        AtomicTypeEnum::INT;
        "atomic type enum int"
    )]
    #[test_case(
        r#""BOOL""#,
        AtomicTypeEnum::BOOL;
        "atomic type enum bool"
    )]
    #[test_case(
        r#"{"type_": "BOOL"}"#,
        AtomicType{type_: AtomicTypeEnum::BOOL};
        "atomic type bool"
    )]
    #[test_case(
        r#"{"types": []}"#,
        TupleType{types: Vec::new()};
        "empty tuple type"
    )]
    #[test_case(
        r#"{"types":[{"AtomicType":{"type_":"BOOL"}},{"TupleType":{"types":[]}}]}"#,
        TupleType{
            types: vec![
                ATOMIC_TYPE_BOOL.into(),
                TupleType{types: Vec::new()}.into(),
            ]
        };
        "non-empty tuple type"
    )]
    #[test_case(
        r#"{"argument_types":[{"AtomicType":{"type_":"INT"}}],"return_type":{"AtomicType":{"type_":"INT"}}}"#,
        FunctionType{
            argument_types: vec![ATOMIC_TYPE_INT.into()],
            return_type: Box::new(
                ATOMIC_TYPE_INT.into()
            )
        };
        "function type"
    )]
    #[test_case(
        r#"{"argument_types":[{"TupleType":{"types":[{"AtomicType":{"type_":"INT"}}]}}],"return_type":{"TupleType":{"types":[]}}}"#,
        FunctionType{
            argument_types: vec![TupleType{types: vec![ATOMIC_TYPE_INT.into()]}.into()],
            return_type: Box::new(
                TupleType{types: Vec::new()}.into()
            )
        };
        "function type single tuple argument"
    )]
    #[test_case(
        r#"{"argument_types":[{"FunctionType":{"argument_types":[{"AtomicType":{"type_":"INT"}}],"return_type":{"AtomicType":{"type_":"INT"}}}}],"return_type":{"AtomicType":{"type_":"INT"}}}"#,
        FunctionType{
            argument_types: vec![
                FunctionType{
                    argument_types: vec![ATOMIC_TYPE_INT.into()],
                    return_type: Box::new(
                        ATOMIC_TYPE_INT.into()
                    )
                }.into()
            ],
            return_type: Box::new(
                ATOMIC_TYPE_INT.into()
            )
        };
        "higher-order function type"
    )]
    #[test_case(
        r#"{"id":"map","type_variables":[{"AtomicType":{"type_":"INT"}},{"AtomicType":{"type_":"BOOL"}}]}"#,
        GenericType{
            id: Id::from("map"),
            type_variables: vec![
                ATOMIC_TYPE_INT.into(),
                ATOMIC_TYPE_BOOL.into()
            ]
        };
        "generic type"
    )]
    #[test_case(
        r#"{"id":"map","type_variables":[{"FunctionType":{"argument_types":[{"AtomicType":{"type_":"INT"}}],"return_type":{"AtomicType":{"type_":"INT"}}}},{"GenericType":{"id":"foo","type_variables":[]}}]}"#,
        GenericType{
            id: Id::from("map"),
            type_variables: vec![
                FunctionType{
                    argument_types: vec![
                        ATOMIC_TYPE_INT.into()
                    ],
                    return_type: Box::new(
                        ATOMIC_TYPE_INT.into()
                    )
                }.into(),
                GenericType {
                    id: Id::from("foo"),
                    type_variables: Vec::new()
                }.into()
            ]
        };
        "nested generic type"
    )]
    #[test_case(
        r#"{"variable":{"id":"Maybe","generic_variables":["T"]},"items":[{"id":"Some","type_":{"GenericType":{"id":"T","type_variables":[]}}},{"id":"None","type_":null}]}"#,
        UnionTypeDefinition {
            variable: GenericTypeVariable{
                id: Id::from("Maybe"),
                generic_variables: vec![Id::from("T")]
            },
            items: vec![
                TypeItem {
                    id: Id::from("Some"),
                    type_: Some(Typename("T").into()),
                },
                TypeItem {
                    id: Id::from("None"),
                    type_: None
                }
            ]
        };
        "union type definition"
    )]
    #[test_case(
        r#"{"variable":{"id":"Pair","generic_variables":["T","U"]},"type_":{"TupleType":{"types":[{"GenericType":{"id":"T","type_variables":[]}},{"GenericType":{"id":"U","type_variables":[]}}]}}}"#,
        OpaqueTypeDefinition{
            variable: GenericTypeVariable{
                id: Id::from("Pair"),
                generic_variables: vec![Id::from("T"), Id::from("U")]
            },
            type_: TupleType{
                types: vec![Typename("T").into(), Typename("U").into()]
            }.into()
        };
        "opaque type definition"
    )]
    #[test_case(
        r#"{"variable":{"id":"F","generic_variables":[]},"type_":{"FunctionType":{"argument_types":[{"FunctionType":{"argument_types":[{"AtomicType":{"type_":"INT"}}],"return_type":{"AtomicType":{"type_":"INT"}}}}],"return_type":{"AtomicType":{"type_":"INT"}}}}}"#,
        OpaqueTypeDefinition{
            variable: GenericTypeVariable{
                id: Id::from("F"),
                generic_variables: Vec::new()
            },
            type_: FunctionType{
                argument_types: vec![
                    FunctionType{
                        argument_types: vec![ATOMIC_TYPE_INT.into()],
                        return_type: Box::new(
                            ATOMIC_TYPE_INT.into()
                        )
                    }.into()
                ],
                return_type: Box::new(
                    ATOMIC_TYPE_INT.into()
                )
            }.into()
        };
        "opaque function type definition"
    )]
    #[test_case(
        r#"{"id":"None"}"#,
        EmptyTypeDefinition{
            id: Id::from("None")
        };
        "empty type definition"
    )]
    #[test_case(
        r#"{"variable":{"id":"ii","generic_variables":[]},"type_":{"TupleType":{"types":[{"AtomicType":{"type_":"INT"}},{"AtomicType":{"type_":"INT"}}]}}}"#,
        TransparentTypeDefinition{
            variable: GenericTypeVariable{
                id: Id::from("ii"),
                generic_variables: Vec::new()
            },
            type_: TupleType{
                types: vec![
                    ATOMIC_TYPE_INT.into(),
                    ATOMIC_TYPE_INT.into(),
                ]
            }.into()
        };
        "transparent type definition"
    )]
    #[test_case(
        r#"{"value":128}"#,
        Integer{
            value: 128
        };
        "positive integer"
    )]
    #[test_case(
        r#"{"value":-128}"#,
        Integer{
            value: -128
        };
        "negative integer"
    )]
    #[test_case(
        r#"{"value":true}"#,
        Boolean{
            value: true
        };
        "boolean"
    )]
    #[test_case(
        r#"{"expressions":[]}"#,
        TupleExpression{
            expressions: Vec::new()
        };
        "empty tuple"
    )]
    #[test_case(
        r#"{"expressions":[{"Boolean":{"value":false}},{"Integer":{"value":5}}]}"#,
        TupleExpression{
            expressions: vec![
                Boolean { value: false }.into(),
                Integer { value: 5 }.into(),
            ]
        };
        "flat tuple"
    )]
    #[test_case(
        r#"{"expressions":[{"TupleExpression":{"expressions":[{"Boolean":{"value":false}},{"Integer":{"value":5}}]}},{"TupleExpression":{"expressions":[]}}]}"#,
        TupleExpression{
            expressions: vec![
                TupleExpression{
                    expressions: vec![
                        Boolean { value: false }.into(),
                        Integer { value: 5 }.into(),
                    ]
                }.into(),
                TupleExpression{
                    expressions: Vec::new()
                }.into()
            ]
        };
        "nested tuple"
    )]
    #[test_case(
        r#"{"id":"foo","type_instances":[]}"#,
        Var("foo");
        "variable"
    )]
    #[test_case(
        r#"{"id":"map","type_instances":[{"AtomicType":{"type_":"INT"}}]}"#,
        GenericVariable{
            id: Id::from("map"),
            type_instances: vec![ATOMIC_TYPE_INT.into()]
        };
        "generic concrete instance"
    )]
    #[test_case(
        r#"{"id":"foo","type_instances":[{"GenericType":{"id":"T","type_variables":[]}}]}"#,
        GenericVariable{
            id: Id::from("foo"),
            type_instances: vec![Typename("T").into()]
        };
        "generic variable instance"
    )]
    #[test_case(
        r#"{"expression":{"TupleExpression":{"expressions":[{"Integer":{"value":0}}]}},"index":0}"#,
        ElementAccess{
            expression: Box::new(TupleExpression{
                expressions: vec![
                    Integer{value: 0}.into()
                ]
            }.into()),
            index: 0
        };
        "single element access"
    )]
    #[test_case(
        r#"{"expression":{"ElementAccess":{"expression":{"GenericVariable":{"id":"foo","type_instances":[]}},"index":13}},"index":1}"#,
        ElementAccess{
            expression: Box::new(
                ElementAccess{
                    expression: Box::new(
                        Var("foo").into()
                    ),
                    index: 13,
                }.into()
            ),
            index: 1
        };
        "nested element access"
    )]
    #[test_case(
        r#"{"assignee":{"id":"a"},"generic_variables":[]}"#,
        ParametricAssignee {
            assignee: Id::from("a").into(),
            generic_variables: Vec::new()
        };
        "basic assignee"
    )]
    #[test_case(
        r#"{"assignee":{"id":"f"},"generic_variables":["T","U"]}"#,
        ParametricAssignee {
            assignee: Id::from("f").into(),
            generic_variables: vec![
                Id::from("T"),
                Id::from("U")
            ]
        };
        "generic assignee"
    )]
    #[test_case(
        r#"{"assignee":{"assignee":{"id":"a"},"generic_variables":[]},"expression":{"GenericVariable":{"id":"b","type_instances":[]}}}"#,
        Assignment {
            assignee: VariableAssignee("a"),
            expression: Box::new(Var("b").into())
        };
        "variable assignment"
    )]
    #[test_case(
        r#"{"assignee":{"assignee":{"id":"a"},"generic_variables":["T"]},"expression":{"GenericVariable":{"id":"b","type_instances":[{"GenericType":{"id":"T","type_variables":[]}}]}}}"#,
        Assignment {
            assignee: ParametricAssignee {
                assignee: Id::from("a").into(),
                generic_variables: vec![
                    Id::from("T")
                ]
            },
            expression: Box::new(GenericVariable{
                id: Id::from("b"),
                type_instances: vec![
                    Typename("T").into()
                ]
            }.into())
        };
        "generic variable assignment"
    )]
    #[test_case(
        r#"{"assignments":[],"expression":{"Integer":{"value":3}}}"#,
        ExpressionBlock(Integer{value:3}.into())
        ;
        "assignment-free block"
    )]
    #[test_case(
        r#"{"assignments":[{"assignee":{"assignee":{"id":"a"},"generic_variables":[]},"expression":{"GenericVariable":{"id":"x","type_instances":[]}}},{"assignee":{"assignee":{"id":"b"},"generic_variables":[]},"expression":{"Integer":{"value":3}}}],"expression":{"Integer":{"value":4}}}"#,
        Block {
            assignments: vec![
                Assignment {
                    assignee: VariableAssignee("a"),
                    expression: Box::new(Var("x").into())
                },
                Assignment {
                    assignee: VariableAssignee("b"),
                    expression: Box::new(Integer{value:3}.into())
                },
            ],
            expression: Box::new(Integer{
                value: 4
            }.into())
        };
        "block"
    )]
    #[test_case(
        r#"{"condition":{"Boolean":{"value":true}},"true_block":{"assignments":[],"expression":{"Integer":{"value":1}}},"false_block":{"assignments":[],"expression":{"Integer":{"value":-1}}}}"#,
        IfExpression {
            condition: Box::new(
                Boolean{ value: true }.into()
            ),
            true_block: ExpressionBlock(Integer{ value: 1 }.into()),
            false_block: ExpressionBlock(Integer{ value: -1 }.into()),
        };
        "flat if expression"
    )]
    #[test_case(
        r#"{"condition":{"IfExpression":{"condition":{"Boolean":{"value":true}},"true_block":{"assignments":[],"expression":{"Boolean":{"value":true}}},"false_block":{"assignments":[],"expression":{"Boolean":{"value":false}}}}},"true_block":{"assignments":[],"expression":{"IfExpression":{"condition":{"Boolean":{"value":false}},"true_block":{"assignments":[],"expression":{"Integer":{"value":1}}},"false_block":{"assignments":[],"expression":{"Integer":{"value":0}}}}}},"false_block":{"assignments":[],"expression":{"Integer":{"value":-1}}}}"#,
        IfExpression {
            condition: Box::new(
                IfExpression {
                    condition: Box::new(
                        Boolean{ value: true }.into()
                    ),
                    true_block: ExpressionBlock(
                        Boolean{ value: true }.into()
                    )
                    ,
                    false_block: ExpressionBlock(
                        Boolean{ value: false }.into()
                    )

                }.into()
            ),
            true_block: ExpressionBlock(IfExpression {
                        condition: Box::new(
                            Boolean{ value: false }.into()
                        ),
                        true_block: ExpressionBlock(
                            Integer{ value: 1 }.into()
                        ),
                        false_block: ExpressionBlock(
                            Integer{ value: 0 }.into()
                        )
                    }.into()),
            false_block: ExpressionBlock(Integer{ value: -1 }.into()),
        };
        "nested if expression"
    )]
    #[test_case(
        r#"{"type_name":"Some","assignee":{"id":"x"}}"#,
        MatchItem {
            type_name: Id::from("Some"),
            assignee: Some(Id::from("x").into()),
        };
        "present match item"
    )]
    #[test_case(
        r#"{"type_name":"None","assignee":null}"#,
        MatchItem {
            type_name: Id::from("None"),
            assignee: None,
        };
        "absent match item"
    )]
    #[test_case(
        r#"{"matches":[{"type_name":"None","assignee":null},{"type_name":"Some","assignee":{"id":"x"}}],"block":{"assignments":[],"expression":{"Boolean":{"value":true}}}}"#,
        MatchBlock {
            matches: vec![
                MatchItem {
                    type_name: Id::from("None"),
                    assignee: None,
                },
                MatchItem {
                    type_name: Id::from("Some"),
                    assignee: Some(Id::from("x").into()),
                }
            ],
            block: Block{
                assignments: Vec::new(),
                expression: Box::new(
                    Boolean{value: true}.into()
                )
            }
        };
        "match block"
    )]
    #[test_case(
        r#"{"subject":{"GenericVariable":{"id":"maybe","type_instances":[]}},"blocks":[{"matches":[{"type_name":"Some","assignee":{"id":"x"}}],"block":{"assignments":[],"expression":{"Boolean":{"value":true}}}},{"matches":[{"type_name":"None","assignee":null}],"block":{"assignments":[],"expression":{"Boolean":{"value":false}}}}]}"#,
        MatchExpression {
            subject: Box::new(Var("maybe").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("Some"),
                            assignee: Some(Id::from("x").into()),
                        }
                    ],
                    block: Block{
                        assignments: Vec::new(),
                        expression: Box::new(
                            Boolean{value: true}.into()
                        )
                    }
                },
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("None"),
                            assignee: None,
                        }
                    ],
                    block: Block{
                        assignments: Vec::new(),
                        expression: Box::new(
                            Boolean{value: false}.into()
                        )
                    }
                }
            ]
        };
        "flat match expression"
    )]
    #[test_case(
        r#"{"subject":{"GenericVariable":{"id":"maybe","type_instances":[]}},"blocks":[{"matches":[{"type_name":"Some","assignee":{"id":"x"}}],"block":{"assignments":[],"expression":{"MatchExpression":{"subject":{"GenericVariable":{"id":"x","type_instances":[]}},"blocks":[{"matches":[{"type_name":"Positive","assignee":null}],"block":{"assignments":[],"expression":{"Integer":{"value":1}}}},{"matches":[{"type_name":"Negative","assignee":null}],"block":{"assignments":[],"expression":{"Integer":{"value":-1}}}}]}}}},{"matches":[{"type_name":"None","assignee":null}],"block":{"assignments":[],"expression":{"Integer":{"value":0}}}}]}"#,
        MatchExpression {
            subject: Box::new(Var("maybe").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("Some"),
                            assignee: Some(Id::from("x").into()),
                        }
                    ],
                    block: Block{
                        assignments: Vec::new(),
                        expression: Box::new(
                            MatchExpression {
                                subject: Box::new(Var("x").into()),
                                blocks: vec![
                                    MatchBlock{
                                        matches: vec![
                                            MatchItem{
                                                type_name: Id::from("Positive"),
                                                assignee: None
                                            }
                                        ],
                                        block: Block{
                                            assignments: Vec::new(),
                                            expression: Box::new(Integer{value: 1}.into())
                                        }
                                    },
                                    MatchBlock{
                                        matches: vec![
                                            MatchItem{
                                                type_name: Id::from("Negative"),
                                                assignee: None
                                            }
                                        ],
                                        block: Block{
                                            assignments: Vec::new(),
                                            expression: Box::new(Integer{value: -1}.into())
                                        }
                                    }
                                ]
                            }.into()
                        )
                    }
                },
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("None"),
                            assignee: None,
                        }
                    ],
                    block: Block{
                        assignments: Vec::new(),
                        expression: Box::new(
                            Integer{value: 0}.into()
                        )
                    }
                }
            ]
        };
        "nested match expression"
    )]
    #[test_case(
        r#"{"function":{"GenericVariable":{"id":"foo","type_instances":[]}},"arguments":[{"Integer":{"value":3}},{"GenericVariable":{"id":"x","type_instances":[]}}]}"#,
        FunctionCall{
            function: Box::new(Var("foo").into()),
            arguments: vec![Integer{value: 3}.into(), Var("x").into()]
        };
        "function call expression"
    )]
    #[test_case(
        r#"{"constructor":{"id":"foo","type_instances":[{"AtomicType":{"type_":"INT"}}]},"arguments":[{"Integer":{"value":3}},{"GenericVariable":{"id":"x","type_instances":[]}}]}"#,
        ConstructorCall{
            constructor: GenericConstructor{
                id: Id::from("foo"),
                type_instances: vec![ATOMIC_TYPE_INT.into()]

            },
            arguments: vec![Integer{value: 3}.into(), Var("x").into()]
        };
        "constructor call expression"
    )]
    #[test_case(
        r#"{"parameters":[{"assignee":{"id":"x"},"type_":{"AtomicType":{"type_":"INT"}}},{"assignee":{"id":"y"},"type_":{"AtomicType":{"type_":"BOOL"}}}],"return_type":{"AtomicType":{"type_":"BOOL"}},"body":{"assignments":[],"expression":{"GenericVariable":{"id":"y","type_instances":[]}}}}"#,
        FunctionDefinition{
            parameters: vec![
                TypedAssignee {
                    assignee: Assignee { id: Id::from("x") },
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee {
                    assignee: Assignee { id: Id::from("y") },
                    type_: ATOMIC_TYPE_BOOL.into()
                }
            ],
            return_type: ATOMIC_TYPE_BOOL.into(),
            body: ExpressionBlock(Var("y").into())
        };
        "function definition expression"
    )]
    #[test_case(
        r#"{"definitions":[]}"#,
        Program{
            definitions: Vec::new(),
        };
        "empty program"
    )]
    #[test_case(
        r#"{"definitions":[{"OpaqueTypeDefinition":{"variable":{"id":"F","generic_variables":[]},"type_":{"FunctionType":{"argument_types":[{"FunctionType":{"argument_types":[{"AtomicType":{"type_":"INT"}}],"return_type":{"AtomicType":{"type_":"INT"}}}}],"return_type":{"AtomicType":{"type_":"INT"}}}}}}]}"#,
        Program{
            definitions: vec![
                OpaqueTypeDefinition{
                    variable: GenericTypeVariable{
                        id: Id::from("F"),
                        generic_variables: Vec::new()
                    },
                    type_: FunctionType{
                        argument_types: vec![
                            FunctionType{
                                argument_types: vec![ATOMIC_TYPE_INT.into()],
                                return_type: Box::new(
                                    ATOMIC_TYPE_INT.into()
                                )
                            }.into()
                        ],
                        return_type: Box::new(
                            ATOMIC_TYPE_INT.into()
                        )
                    }.into()
                }.into()
            ]
        };
        "short program"
    )]
    #[test_case(
        r#"{"definitions":[{"UnionTypeDefinition":{"variable":{"id":"Maybe","generic_variables":["T"]},"items":[{"id":"Some","type_":{"GenericType":{"id":"T","type_variables":[]}}},{"id":"None","type_":null}]}},{"TransparentTypeDefinition":{"variable":{"id":"Pair","generic_variables":["T","U"]},"type_":{"TupleType":{"types":[{"GenericType":{"id":"T","type_variables":[]}},{"GenericType":{"id":"U","type_variables":[]}}]}}}},{"Assignment":{"assignee":{"assignee":{"id":"a"},"generic_variables":[]},"expression":{"GenericVariable":{"id":"x","type_instances":[]}}}},{"Assignment":{"assignee":{"assignee":{"id":"b"},"generic_variables":[]},"expression":{"Integer":{"value":3}}}}]}"#,
        Program{
            definitions: vec![
                UnionTypeDefinition {
                    variable: GenericTypeVariable{
                        id: Id::from("Maybe"),
                        generic_variables: vec![Id::from("T")]
                    },
                    items: vec![
                        TypeItem {
                            id: Id::from("Some"),
                            type_: Some(Typename("T").into()),
                        },
                        TypeItem {
                            id: Id::from("None"),
                            type_: None
                        }
                    ]
                }.into(),
                TransparentTypeDefinition{
                    variable: GenericTypeVariable{
                        id: Id::from("Pair"),
                        generic_variables: vec![Id::from("T"), Id::from("U")]
                    },
                    type_: TupleType{
                        types: vec![Typename("T").into(), Typename("U").into()]
                    }.into()
                }.into(),
                Assignment {
                    assignee: VariableAssignee("a"),
                    expression: Box::new(Var("x").into())
                }.into(),
                Assignment {
                    assignee: VariableAssignee("b"),
                    expression: Box::new(Integer{value:3}.into())
                }.into(),
            ]
        };
        "non-empty program"
    )]
    fn test_deserialize_json<
        T: std::fmt::Debug + std::cmp::PartialEq + for<'a> serde::Deserialize<'a> + serde::Serialize,
    >(
        json: &str,
        node: T,
    ) {
        let result = serde_json::from_str::<T>(&json);
        if !result.is_ok() {
            println!("{:?}", serde_json::to_string(&node))
        }
        assert!(result.is_ok());
        let _ = result.inspect(|ast| assert_eq!(ast, &node));
    }

    #[test]
    fn test() {
        let string = "{\"definitions\": []}\n";
        // Test deserializing JSON (examples copied from Python code).
        let result = serde_json::from_str::<Program>(&string);
        dbg!(&result);
        assert!(result.is_ok())
    }
}
