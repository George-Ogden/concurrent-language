use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    iter::zip,
    rc::Rc,
};

use crate::intermediate_nodes::*;
use type_checker::*;

type Scope = HashMap<(Variable, Vec<Type>), IntermediateMemory>;
type History = HashMap<IntermediateExpression, IntermediateValue>;
type Uninstantiated = HashMap<Variable, ParametricExpression>;
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
        let mut lowerer = Lowerer {
            scope: Scope::new(),
            history: History::new(),
            uninstantiated: Uninstantiated::new(),
            type_defs: TypeDefs::new(),
            statements: Vec::new(),
            visited_references: VisitedReferences::new(),
        };
        DEFAULT_CONTEXT.with(|context| {
            lowerer.scope = HashMap::from_iter(context.iter().map(|(id, var)| {
                let type_ = lowerer.lower_type(&var.type_.type_);
                let variable = var.variable.clone();
                (
                    (variable, Vec::new()),
                    IntermediateExpression::IntermediateValue(
                        IntermediateBuiltIn::BuiltInFn(id.clone(), type_).into(),
                    )
                    .into(),
                )
            }))
        });
        lowerer
    }
    fn get_cached_value(
        &mut self,
        intermediate_expression: IntermediateExpression,
    ) -> IntermediateValue {
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
    fn lower_expression(&mut self, expression: TypedExpression) -> IntermediateValue {
        match expression {
            TypedExpression::Integer(integer) => IntermediateBuiltIn::Integer(integer).into(),
            TypedExpression::Boolean(boolean) => IntermediateBuiltIn::Boolean(boolean).into(),
            TypedExpression::TypedTuple(TypedTuple { expressions }) => {
                let intermediate_expressions = self.lower_expressions(expressions);
                let intermediate_expression: IntermediateExpression =
                    IntermediateTupleExpression(intermediate_expressions).into();
                self.get_cached_value(intermediate_expression)
            }
            TypedExpression::TypedAccess(TypedAccess {
                variable: TypedVariable { variable, type_ },
                parameters,
            }) => {
                if !self
                    .scope
                    .contains_key(&(variable.clone(), parameters.clone()))
                {
                    let uninstantiated = self.uninstantiated.get(&variable).unwrap();
                    let (expression, placeholder) = self
                        .add_placeholder_assignment(
                            TypedAssignment {
                                variable: TypedVariable {
                                    variable: variable.clone(),
                                    type_,
                                },
                                expression: uninstantiated.clone(),
                            },
                            Some(parameters.clone()),
                        )
                        .unwrap();
                    self.perform_assignment(expression, placeholder);
                };
                self.scope
                    .get(&(variable, parameters))
                    .unwrap()
                    .clone()
                    .into()
            }
            TypedExpression::TypedFunctionCall(TypedFunctionCall {
                function,
                arguments,
            }) => {
                let intermediate_function = self.lower_expression(*function);
                let intermediate_args = self.lower_expressions(arguments);
                let intermediate_expression = IntermediateFnCall {
                    fn_: intermediate_function,
                    args: intermediate_args,
                };
                self.get_cached_value(intermediate_expression.into())
            }
            TypedExpression::TypedFunctionDefinition(TypedFunctionDefinition {
                parameters,
                return_type: _,
                body,
            }) => {
                let variables = parameters
                    .iter()
                    .map(|variable| variable.variable.clone())
                    .collect::<Vec<_>>();
                let arguments = parameters
                    .iter()
                    .map(|variable| {
                        IntermediateArgument::from(self.lower_type(&variable.type_.type_))
                    })
                    .collect::<Vec<_>>();
                for (variable, argument) in zip(&variables, &arguments) {
                    self.scope
                        .insert((variable.clone(), Vec::new()), argument.clone().into());
                }
                let (statements, return_value) = self.lower_block(body);
                let intermediate_expression = IntermediateFnDef {
                    arguments: arguments,
                    statements: statements,
                    return_value: return_value,
                }
                .into();
                self.get_cached_value(intermediate_expression)
            }
            TypedExpression::TypedConstructorCall(TypedConstructorCall {
                idx,
                output_type,
                arguments,
            }) => {
                let IntermediateType::IntermediateUnionType(lower_type) =
                    self.lower_type(&output_type)
                else {
                    panic!("Expected constructor call to have union type.")
                };
                let lower_data = match &arguments[..] {
                    [] => None,
                    [argument] => Some(self.lower_expression(argument.clone())),
                    _ => panic!("Multiple arguments in constructor call."),
                };
                let intermediate_expression = IntermediateCtorCall {
                    idx,
                    data: lower_data,
                    type_: lower_type,
                }
                .into();
                self.get_cached_value(intermediate_expression)
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
    fn lower_block(
        &mut self,
        block: TypedBlock,
    ) -> (Vec<IntermediateStatement>, IntermediateValue) {
        let statements = self.statements.clone();
        let history = self.history.clone();
        self.statements = Vec::new();
        self.history = History::new();
        self.lower_assignments(block.assignments);
        let intermediate_value = self.lower_expression(*block.expression);
        let intermediate_statements = self.statements.clone();
        self.statements = statements;
        self.history = history;
        (intermediate_statements, intermediate_value)
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
    fn remove_wasted_allocations_from_expression(
        &mut self,
        expression: IntermediateExpression,
    ) -> IntermediateExpression {
        match expression {
            IntermediateExpression::IntermediateValue(value) => match value.clone() {
                IntermediateValue::IntermediateArgument(argument) => {
                    self.remove_wasted_allocations_from_expression(argument.into())
                }
                _ => IntermediateExpression::IntermediateValue(
                    self.remove_wasted_allocations_from_value(value),
                ),
            },
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx,
            }) => IntermediateElementAccess {
                value: self.remove_wasted_allocations_from_value(value),
                idx,
            }
            .into(),
            IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                values,
            )) => IntermediateTupleExpression(self.remove_wasted_allocations_from_values(values))
                .into(),
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args }) => {
                IntermediateFnCall {
                    fn_: self.remove_wasted_allocations_from_value(fn_),
                    args: self.remove_wasted_allocations_from_values(args),
                }
                .into()
            }
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                idx,
                data,
                type_,
            }) => IntermediateCtorCall {
                idx,
                data: data.map(|data| self.remove_wasted_allocations_from_value(data)),
                type_,
            }
            .into(),
            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                arguments,
                statements,
                return_value,
            }) => IntermediateFnDef {
                arguments,
                statements: self.remove_wasted_allocations_from_statements(statements),
                return_value: self.remove_wasted_allocations_from_value(return_value),
            }
            .into(),
        }
    }
    fn remove_wasted_allocations_from_value(
        &mut self,
        value: IntermediateValue,
    ) -> IntermediateValue {
        match value.clone() {
            IntermediateValue::IntermediateBuiltIn(built_in) => built_in.into(),
            IntermediateValue::IntermediateArgument(argument) => argument.into(),
            IntermediateValue::IntermediateMemory(IntermediateMemory(memory)) => {
                match memory.clone().borrow().clone() {
                    IntermediateExpression::IntermediateValue(value) => {
                        self.remove_wasted_allocations_from_value(value)
                    }
                    _ => IntermediateMemory(memory).into(),
                }
            }
        }
    }
    fn remove_wasted_allocations_from_values(
        &mut self,
        values: Vec<IntermediateValue>,
    ) -> Vec<IntermediateValue> {
        values
            .into_iter()
            .map(|value| self.remove_wasted_allocations_from_value(value))
            .collect()
    }
    fn remove_wasted_allocations_from_statement(
        &mut self,
        statement: IntermediateStatement,
    ) -> Option<IntermediateStatement> {
        match statement {
            IntermediateStatement::Assignment(assignment) => {
                let IntermediateMemory(memory) = assignment;
                if matches!(
                    &*memory.clone().borrow(),
                    IntermediateExpression::IntermediateValue(_)
                ) {
                    return None;
                }
                let condensed =
                    self.remove_wasted_allocations_from_expression(memory.clone().borrow().clone());
                *memory.borrow_mut() = condensed;
                Some(IntermediateStatement::Assignment(
                    IntermediateMemory(memory.clone()).into(),
                ))
            }
            IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                condition,
                branches,
            }) => Some(
                IntermediateIfStatement {
                    condition: self.remove_wasted_allocations_from_value(condition),
                    branches: (
                        self.remove_wasted_allocations_from_statements(branches.0),
                        self.remove_wasted_allocations_from_statements(branches.1),
                    ),
                }
                .into(),
            ),
            IntermediateStatement::IntermediateMatchStatement(match_statement) => {
                Some(match_statement.into())
            }
        }
    }
    fn remove_wasted_allocations_from_statements(
        &mut self,
        statements: Vec<IntermediateStatement>,
    ) -> Vec<IntermediateStatement> {
        statements
            .into_iter()
            .filter_map(|statement| self.remove_wasted_allocations_from_statement(statement))
            .collect()
    }
    pub fn lower_type(&mut self, type_: &Type) -> IntermediateType {
        self.visited_references.clear();
        let type_ = self.clear_names(type_);
        let lower_type = self.lower_type_internal(&type_);
        self.visited_references.clear();
        lower_type
    }
    fn lower_type_internal(&mut self, type_: &Type) -> IntermediateType {
        match type_ {
            Type::Atomic(atomic) => atomic.clone().into(),
            Type::Union(_, types) => {
                let type_ = self.clear_names(&Type::Union(String::new(), types.clone()));
                let lower_type = |this: &mut Self| {
                    IntermediateUnionType(
                        types
                            .iter()
                            .map(|type_: &Option<Type>| {
                                type_.as_ref().map(|type_| this.lower_type_internal(type_))
                            })
                            .collect(),
                    )
                    .into()
                };
                match self.type_defs.entry(type_.clone()) {
                    std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                        self.visited_references
                            .insert(occupied_entry.get().as_ptr());
                        lower_type(self)
                    }
                    std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                        let reference =
                            Rc::new(RefCell::new(IntermediateUnionType(Vec::new()).into()));
                        vacant_entry.insert(reference.clone());
                        self.visited_references.insert(reference.as_ptr());
                        let lower_type = lower_type(self);
                        *reference.clone().borrow_mut() = lower_type.clone();
                        lower_type
                    }
                }
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
                Box::new(self.lower_type_internal(&*ret)),
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
    fn add_placeholder_assignment(
        &mut self,
        assignment: TypedAssignment,
        parameters: Option<Vec<Type>>,
    ) -> Option<(TypedExpression, IntermediateMemory)> {
        let variable = assignment.variable;
        if parameters.is_none() && variable.type_.parameters.len() > 0 {
            self.uninstantiated
                .insert(variable.variable, assignment.expression);
            return None;
        }
        let parameters = parameters.unwrap_or(Vec::new());
        let expression = assignment.expression.instantiate(&parameters);
        let type_ = expression.type_();
        let lower_type = self.lower_type(&type_);
        let placeholder: IntermediateMemory = IntermediateArgument::from(lower_type).into();
        self.scope
            .insert((variable.variable.clone(), parameters), placeholder.clone());
        Some((expression, placeholder))
    }
    fn perform_assignment(&mut self, expression: TypedExpression, placeholder: IntermediateMemory) {
        let value = self.lower_expression(expression);
        *placeholder.0.borrow_mut() = value.into();
    }
    fn lower_assignments(&mut self, assignments: Vec<TypedAssignment>) {
        let expressions = assignments
            .into_iter()
            .filter_map(|assignment| self.add_placeholder_assignment(assignment, None))
            .collect::<Vec<_>>();
        for (expression, placeholder) in expressions {
            self.perform_assignment(expression, placeholder);
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{Id, Name};

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
    #[test_case(
        TypedFunctionCall{
            function: Box::new(
                TypedAccess {
                    variable: TypedVariable {
                        variable: DEFAULT_CONTEXT.with(|context| context.get(&Id::from("+")).unwrap().variable.clone()),
                        type_: Type::Function(vec![TYPE_INT, TYPE_INT], Box::new(TYPE_INT)).into(),
                    },
                    parameters :Vec::new()
                }.into()
            ),
            arguments: vec![
                Integer{value: 5}.into(),
                Integer{value: -4}.into(),
            ]
        }.into(),
        {
            let memory: IntermediateMemory = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: IntermediateBuiltIn::BuiltInFn(
                        Name::from("+"),
                        IntermediateFnType(
                            vec![AtomicTypeEnum::INT.into(), AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ).into(),
                args: vec![
                    IntermediateBuiltIn::Integer(Integer { value: 5 }).into(),
                    IntermediateBuiltIn::Integer(Integer { value: -4 }).into(),
                ]
            }).into();
            (memory.clone().into(), vec![memory.into()])
        };
        "operator call"
    )]
    #[test_case(
        {
            let parameters = vec![
                TYPE_INT.into(),
                TYPE_BOOL.into()
            ];
            TypedFunctionDefinition{
                parameters: parameters.clone(),
                return_type: Box::new(TYPE_INT),
                body: TypedBlock{
                    assignments: Vec::new(),
                    expression: Box::new(TypedAccess{
                        variable: parameters[0].clone().into(),
                        parameters: Vec::new()
                    }.into())
                }
            }.into()
        },
        {
            let arguments = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::BOOL).into(),
            ];
            let memory: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                arguments: arguments.clone(),
                statements: Vec::new(),
                return_value: arguments[0].clone().into()
            }).into();
            (memory.clone().into(), vec![memory.into()])
        };
        "projection fn def"
    )]
    #[test_case(
        {
            let parameters = vec![
                Type::Function(
                    vec![
                        TYPE_INT,
                    ],
                    Box::new(TYPE_INT)
                ).into(),
                TYPE_INT.into(),
            ];
            let y: TypedVariable = TYPE_INT.into();
            let z: TypedVariable = TYPE_INT.into();
            TypedFunctionDefinition{
                parameters: parameters.clone(),
                return_type: Box::new(TYPE_INT),
                body: TypedBlock{
                    assignments: vec![
                        TypedAssignment{
                            variable: y.clone(),
                            expression: ParametricExpression{
                                parameters: Vec::new(),
                                expression: TypedFunctionCall{
                                    function: Box::new(
                                        TypedAccess{
                                            variable: parameters[0].clone().into(),
                                            parameters: Vec::new()
                                        }.into()
                                    ),
                                    arguments: vec![
                                        TypedAccess {
                                            variable: parameters[1].clone().into(),
                                            parameters: Vec::new()
                                        }.into()
                                    ]
                                }.into()
                            }
                        },
                        TypedAssignment{
                            variable: z.clone(),
                            expression: ParametricExpression{
                                parameters: Vec::new(),
                                expression: TypedFunctionCall{
                                    function: Box::new(
                                        TypedAccess{
                                            variable: parameters[0].clone().into(),
                                            parameters: Vec::new()
                                        }.into()
                                    ),
                                    arguments: vec![
                                        TypedAccess {
                                            variable: y.clone(),
                                            parameters: Vec::new()
                                        }.into()
                                    ]
                                }.into()
                            }
                        }
                    ],
                    expression: Box::new(TypedAccess{
                        variable: z.clone(),
                        parameters: Vec::new()
                    }.into())
                }
            }.into()
        },
        {
            let arguments: Vec<IntermediateArgument> = vec![
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            let call1: IntermediateMemory = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: arguments[0].clone().into(),
                args: vec![arguments[1].clone().into()]
            }).into();
            let call2: IntermediateMemory = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: arguments[0].clone().into(),
                args: vec![call1.clone().into()]
            }).into();
            let fn_def: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                arguments: arguments.clone(),
                statements: vec![
                    IntermediateStatement::Assignment(call1),
                    IntermediateStatement::Assignment(call2.clone()),
                ],
                return_value: call2.into()
            }).into();
            (fn_def.clone().into(), vec![fn_def.into()])
        };
        "double apply fn def"
    )]
    #[test_case(
        TypedConstructorCall{
            idx: 0,
            output_type: Type::Union(
                Id::from("Bull"),
                vec![
                    None,
                    None
                ],
            ),
            arguments: Vec::new()
        }.into(),
        {
            let memory: IntermediateMemory = IntermediateExpression::from(
                IntermediateCtorCall{
                    idx: 0,
                    data: None,
                    type_: IntermediateUnionType(vec![None, None]).into()
                }
            ).into();
            (
                memory.clone().into(),
                vec![memory.into()]
            )
        }
        ;
        "data-free constructor"
    )]
    #[test_case(
        TypedConstructorCall{
            idx: 1,
            output_type: Type::Union(
                Id::from("Option_Int"),
                vec![
                    None,
                    Some(TYPE_INT),
                ],
            ),
            arguments: vec![
                Integer{value: 8}.into()
            ]
        }.into(),
        {
            let memory: IntermediateMemory = IntermediateExpression::from(
                IntermediateCtorCall{
                    idx: 1,
                    data: Some(IntermediateBuiltIn::from(Integer{value: 8}).into()),
                    type_: IntermediateUnionType(vec![None, Some(AtomicTypeEnum::INT.into())]).into()
                }
            ).into();
            (
                memory.clone().into(),
                vec![memory.into()]
            )
        };
        "data-value constructor"
    )]
    #[test_case(
        {
            let reference = Rc::new(RefCell::new(ParametricType::new()));
            let list_int_type = Type::Union(Id::from("list_int"),vec![
                Some(Type::Tuple(vec![
                    TYPE_INT,
                    Type::Instantiation(Rc::clone(&reference), Vec::new()),
                ])),
                None,
            ]);
            *reference.borrow_mut() = list_int_type.clone().into();
            TypedConstructorCall{
                idx: 1,
                output_type: list_int_type.clone(),
                arguments: vec![
                    TypedTuple{
                        expressions: vec![
                            Integer{value: -8}.into(),
                            TypedConstructorCall{
                                idx: 0,
                                output_type: list_int_type.clone(),
                                arguments: Vec::new()
                            }.into()
                        ]
                    }.into()
                ]
            }.into()
        },
        {
            let reference = Rc::new(RefCell::new(IntermediateTupleType(Vec::new()).into()));
            let union_type = IntermediateUnionType(vec![
                Some(IntermediateTupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    IntermediateType::IntermediateReferenceType(reference.clone().into())
                ]).into()),
                None
            ]);
            let list_int_type = IntermediateType::from(union_type.clone());
            *reference.borrow_mut() = list_int_type.clone();
            let nil: IntermediateMemory = IntermediateExpression::from(
                IntermediateCtorCall{
                    idx: 0,
                    data: None,
                    type_: union_type.clone()
                }
            ).into();
            let tuple: IntermediateMemory = IntermediateExpression::from(
                IntermediateTupleExpression(
                    vec![
                        IntermediateBuiltIn::from(Integer{value: -8}).into(),
                        nil.clone().into()
                    ]
                )
            ).into();
            let head: IntermediateMemory = IntermediateExpression::from(
                IntermediateCtorCall{
                    idx: 1,
                    data: Some(tuple.clone().into()),
                    type_: union_type
                }
            ).into();
            (
                head.clone().into(),
                vec![
                    nil.into(),
                    tuple.into(),
                    head.into()
                ]
            )
        };
        "recursive constructor"
    )]
    fn test_lower_expression(
        expression: TypedExpression,
        value_statements: (IntermediateValue, Vec<IntermediateStatement>),
    ) {
        let (value, statements) = value_statements;
        let mut lowerer = Lowerer::new();
        let computation = lowerer.lower_expression(expression);
        let efficient_computation = lowerer.remove_wasted_allocations_from_value(computation);
        let efficient_statements =
            lowerer.remove_wasted_allocations_from_statements(lowerer.statements.clone());
        assert_eq!(efficient_computation, value);
        assert_eq!(efficient_statements, statements)
    }

    #[test]
    fn test_projection_equalities() {
        let p0: IntermediateMemory = {
            let arguments = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                arguments: arguments.clone(),
                statements: Vec::new(),
                return_value: arguments[0].clone().into(),
            })
            .into()
        };
        let p1: IntermediateMemory = {
            let arguments = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                arguments: arguments.clone(),
                statements: Vec::new(),
                return_value: arguments[1].clone().into(),
            })
            .into()
        };
        let q0: IntermediateMemory = {
            let arguments = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                arguments: arguments.clone(),
                statements: Vec::new(),
                return_value: arguments[0].clone().into(),
            })
            .into()
        };
        let q1: IntermediateMemory = {
            let arguments = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                arguments: arguments.clone(),
                statements: Vec::new(),
                return_value: arguments[1].clone().into(),
            })
            .into()
        };

        assert_eq!(p0, q0);
        assert_eq!(p1, q1);
        assert_ne!(p0, p1);
        assert_ne!(q0, q1);
        assert_ne!(p0, q1);
        assert_ne!(p1, q0);
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

    #[ignore]
    #[test]
    fn test_blowup_type() {
        let parameter = Rc::new(RefCell::new(None));
        let blowup_type = Rc::new(RefCell::new(ParametricType {
            parameters: vec![parameter.clone()],
            type_: Type::new(),
        }));
        blowup_type.borrow_mut().type_ = Type::Union(
            Id::from("List"),
            vec![Some(Type::Instantiation(
                blowup_type.clone(),
                vec![Type::Tuple(vec![
                    Type::Variable(parameter.clone()),
                    Type::Variable(parameter.clone()),
                ])],
            ))],
        );
        let type_ = Type::Instantiation(blowup_type.clone(), vec![TYPE_INT]);

        let mut lowerer = Lowerer::new();
        lowerer.lower_type(&type_);
    }

    #[test]
    fn test_lower_types_without_interference() {
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
                    Type::Instantiation(list_type.clone(), vec![Type::Variable(parameter.clone())]),
                ])),
                None,
            ],
        );
        let list_bool = Type::Instantiation(list_type.clone(), vec![TYPE_BOOL]);
        let list_int = Type::Instantiation(list_type.clone(), vec![TYPE_INT]);
        let inst_list_bool = list_type.borrow().instantiate(&vec![TYPE_BOOL]);
        let inst_list_int = list_type.borrow().instantiate(&vec![TYPE_INT]);

        let mut lowerer = Lowerer::new();
        let lower_list_int = lowerer.lower_type(&list_int);
        let lower_inst_list_bool = lowerer.lower_type(&inst_list_bool);
        let lower_inst_list_int = lowerer.lower_type(&inst_list_int);
        let lower_list_bool = lowerer.lower_type(&list_bool);

        assert_ne!(lower_list_bool, lower_list_int);
        assert_ne!(lower_list_bool, lower_inst_list_int);
        assert_ne!(lower_inst_list_bool, lower_list_int);
        assert_ne!(lower_inst_list_bool, lower_inst_list_int);

        assert_eq!(lower_inst_list_bool, lower_list_bool);
        assert_eq!(lower_inst_list_int, lower_list_int);
    }

    #[test_case(
        (Vec::new(), Vec::new());
        "no statements"
    )]
    #[test_case(
        {
            let x: TypedVariable = TYPE_INT.into();
            (
                vec![
                    TypedAssignment {
                        variable: x.clone(),
                        expression: TypedExpression::Integer(Integer { value: 5 }).into()
                    }
                ],
                vec![
                    (
                        x.variable,
                        IntermediateBuiltIn::Integer(Integer { value: 5 }).into(),
                    )
                ]
            )
        };
        "simple statement"
    )]
    #[test_case(
        {
            let x: TypedVariable = Type::Tuple(vec![TYPE_INT, TYPE_BOOL]).into();
            let value: IntermediateMemory = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                vec![
                    IntermediateBuiltIn::Integer(Integer { value: 3 }).into(),
                    IntermediateBuiltIn::Boolean(Boolean { value: false }).into(),
                ]
            )).into();
            (
                vec![
                    TypedAssignment {
                        variable: x.clone(),
                        expression: TypedExpression::TypedTuple(TypedTuple{
                            expressions: vec![
                                Integer{value: 3}.into(),
                                Boolean{value: false}.into()
                            ]
                        }).into(),
                    }
                ],
                vec![
                    (
                        x.variable,
                        value.into()
                    )
                ]
            )
        };
        "compound statement"
    )]
    #[test_case(
        {
            let x: TypedVariable = TYPE_INT.into();
            let y: TypedVariable = TYPE_BOOL.into();
            let value: IntermediateMemory = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                vec![
                    IntermediateBuiltIn::Integer(Integer { value: 3 }).into(),
                    IntermediateBuiltIn::Boolean(Boolean { value: false }).into(),
                ]
            )).into();
            (
                vec![
                    TypedAssignment {
                        variable: x.clone(),
                        expression: TypedExpression::Integer(Integer { value: 3 }).into()
                    },
                    TypedAssignment {
                        variable: y.clone(),
                        expression: TypedExpression::TypedTuple(TypedTuple{
                            expressions: vec![
                                TypedAccess{
                                    variable: x.clone(),
                                    parameters: Vec::new()
                                }.into(),
                                Boolean{value: false}.into()
                            ]
                        }).into(),
                    }
                ],
                vec![
                    (
                        x.variable,
                        IntermediateBuiltIn::Integer(Integer { value: 3 }).into(),
                    ),
                    (
                        y.variable,
                        value.into()
                    )
                ]
            )
        };
        "dual assignment"
    )]
    #[test_case(
        {
            let f: TypedVariable = Type::Function(Vec::new(), Box::new(TYPE_INT)).into();
            let y: TypedVariable = TYPE_INT.into();
            let argument: IntermediateArgument = IntermediateType::from(AtomicTypeEnum::INT).into();
            let fn_: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef{
                arguments: vec![argument.clone()],
                statements: Vec::new(),
                return_value: argument.clone().into()
            }).into();
            let value: IntermediateMemory = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: fn_.clone().into(),
                args: vec![IntermediateBuiltIn::Integer(Integer { value: 11 }).into()]
            }).into();
            let parameter: TypedVariable = TYPE_INT.into();
            (
                vec![
                    TypedAssignment {
                        variable: f.clone(),
                        expression: TypedExpression::TypedFunctionDefinition(TypedFunctionDefinition{
                            parameters: vec![parameter.clone()],
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock{
                                assignments: Vec::new(),
                                expression: Box::new(TypedAccess{
                                    variable: parameter.clone().into(),
                                    parameters: Vec::new()
                                }.into())
                            }
                        }).into()
                    },
                    TypedAssignment {
                        variable: y.clone(),
                        expression: TypedExpression::TypedFunctionCall(TypedFunctionCall{
                            function: Box::new(
                                TypedAccess{
                                    variable: f.clone(),
                                    parameters: Vec::new()
                                }.into()
                            ),
                            arguments: vec![
                                Integer{value: 11}.into()
                            ]
                        }).into(),
                    }
                ],
                vec![
                    (
                        f.variable,
                        fn_.into()
                    ),
                    (
                        y.variable,
                        value.into()
                    )
                ]
            )
        };
        "user-defined fn call"
    )]
    #[test_case(
        {
            let foo: TypedVariable = Type::Function(Vec::new(), Box::new(TYPE_INT)).into();
            let y: TypedVariable = TYPE_INT.into();
            let fn_: IntermediateMemory = IntermediateArgument::from(
                IntermediateType::from(IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())))
            ).into();
            let recursive_call: IntermediateMemory = IntermediateExpression::IntermediateFnCall(
                IntermediateFnCall{
                    fn_: fn_.clone().into(),
                    args: Vec::new()
                }
            ).into();
            *fn_.0.clone().borrow_mut() = IntermediateExpression::IntermediateFnDef(IntermediateFnDef{
                arguments: Vec::new(),
                statements: vec![
                    recursive_call.clone().into()
                ],
                return_value: recursive_call.into()
            }).into();
            let value = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: fn_.clone().into(),
                args: Vec::new()
            });
            (
                vec![
                    TypedAssignment {
                        variable: foo.clone(),
                        expression: TypedExpression::TypedFunctionDefinition(TypedFunctionDefinition{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock{
                                assignments: Vec::new(),
                                expression: Box::new(TypedFunctionCall{
                                    function: Box::new(
                                        TypedAccess{
                                            variable: foo.clone().into(),
                                            parameters: Vec::new()
                                        }.into()
                                    ),
                                    arguments: Vec::new()
                                }.into())
                            }
                        }).into()
                    },
                    TypedAssignment {
                        variable: y.clone(),
                        expression: TypedExpression::TypedFunctionCall(TypedFunctionCall{
                            function: Box::new(
                                TypedAccess{
                                    variable: foo.clone().into(),
                                    parameters: Vec::new()
                                }.into()
                            ),
                            arguments: Vec::new()
                        }).into(),
                    }
                ],
                vec![
                    (
                        foo.variable,
                        fn_.into()
                    ),
                    (
                        y.variable,
                        value.into()
                    )
                ]
            )
        };
        "recursive fn call"
    )]
    #[test_case(
        {
            let a: TypedVariable = Type::Function(Vec::new(), Box::new(TYPE_BOOL)).into();
            let b: TypedVariable = Type::Function(Vec::new(), Box::new(TYPE_BOOL)).into();
            let a_fn: IntermediateMemory = IntermediateArgument::from(
                IntermediateType::from(IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::BOOL.into())))
            ).into();
            let b_fn: IntermediateMemory = IntermediateArgument::from(
                IntermediateType::from(IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::BOOL.into())))
            ).into();
            let a_call: IntermediateMemory = IntermediateExpression::IntermediateFnCall(
                IntermediateFnCall{
                    fn_: a_fn.clone().into(),
                    args: Vec::new()
                }
            ).into();
            let b_call: IntermediateMemory = IntermediateExpression::IntermediateFnCall(
                IntermediateFnCall{
                    fn_: b_fn.clone().into(),
                    args: Vec::new()
                }
            ).into();
            *a_fn.0.clone().borrow_mut() = IntermediateExpression::IntermediateFnDef(IntermediateFnDef{
                arguments: Vec::new(),
                statements: vec![
                    b_call.clone().into()
                ],
                return_value: b_call.into()
            }).into();
            *b_fn.0.clone().borrow_mut() = IntermediateExpression::IntermediateFnDef(IntermediateFnDef{
                arguments: Vec::new(),
                statements: vec![
                    a_call.clone().into()
                ],
                return_value: a_call.into()
            }).into();
            (
                vec![
                    TypedAssignment {
                        variable: a.clone(),
                        expression: TypedExpression::TypedFunctionDefinition(TypedFunctionDefinition{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_BOOL),
                            body: TypedBlock{
                                assignments: Vec::new(),
                                expression: Box::new(TypedFunctionCall{
                                    function: Box::new(
                                        TypedAccess{
                                            variable: b.clone(),
                                            parameters: Vec::new()
                                        }.into()
                                    ),
                                    arguments: Vec::new()
                                }.into())
                            }
                        }).into()
                    },
                    TypedAssignment {
                        variable: b.clone(),
                        expression: TypedExpression::TypedFunctionDefinition(TypedFunctionDefinition{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_BOOL),
                            body: TypedBlock{
                                assignments: Vec::new(),
                                expression: Box::new(TypedFunctionCall{
                                    function: Box::new(
                                        TypedAccess{
                                            variable: a.clone(),
                                            parameters: Vec::new()
                                        }.into()
                                    ),
                                    arguments: Vec::new()
                                }.into())
                            }
                        }).into()
                    },
                ],
                vec![
                    (
                        a.variable,
                        a_fn.into()
                    ),
                    (
                        b.variable,
                        b_fn.into()
                    )
                ]
            )
        };
        "mutually recursive fn calls"
    )]
    #[test_case(
        {
        let parameter = Rc::new(RefCell::new(None));
        let id_type = ParametricType {
            parameters: vec![parameter.clone()],
            type_: Type::Function(
                vec![
                    Type::Variable(parameter.clone()),
                ],
                Box::new(Type::Variable(parameter.clone())),
            )
        };
        let id: TypedVariable = id_type.clone().into();
        let id_int: TypedVariable = id_type.instantiate(&vec![TYPE_INT]).into();
        let id_bool: TypedVariable = id_type.instantiate(&vec![TYPE_BOOL]).into();
        let id_bool2: TypedVariable = id_type.instantiate(&vec![TYPE_BOOL]).into();
        let x = TypedVariable {
            variable: Variable::new(),
            type_: Type::Variable(parameter.clone()).into(),
        };
        let int_argument: IntermediateArgument = IntermediateType::from(AtomicTypeEnum::INT).into();
        let bool_argument: IntermediateArgument = IntermediateType::from(AtomicTypeEnum::BOOL).into();
        let id_int_fn: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
            arguments: vec![int_argument.clone()],
            statements: Vec::new(),
            return_value: int_argument.into()
        }).into();
        let id_bool_fn: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
            arguments: vec![bool_argument.clone()],
            statements: Vec::new(),
            return_value: bool_argument.into()
        }).into();
        (
            vec![
                TypedAssignment{
                    variable: id.clone(),
                    expression: ParametricExpression {
                        expression: TypedFunctionDefinition{
                            parameters: vec![x.clone()],
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock{
                                assignments: Vec::new(),
                                expression: Box::new(TypedAccess{
                                    variable: x.clone(),
                                    parameters: Vec::new()
                                }.into())
                            }
                        }.into(),
                        parameters: vec![(String::from("T"),parameter.clone())]
                    },
                },
                TypedAssignment{
                    variable: id_int.clone(),
                    expression: ParametricExpression{
                        expression: TypedAccess{
                            variable: id.clone(),
                            parameters: vec![TYPE_INT]
                        }.into(),
                        parameters: Vec::new()
                    }
                },
                TypedAssignment{
                    variable: id_bool.clone(),
                    expression: ParametricExpression{
                        expression: TypedAccess{
                            variable: id.clone(),
                            parameters: vec![TYPE_BOOL]
                        }.into(),
                        parameters: Vec::new()
                    }
                },
                TypedAssignment{
                    variable: id_bool2.clone(),
                    expression: ParametricExpression{
                        expression: TypedAccess{
                            variable: id.clone(),
                            parameters: vec![TYPE_BOOL]
                        }.into(),
                        parameters: Vec::new()
                    }
                }
            ],
            vec![
                (
                    id_int.variable,
                    id_int_fn.into()
                ),
                (
                    id_bool.variable,
                    id_bool_fn.clone().into()
                ),
                (
                    id_bool2.variable,
                    id_bool_fn.into()
                ),
            ]
        )
        };
        "parametric identity function"
    )]
    fn test_lower_assignments(
        assignments_scope: (Vec<TypedAssignment>, Vec<(Variable, IntermediateValue)>),
    ) {
        let (assignments, expected_scope) = assignments_scope;
        let mut lowerer = Lowerer::new();
        lowerer.lower_assignments(assignments);
        lowerer.remove_wasted_allocations_from_statements(lowerer.statements.clone());
        let flat_scope = lowerer
            .scope
            .clone()
            .into_iter()
            .map(|(k, v)| (k, v.0.borrow().clone()))
            .collect::<HashMap<_, _>>();
        for (k, v) in expected_scope {
            let value = flat_scope
                .get(&(k, Vec::new()))
                .as_ref()
                .map(|&v| lowerer.remove_wasted_allocations_from_expression(v.clone()));
            assert_eq!(value, Some(v.into()))
        }
    }
}
