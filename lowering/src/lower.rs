use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::intermediate_nodes::*;
use type_checker::*;

type Scope = HashMap<Variable, IntermediateValue>;
type History = HashMap<IntermediateExpression, IntermediateValue>;
type Uninstantiated = HashMap<(Variable, Rc<RefCell<Option<Type>>>), IntermediateExpression>;
type TypeDefs = HashMap<Type, Rc<RefCell<IntermediateType>>>;
type VisitedReferences = HashSet<*mut IntermediateType>;

struct Lowerer {
    scope: Scope,
    history: History,
    uninstantiated: Uninstantiated,
    type_defs: TypeDefs,
    statements: Vec<IntermediateStatement>,
    visited_references: VisitedReferences,
}

impl Lowerer {
    pub fn new() -> Self {
        Lowerer {
            scope: Scope::new(),
            history: History::new(),
            uninstantiated: Uninstantiated::new(),
            type_defs: TypeDefs::new(),
            statements: Vec::new(),
            visited_references: VisitedReferences::new(),
        }
    }
    fn lower_expression(&mut self, expression: TypedExpression) -> IntermediateValue {
        match expression {
            TypedExpression::Integer(integer) => IntermediateBuiltIn::Integer(integer).into(),
            TypedExpression::Boolean(boolean) => IntermediateBuiltIn::Boolean(boolean).into(),
            TypedExpression::TypedTuple(TypedTuple { expressions }) => {
                let intermediate_expressions = self.lower_expressions(expressions);
                let intermediate_expression: IntermediateExpression =
                    IntermediateTupleExpression(intermediate_expressions).into();
                self.history
                    .entry(intermediate_expression.clone())
                    .or_insert_with(|| {
                        let value: IntermediateMemory = intermediate_expression.into();
                        self.statements
                            .push(IntermediateStatement::Assignment(value.clone()));
                        value.into()
                    })
                    .clone()
            }
            _ => todo!(),
        }
    }
    fn lower_expressions(&mut self, expressions: Vec<TypedExpression>) -> Vec<IntermediateValue> {
        expressions
            .into_iter()
            .map(|expression| self.lower_expression(expression))
            .collect()
    }
    pub fn lower_type(&mut self, type_: &Type) -> IntermediateType {
        self.visited_references.clear();
        let type_ = self.clear_names(type_);
        let lower_type = self.lower_type_internal(&type_);
        self.visited_references.clear();
        lower_type
    }
    fn clear_names(&self, type_: &Type) -> Type {
        let clear_names = |types: &Vec<Type>| {
            types
                .iter()
                .map(|type_| self.clear_names(type_))
                .collect::<Vec<_>>()
        };
        match type_ {
            Type::Atomic(atomic_type_enum) => Type::Atomic(atomic_type_enum.clone()),
            Type::Union(_, types) => Type::Union(
                String::new(),
                types
                    .iter()
                    .map(|type_| type_.as_ref().map(|type_| self.clear_names(&type_)))
                    .collect(),
            ),
            Type::Instantiation(type_, types) => {
                Type::Instantiation(type_.clone(), clear_names(types))
            }
            Type::Tuple(types) => Type::Tuple(clear_names(types)),
            Type::Function(args, ret) => {
                Type::Function(clear_names(args), Box::new(self.clear_names(&*ret)))
            }
            Type::Variable(var) => Type::Variable(var.clone()),
        }
    }
    fn lower_type_internal(&mut self, type_: &Type) -> IntermediateType {
        match type_ {
            Type::Atomic(atomic) => atomic.clone().into(),
            Type::Union(_, types) => {
                let type_ = self.clear_names(&Type::Union(String::new(), types.clone()));
                if !self.type_defs.contains_key(&type_) {
                    let reference = Rc::new(RefCell::new(IntermediateUnionType(Vec::new()).into()));
                    self.visited_references.insert(reference.as_ptr());
                    self.type_defs.insert(type_, reference);
                }
                let ctors = types
                    .iter()
                    .map(|type_: &Option<Type>| {
                        type_.as_ref().map(|type_| self.lower_type_internal(type_))
                    })
                    .collect();
                IntermediateUnionType(ctors).into()
            }
            Type::Instantiation(type_, params) => {
                let instantiation = self.clear_names(&type_.borrow().instantiate(params));
                match self.type_defs.entry(instantiation.clone()) {
                    std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                        if self
                            .visited_references
                            .contains(&occupied_entry.get().as_ptr())
                        {
                            IntermediateType::IntermediateReferenceType(
                                occupied_entry.get().clone(),
                            )
                        } else {
                            self.visited_references
                                .insert(occupied_entry.get().as_ptr());
                            occupied_entry.get().borrow().clone()
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                        let reference =
                            Rc::new(RefCell::new(IntermediateUnionType(Vec::new()).into()));
                        vacant_entry.insert(reference.clone());
                        self.visited_references.insert(reference.as_ptr());
                        let lower_type = self.lower_type_internal(&instantiation);
                        *reference.clone().borrow_mut() = lower_type;
                        self.type_defs.get(&instantiation).unwrap().borrow().clone()
                    }
                }
            }
            Type::Tuple(types) => IntermediateTupleType(self.lower_types_internal(types)).into(),
            Type::Function(args, ret) => IntermediateFnType(
                self.lower_types_internal(args),
                Box::new(self.lower_type(&*ret)),
            )
            .into(),
            Type::Variable(_) => panic!("Attempt to lower type variable."),
        }
    }
    pub fn lower_types_internal(&mut self, types: &Vec<Type>) -> Vec<IntermediateType> {
        types
            .iter()
            .map(|type_| self.lower_type_internal(type_))
            .collect()
    }
}

#[cfg(test)]
mod tests {

    use crate::Id;

    use super::*;

    use test_case::test_case;

    #[test_case(
        TypedExpression::Integer(Integer { value: 4 }),
        (
            IntermediateBuiltIn::Integer(Integer { value: 4 }).into(),
            Vec::new()
        );
        "integer"
    )]
    #[test_case(
        TypedExpression::Boolean(Boolean { value: true }),
        (
            IntermediateBuiltIn::Boolean(Boolean { value: true }).into(),
            Vec::new()
        );
        "boolean"
    )]
    #[test_case(
        TypedTuple{
            expressions: Vec::new()
        }.into(),
        {
            let value: IntermediateMemory = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(Vec::new())).into();
            (value.clone().into(), vec![value.into()])
        };
        "empty tuple"
    )]
    #[test_case(
        TypedTuple{
            expressions: vec![
                Integer{value: 3}.into(),
                Boolean{value: false}.into()
            ]
        }.into(),
        {
            let value: IntermediateMemory = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                vec![
                    IntermediateBuiltIn::Integer(Integer { value: 3 }).into(),
                    IntermediateBuiltIn::Boolean(Boolean { value: false }).into(),
                ]
            )).into();
            (value.clone().into(), vec![value.into()])
        };
        "non-empty tuple"
    )]
    #[test_case(
        TypedTuple{
            expressions: vec![
                TypedTuple{
                    expressions: Vec::new()
                }.into(),
                Integer{value: 1}.into(),
                TypedTuple{
                    expressions: vec![
                        Boolean{value: true}.into()
                    ]
                }.into(),
            ]
        }.into(),
        {
            let inner1: IntermediateMemory = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(Vec::new()).into()).into();
            let inner3: IntermediateMemory = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                vec![
                    IntermediateBuiltIn::Boolean(Boolean { value: true }).into(),
                ]
            )).into();
            let outer: IntermediateMemory = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                vec![
                    inner1.clone().into(),
                    IntermediateBuiltIn::Integer(Integer { value: 1 }).into(),
                    inner3.clone().into(),
                ]
            )).into();
            (outer.clone().into(), vec![inner1.into(), inner3.into(), outer.into()])
        };
        "nested tuple"
    )]
    fn test_lower_expression(
        expression: TypedExpression,
        value_statements: (IntermediateValue, Vec<IntermediateStatement>),
    ) {
        let (value, statements) = value_statements;
        let mut lowerer = Lowerer::new();
        let computation = lowerer.lower_expression(expression);
        assert_eq!(computation, value);
        assert_eq!(lowerer.statements, statements)
    }

    #[test_case(
        Type::Atomic(AtomicTypeEnum::INT),
        |_| AtomicTypeEnum::INT.into();
        "int"
    )]
    #[test_case(
        Type::Atomic(AtomicTypeEnum::BOOL),
        |_| AtomicTypeEnum::BOOL.into();
        "bool"
    )]
    #[test_case(
        Type::Tuple(Vec::new()),
        |_| IntermediateTupleType(Vec::new()).into();
        "empty tuple"
    )]
    #[test_case(
        Type::Tuple(vec![
            Type::Atomic(AtomicTypeEnum::INT),
            Type::Atomic(AtomicTypeEnum::BOOL),
        ]),
        |_| IntermediateTupleType(vec![
            AtomicTypeEnum::INT.into(),
            AtomicTypeEnum::BOOL.into(),
        ]).into();
        "flat tuple"
    )]
    #[test_case(
        Type::Tuple(vec![
            Type::Tuple(vec![
                Type::Atomic(AtomicTypeEnum::INT),
                Type::Atomic(AtomicTypeEnum::BOOL),
            ]),
            Type::Tuple(Vec::new()),
        ]),
        |_| IntermediateTupleType(vec![
            IntermediateTupleType(vec![
                AtomicTypeEnum::INT.into(),
                AtomicTypeEnum::BOOL.into(),
            ]).into(),
            IntermediateTupleType(Vec::new()).into()
        ]).into();
        "nested tuple"
    )]
    #[test_case(
        Type::Union(Id::from("Bull"), vec![None, None]),
        |_| {
            IntermediateUnionType(vec![None, None]).into()
        };
        "bull correct"
    )]
    #[test_case(
        Type::Union(
            Id::from("LR"),
            vec![
                Some(TYPE_INT),
                Some(TYPE_BOOL),
            ]
        ),
        |_| {
            IntermediateUnionType(vec![
                Some(AtomicTypeEnum::INT.into()),
                Some(AtomicTypeEnum::BOOL.into()),
            ]).into()
        };
        "left right"
    )]
    #[test_case(
        Type::Function(
            Vec::new(),
            Box::new(Type::Tuple(Vec::new()))
        ),
        |_| {
            IntermediateFnType(
                Vec::new(),
                Box::new(IntermediateTupleType(Vec::new()).into())
            ).into()
        };
        "unit function"
    )]
    #[test_case(
        Type::Function(
            vec![
                TYPE_INT,
                TYPE_INT,
            ],
            Box::new(TYPE_INT)
        ),
        |_| {
            IntermediateFnType(
                vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into())
            ).into()
        };
        "binary int function"
    )]
    #[test_case(
        {
            let parameter = Rc::new(RefCell::new(None));
            let type_ = Rc::new(RefCell::new(ParametricType {
                parameters: vec![parameter.clone()],
                type_: Type::Function(
                    vec![
                        Type::Variable(parameter.clone()),
                    ],
                    Box::new(Type::Variable(parameter)),
                )
            }));
            Type::Instantiation(type_, vec![TYPE_INT])
        },
        |_| {
            IntermediateFnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into())
            ).into()
        };
        "instantiated identity function"
    )]
    #[test_case(
        Type::Function(
            vec![
                Type::Function(
                    vec![
                        TYPE_INT,
                    ],
                    Box::new(TYPE_BOOL)
                ),
                TYPE_INT,
            ],
            Box::new(TYPE_BOOL)
        ),
        |_| {
            IntermediateFnType(
                vec![
                    IntermediateFnType(
                        vec![
                            AtomicTypeEnum::INT.into(),
                        ],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ).into(),
                    AtomicTypeEnum::INT.into()
                ],
                Box::new(AtomicTypeEnum::BOOL.into())
            ).into()
        };
        "higher order function"
    )]
    #[test_case(
        {
            let reference = Rc::new(RefCell::new(ParametricType::new()));
            let union_type = Type::Union(Id::from("nat"),vec![
                Some(Type::Instantiation(Rc::clone(&reference), Vec::new())),
                None,
            ]);
            *reference.borrow_mut() = union_type.clone().into();
            union_type
        },
        |type_defs| {
            assert_eq!(type_defs.len(), 1);
            let reference = type_defs.values().cloned().collect::<Vec<_>>()[0].clone();
            IntermediateUnionType(vec![
                Some(IntermediateType::IntermediateReferenceType(reference.into())),
                None
            ]).into()
        };
        "nat"
    )]
    #[test_case(
        {
            let reference = Rc::new(RefCell::new(ParametricType::new()));
            let union_type = Type::Union(Id::from("list_int"),vec![
                Some(Type::Tuple(vec![
                    TYPE_INT,
                    Type::Instantiation(Rc::clone(&reference), Vec::new()),
                ])),
                None,
            ]);
            *reference.borrow_mut() = union_type.clone().into();
            union_type
        },
        |type_defs| {
            assert_eq!(type_defs.len(), 1);
            let reference = type_defs.values().cloned().collect::<Vec<_>>()[0].clone();
            IntermediateUnionType(vec![
                Some(IntermediateTupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    IntermediateType::IntermediateReferenceType(reference.into())
                ]).into()),
                None
            ]).into()
        };
        "list int"
    )]
    #[test_case(
        {
            let parameters = vec![Rc::new(RefCell::new(None)), Rc::new(RefCell::new(None))];
            let pair = Rc::new(RefCell::new(ParametricType {
                parameters: parameters.clone(),
                type_: Type::Tuple(parameters.iter().map(|parameter| Type::Variable(parameter.clone())).collect()),
            }));
            Type::Instantiation(pair, vec![TYPE_INT, TYPE_BOOL])
        },
        |_| IntermediateTupleType(vec![
            AtomicTypeEnum::INT.into(),
            AtomicTypeEnum::BOOL.into(),
        ]).into()
        ;
        "instantiated pair int bool"
    )]
    #[test_case(
        {
            let parameter = Rc::new(RefCell::new(None));
            let type_ = Rc::new(RefCell::new(ParametricType {
                parameters: vec![parameter.clone()],
                type_: Type::Variable(parameter),
            }));
            Type::Instantiation(type_, vec![TYPE_INT])
        },
        |_| AtomicTypeEnum::INT.into();
        "transparent type alias"
    )]
    #[test_case(
        {
            let parameter = Rc::new(RefCell::new(None));
            let list_type = Rc::new(RefCell::new(ParametricType {
                parameters: vec![parameter.clone()],
                type_: Type::new(),
            }));
            list_type.borrow_mut().type_ = Type::Union(
                Id::from("List"),
                vec![
                    Some(Type::Tuple(vec![
                        Type::Variable(parameter.clone()),
                        Type::Instantiation(
                            list_type.clone(),
                            vec![Type::Variable(parameter.clone())],
                        ),
                    ])),
                    None,
                ],
            );
            Type::Instantiation(list_type.clone(), vec![TYPE_INT])
        },
        |type_defs| {
            assert_eq!(type_defs.len(), 1);
            let reference = type_defs.values().cloned().collect::<Vec<_>>()[0].clone();
            IntermediateUnionType(vec![
                Some(IntermediateTupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    IntermediateType::IntermediateReferenceType(reference.into())
                ]).into()),
                None
            ]).into()
        };
        "instantiated list int"
    )]
    fn test_lower_type(type_: Type, expected_gen: impl Fn(&TypeDefs) -> IntermediateType) {
        let mut lowerer = Lowerer::new();
        let type_ = lowerer.lower_type(&type_);
        let expected = expected_gen(&lowerer.type_defs);
        assert_eq!(type_, expected);
        assert!(lowerer.visited_references.is_empty())
    }
}
