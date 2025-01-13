use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::intermediate_nodes::*;
use type_checker::*;

type Scope = HashMap<Variable, IntermediateValue>;
type History = HashMap<IntermediateExpression, IntermediateValue>;
type Uninstantiated = HashMap<(Variable, Rc<RefCell<Option<Type>>>), IntermediateExpression>;
type TypeDefs = HashMap<Type, Rc<RefCell<IntermediateType>>>;

struct Lowerer {
    scope: Scope,
    history: History,
    uninstantiated: Uninstantiated,
    type_defs: TypeDefs,
    statements: Vec<IntermediateStatement>,
}

impl Lowerer {
    pub fn new() -> Self {
        Lowerer {
            scope: HashMap::new(),
            history: HashMap::new(),
            uninstantiated: HashMap::new(),
            type_defs: HashMap::new(),
            statements: Vec::new(),
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
    fn lower_type(&mut self, type_: &Type) -> IntermediateType {
        match type_ {
            Type::Atomic(atomic) => atomic.clone().into(),
            Type::Union(_, types) => {
                let ctors = types
                    .iter()
                    .map(|type_: &Option<Type>| type_.as_ref().map(|type_| self.lower_type(type_)))
                    .collect();
                IntermediateUnionType(ctors).into()
            }
            Type::Instantiation(type_, params) => {
                let instantiation = type_.borrow().instantiate(params);
                match self.type_defs.entry(instantiation.clone()) {
                    std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                        IntermediateType::IntermediateReferenceType(occupied_entry.get().clone())
                    }
                    std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                        let reference =
                            Rc::new(RefCell::new(IntermediateUnionType(Vec::new()).into()));
                        vacant_entry.insert(reference.clone());
                        let lower_type = self.lower_type(&instantiation);
                        *reference.clone().borrow_mut() = lower_type;
                        IntermediateType::IntermediateReferenceType(reference)
                    }
                }
            }
            Type::Tuple(types) => IntermediateTupleType(self.lower_types(types)).into(),
            _ => todo!(),
        }
    }
    fn lower_types(&mut self, types: &Vec<Type>) -> Vec<IntermediateType> {
        types.iter().map(|type_| self.lower_type(type_)).collect()
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
    fn test_lower_type(type_: Type, expected_gen: impl Fn(&TypeDefs) -> IntermediateType) {
        let mut lowerer = Lowerer::new();
        let type_ = lowerer.lower_type(&type_);
        let expected = expected_gen(&lowerer.type_defs);
        assert_eq!(type_, expected);
    }
}
