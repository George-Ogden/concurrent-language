use crate::type_check_nodes::{
    ConstructorType, GenericVariables, ParametricType, Type, TypeCheckError, TypeContext,
    TypeDefinitions, TypedAccess, TypedAssignment, TypedBlock, TypedConstructorCall,
    TypedElementAccess, TypedExpression, TypedFunctionCall, TypedIf, TypedLambdaDef, TypedMatch,
    TypedMatchBlock, TypedMatchItem, TypedProgram, TypedTuple, TypedVariable, TYPE_BOOL, TYPE_INT,
};
use crate::utils::UniqueError;
use crate::{
    utils, AtomicType, AtomicTypeEnum, Block, ConstructorCall, Definition, ElementAccess,
    EmptyTypeDefinition, Expression, FunctionCall, FunctionDefinition, FunctionType, GenericType,
    GenericTypeVariable, GenericVariable, Id, IfExpression, MatchExpression, OpaqueTypeDefinition,
    ParametricExpression, Program, TransparentTypeDefinition, TupleExpression, TupleType,
    TypeAtomic, TypeFn, TypeInstance, TypeInstantiation, TypeTuple, TypeUnion, TypeVariable,
    TypedFnDef, UnionTypeDefinition, Variable,
};
use itertools::Either::*;
use itertools::Itertools;
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use strum::IntoEnumIterator;

thread_local! {pub static DEFAULT_CONTEXT: Lazy<TypeContext> = Lazy::new(|| {
    let integer_binary_operators = [
        "**", "*", "/", "%", "+", "-", ">>", "<<", "<=>", "&", "^", "|",
    ]
    .into_iter()
    .map(|operator| {
        (
            Id::from(operator),
            Type::from(TypeFn(vec![TYPE_INT, TYPE_INT], Box::new(TYPE_INT))),
        )
    });
    let integer_unary_operators = ["++", "--"].into_iter().map(|operator| {
        (
            Id::from(operator),
            Type::from(TypeFn(vec![TYPE_INT], Box::new(TYPE_INT))),
        )
    });
    let boolean_unary_operators = ["!"].into_iter().map(|operator| {
        (
            Id::from(operator),
            Type::from(TypeFn(vec![TYPE_BOOL], Box::new(TYPE_BOOL))),
        )
    });
    let boolean_binary_operators = ["&&", "||"].into_iter().map(|operator| {
        (
            Id::from(operator),
            Type::from(TypeFn(vec![TYPE_BOOL, TYPE_BOOL], Box::new(TYPE_BOOL))),
        )
    });
    let integer_comparisons = ["<", "<=", ">", ">=", "==", "!="]
        .into_iter()
        .map(|operator| {
            (
                Id::from(operator),
                Type::from(TypeFn(vec![TYPE_INT, TYPE_INT], Box::new(TYPE_BOOL))),
            )
        });
    TypeContext::from_iter(
        integer_binary_operators
            .chain(boolean_binary_operators)
            .chain(integer_comparisons)
            .chain(integer_unary_operators)
            .chain(boolean_unary_operators)
            .map(|(id, type_)| (id, type_.into())),
    )
});
}

#[derive(Debug)]
pub struct TypeChecker {
    type_definitions: TypeDefinitions,
    constructors: HashMap<Id, ConstructorType>,
}

impl TypeChecker {
    fn convert_ast_type(
        type_instance: TypeInstance,
        type_definitions: &TypeDefinitions,
        generic_variables: &GenericVariables,
    ) -> Result<Type, TypeCheckError> {
        Ok(match type_instance {
            TypeInstance::AtomicType(AtomicType {
                type_: atomic_type_enum,
            }) => TypeAtomic(atomic_type_enum).into(),
            TypeInstance::GenericType(GenericType { id, type_variables }) => {
                if let Some(reference) = generic_variables.get(&id) {
                    if type_variables.is_empty() {
                        TypeVariable(reference.clone()).into()
                    } else {
                        return Err(TypeCheckError::InstantiationOfTypeVariable {
                            variable: id,
                            type_instances: type_variables.clone(),
                        });
                    }
                } else if let Some(reference) = type_definitions.get(&id) {
                    if type_variables.len() != reference.borrow().parameters.len() {
                        return Err(TypeCheckError::WrongNumberOfTypeParameters {
                            type_: reference.borrow().clone(),
                            type_instances: type_variables.clone(),
                        });
                    }
                    TypeInstantiation {
                        reference: reference.clone(),
                        instances: type_variables
                            .into_iter()
                            .map(|type_instance| {
                                TypeChecker::convert_ast_type(
                                    type_instance,
                                    type_definitions,
                                    generic_variables,
                                )
                            })
                            .collect::<Result<_, _>>()?,
                    }
                    .into()
                } else {
                    return Err(TypeCheckError::UnknownError {
                        id,
                        options: type_definitions
                            .keys()
                            .chain(generic_variables.keys())
                            .map(|id| id.clone())
                            .collect_vec(),
                        place: String::from("type name"),
                    });
                }
            }
            TypeInstance::TupleType(TupleType { types }) => TypeTuple(
                types
                    .into_iter()
                    .map(|t| TypeChecker::convert_ast_type(t, type_definitions, generic_variables))
                    .collect::<Result<_, _>>()?,
            )
            .into(),
            TypeInstance::FunctionType(FunctionType {
                argument_types,
                return_type,
            }) => TypeFn(
                argument_types
                    .into_iter()
                    .map(|type_| {
                        TypeChecker::convert_ast_type(type_, type_definitions, generic_variables)
                    })
                    .collect::<Result<_, _>>()?,
                Box::new(TypeChecker::convert_ast_type(
                    *return_type,
                    type_definitions,
                    generic_variables,
                )?),
            )
            .into(),
        })
    }
    fn check_type_definitions(definitions: Vec<Definition>) -> Result<Self, TypeCheckError> {
        let type_names = definitions.iter().map(|definition| definition.get_id());
        let all_type_parameters = definitions.iter().map(Definition::get_parameters);
        let predefined_type_names = AtomicTypeEnum::iter()
            .map(|a| AtomicTypeEnum::to_string(&a).to_lowercase())
            .collect_vec();
        if let Err(UniqueError { duplicate }) =
            utils::check_unique(type_names.clone().chain(predefined_type_names.iter()))
        {
            if predefined_type_names.contains(&duplicate) {
                return Err(TypeCheckError::BuiltInOverride {
                    name: duplicate.clone(),
                    reason: String::from("type name"),
                });
            } else {
                return Err(TypeCheckError::DuplicatedName {
                    duplicate: duplicate.clone(),
                    reason: String::from("type name"),
                });
            }
        }
        for (type_name, type_parameters) in type_names.clone().zip(all_type_parameters.clone()) {
            if type_parameters.contains(&type_name) {
                return Err(TypeCheckError::TypeAsParameter {
                    type_name: type_name.clone(),
                });
            }
        }
        for type_parameters in all_type_parameters.clone() {
            if let Err(UniqueError { duplicate }) =
                utils::check_unique(type_parameters.iter().chain(predefined_type_names.iter()))
            {
                if predefined_type_names.contains(&duplicate) {
                    return Err(TypeCheckError::BuiltInOverride {
                        name: duplicate.clone(),
                        reason: String::from("type parameter"),
                    });
                } else {
                    return Err(TypeCheckError::DuplicatedName {
                        duplicate: duplicate.clone(),
                        reason: String::from("type parameter"),
                    });
                }
            }
        }
        let mut type_definitions: TypeDefinitions = type_names
            .zip(all_type_parameters)
            .map(|(id, parameters)| {
                (
                    id.clone(),
                    ParametricType {
                        type_: Type::new(),
                        parameters: (0..parameters.len())
                            .map(|_| Rc::new(RefCell::new(None)))
                            .collect_vec(),
                    },
                )
            })
            .collect();
        let mut constructors = HashMap::new();
        let transparent_definitions = definitions
            .iter()
            .map(|definition| {
                if let Definition::TransparentTypeDefinition(TransparentTypeDefinition {
                    variable:
                        GenericTypeVariable {
                            id,
                            generic_variables: _,
                        },
                    type_: _,
                }) = definition
                {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect_vec();
        for definition in definitions {
            let type_name = definition.get_id().clone();
            let type_reference = &type_definitions[&type_name];
            let type_ = match definition {
                Definition::OpaqueTypeDefinition(OpaqueTypeDefinition {
                    variable:
                        GenericTypeVariable {
                            id,
                            generic_variables,
                        },
                    type_,
                }) => {
                    if let Some(_) = constructors.insert(
                        id.clone(),
                        ConstructorType {
                            type_: type_reference.clone(),
                            index: 0,
                        },
                    ) {
                        return Err(TypeCheckError::DuplicatedName {
                            duplicate: id,
                            reason: String::from("constructor name"),
                        });
                    }
                    TypeUnion {
                        id: id.clone(),
                        variants: vec![Some(TypeChecker::convert_ast_type(
                            type_,
                            &type_definitions,
                            &GenericVariables::from((&generic_variables, &type_definitions[&id])),
                        )?)],
                    }
                    .into()
                }
                Definition::UnionTypeDefinition(UnionTypeDefinition {
                    variable:
                        GenericTypeVariable {
                            id,
                            generic_variables,
                        },
                    items,
                }) => {
                    let variant_names = items.iter().map(|item| &item.id);
                    match utils::check_unique(variant_names.clone()) {
                        Ok(()) => {}
                        Err(UniqueError { duplicate }) => {
                            return Err(TypeCheckError::DuplicatedName {
                                duplicate: duplicate.clone(),
                                reason: String::from("variant name"),
                            })
                        }
                    }
                    let variants = items.into_iter().enumerate().map(|(index, item)| {
                        if let Some(_) = constructors.insert(
                            item.id.clone(),
                            ConstructorType {
                                type_: type_reference.clone(),
                                index,
                            },
                        ) {
                            return Err(TypeCheckError::DuplicatedName {
                                duplicate: item.id,
                                reason: String::from("constructor name"),
                            });
                        }
                        item.type_
                            .map(|type_instance| {
                                TypeChecker::convert_ast_type(
                                    type_instance,
                                    &type_definitions,
                                    &GenericVariables::from((
                                        &generic_variables,
                                        &type_definitions[&id],
                                    )),
                                )
                            })
                            .transpose()
                    });
                    TypeUnion {
                        id: id.clone(),
                        variants: variants.collect::<Result<_, _>>()?,
                    }
                    .into()
                }
                Definition::TransparentTypeDefinition(TransparentTypeDefinition {
                    variable:
                        GenericTypeVariable {
                            id,
                            generic_variables,
                        },
                    type_,
                }) => TypeChecker::convert_ast_type(
                    type_,
                    &type_definitions,
                    &GenericVariables::from((&generic_variables, &type_definitions[&id])),
                )?,
                Definition::EmptyTypeDefinition(EmptyTypeDefinition { id }) => {
                    if let Some(_) = constructors.insert(
                        id.clone(),
                        ConstructorType {
                            type_: type_reference.clone(),
                            index: 0,
                        },
                    ) {
                        return Err(TypeCheckError::DuplicatedName {
                            duplicate: id,
                            reason: String::from("constructor name"),
                        });
                    }
                    Type::from(TypeUnion {
                        id: id,
                        variants: vec![None],
                    })
                }
                Definition::Assignment(_) => continue,
            };
            if let Some(type_reference) = type_definitions.get_mut(&type_name) {
                type_reference.borrow_mut().type_ = type_;
            } else {
                panic!("{} not found in type definitions", type_name)
            }
        }
        transparent_definitions.into_iter().try_for_each(|id| {
            id.map_or(Ok(()), |id| {
                if TypeChecker::is_self_recursive(&id, &type_definitions).is_err() {
                    Err(TypeCheckError::RecursiveTypeAlias { type_alias: id })
                } else {
                    Ok(())
                }
            })
        })?;

        return Ok(TypeChecker {
            type_definitions,
            constructors,
        });
    }
    fn is_self_recursive(id: &Id, definitions: &TypeDefinitions) -> Result<(), ()> {
        let start = definitions.get(id).unwrap();
        let mut queue = VecDeque::from([start.clone()]);
        let mut visited: HashMap<*mut ParametricType, bool> =
            HashMap::from_iter(definitions.values().map(|p| (p.as_ptr(), false)));
        fn update_queue(
            type_: &Type,
            start: &Rc<RefCell<ParametricType>>,
            queue: &mut VecDeque<Rc<RefCell<ParametricType>>>,
            visited: &mut HashMap<*mut ParametricType, bool>,
        ) -> Result<(), ()> {
            match type_ {
                Type::TypeUnion(TypeUnion {
                    id: _,
                    variants: items,
                }) => {
                    for type_ in items {
                        if let Some(type_) = type_ {
                            update_queue(type_, start, queue, visited)?;
                        }
                    }
                }
                Type::TypeInstantiation(TypeInstantiation {
                    reference: rc,
                    instances: ts,
                }) => {
                    if rc.as_ptr() == start.as_ptr() {
                        return Err(());
                    }
                    if !visited.get(&rc.as_ptr()).unwrap() {
                        visited.insert(rc.as_ptr(), true);
                        queue.push_back(rc.clone());
                    }
                    update_queue(&TypeTuple(ts.clone()).into(), start, queue, visited)?
                }
                Type::TypeTuple(TypeTuple(types)) => {
                    for type_ in types {
                        update_queue(type_, start, queue, visited)?;
                    }
                }
                Type::TypeFn(TypeFn(argument_types, return_type)) => {
                    update_queue(
                        &TypeTuple(argument_types.clone()).into(),
                        start,
                        queue,
                        visited,
                    )?;
                    update_queue(&*return_type, start, queue, visited)?;
                }
                _ => (),
            }
            Ok(())
        }
        while let Some(rc) = queue.pop_front() {
            update_queue(&rc.borrow().type_, start, &mut queue, &mut visited)?;
        }
        Ok(())
    }
    fn check_expression(
        &self,
        expression: Expression,
        context: &TypeContext,
        generic_variables: &GenericVariables,
    ) -> Result<TypedExpression, TypeCheckError> {
        Ok(match expression {
            Expression::Integer(i) => i.into(),
            Expression::Boolean(b) => b.into(),
            Expression::TupleExpression(TupleExpression { expressions }) => TypedTuple {
                expressions: self.check_expressions(expressions, context, generic_variables)?,
            }
            .into(),
            Expression::GenericVariable(GenericVariable { id, type_instances }) => {
                let variable = context.get(&id);
                match variable {
                    Some(typed_variable) => {
                        let type_ = &typed_variable.type_;
                        if type_instances.len() != type_.parameters.len() {
                            return Err(TypeCheckError::WrongNumberOfTypeParameters {
                                type_: type_.clone(),
                                type_instances,
                            });
                        }
                        let types = type_instances
                            .into_iter()
                            .map(|type_instance| {
                                TypeChecker::convert_ast_type(
                                    type_instance,
                                    &self.type_definitions,
                                    &generic_variables,
                                )
                            })
                            .collect::<Result<_, _>>()?;
                        TypedAccess {
                            variable: typed_variable.clone(),
                            parameters: types,
                        }
                        .into()
                    }
                    None => {
                        return Err(TypeCheckError::UnknownError {
                            place: String::from("variable"),
                            id,
                            options: context.keys().map(|id| id.clone()).collect_vec(),
                        })
                    }
                }
            }
            Expression::ElementAccess(ElementAccess { expression, index }) => {
                let typed_expression =
                    self.check_expression(*expression, context, generic_variables)?;
                let Type::TypeTuple(TypeTuple(types)) = typed_expression.type_() else {
                    return Err(TypeCheckError::InvalidAccess {
                        expression: typed_expression,
                        index,
                    });
                };
                if index as usize >= types.len() {
                    return Err(TypeCheckError::InvalidAccess {
                        index,
                        expression: typed_expression,
                    });
                };
                TypedElementAccess {
                    expression: Box::new(typed_expression),
                    index,
                }
                .into()
            }
            Expression::IfExpression(IfExpression {
                condition,
                true_block,
                false_block,
            }) => {
                let condition = self.check_expression(*condition, context, generic_variables)?;
                if condition.type_() != TYPE_BOOL {
                    return Err(TypeCheckError::InvalidCondition { condition });
                }
                let typed_true_block =
                    self.check_block(true_block, context.clone(), generic_variables.clone())?;
                let typed_false_block =
                    self.check_block(false_block, context.clone(), generic_variables.clone())?;
                if typed_true_block.type_() != typed_false_block.type_() {
                    return Err(TypeCheckError::NonMatchingIfBlocks {
                        true_block: typed_true_block,
                        false_block: typed_false_block,
                    });
                }
                TypedIf {
                    condition: Box::new(condition),
                    true_block: typed_true_block,
                    false_block: typed_false_block,
                }
                .into()
            }
            Expression::FunctionDefinition(FunctionDefinition {
                parameters,
                return_type,
                body,
            }) => {
                let parameter_ids = parameters
                    .iter()
                    .map(|typed_assignee| typed_assignee.assignee.id.clone())
                    .collect_vec();
                if let Err(UniqueError { duplicate }) =
                    utils::check_unique::<_, &String>(parameter_ids.iter())
                {
                    return Err(TypeCheckError::DuplicatedName {
                        duplicate: duplicate.clone(),
                        reason: String::from("function parameter"),
                    });
                }
                let parameter_types = parameters
                    .iter()
                    .map(|typed_assignee| {
                        TypeChecker::convert_ast_type(
                            typed_assignee.type_.clone(),
                            &self.type_definitions,
                            &generic_variables,
                        )
                    })
                    .collect::<Result<Vec<Type>, _>>()?;
                let parameters = parameter_ids
                    .into_iter()
                    .zip_eq(parameter_types.into_iter())
                    .map(|(id, type_)| {
                        (
                            id,
                            TypedVariable {
                                variable: Variable::new(),
                                type_: type_.into(),
                            },
                        )
                    })
                    .collect_vec();
                let return_type = TypeChecker::convert_ast_type(
                    return_type,
                    &self.type_definitions,
                    generic_variables,
                )?;
                let mut new_context = context.clone();
                for (id, variable) in &parameters {
                    new_context.insert(id.clone(), variable.clone().into());
                }
                let body = self.check_block(body, new_context, generic_variables.clone())?;
                if return_type != body.type_() {
                    return Err(TypeCheckError::FunctionReturnTypeMismatch {
                        return_type: return_type.clone(),
                        body,
                    });
                }
                TypedLambdaDef {
                    parameters: parameters
                        .into_iter()
                        .map(|(_, variable)| variable)
                        .collect_vec(),
                    return_type: Box::new(return_type),
                    body,
                }
                .into()
            }
            Expression::FunctionCall(FunctionCall {
                function,
                arguments,
            }) => {
                let function = self.check_expression(*function, context, generic_variables)?;
                let arguments = self.check_expressions(arguments, context, generic_variables)?;
                let types = arguments.iter().map(TypedExpression::type_).collect_vec();
                let Type::TypeFn(TypeFn(argument_types, _)) = function.type_() else {
                    return Err(TypeCheckError::InvalidFunctionCall {
                        expression: function,
                        arguments,
                    });
                };
                if argument_types != types {
                    return Err(TypeCheckError::InvalidFunctionCall {
                        expression: function,
                        arguments,
                    });
                }
                TypedFunctionCall {
                    function: Box::new(function),
                    arguments,
                }
                .into()
            }
            Expression::ConstructorCall(ConstructorCall {
                constructor,
                arguments,
            }) => {
                let Some(constructor_type) = self.constructors.get(&constructor.id) else {
                    return Err(TypeCheckError::UnknownError {
                        id: constructor.id.clone(),
                        options: self.constructors.keys().map(|id| id.clone()).collect_vec(),
                        place: String::from("type constructor"),
                    });
                };
                if constructor.type_instances.len()
                    != constructor_type.type_.borrow().parameters.len()
                {
                    return Err(TypeCheckError::WrongNumberOfTypeParameters {
                        type_: constructor_type.type_.borrow().clone(),
                        type_instances: constructor.type_instances,
                    });
                }
                let arguments_tuple = self.check_expression(
                    TupleExpression {
                        expressions: arguments,
                    }
                    .into(),
                    context,
                    generic_variables,
                )?;
                let Type::TypeTuple(TypeTuple(types)) = arguments_tuple.type_() else {
                    panic!("Tuple expression has non-tuple type")
                };
                let TypedExpression::TypedTuple(TypedTuple {
                    expressions: arguments,
                }) = arguments_tuple
                else {
                    panic!("Tuple expression became non-tuple type.")
                };
                let Type::TypeTuple(TypeTuple(type_variables)) = TypeChecker::convert_ast_type(
                    TypeInstance::TupleType(TupleType {
                        types: constructor.type_instances,
                    }),
                    &self.type_definitions,
                    &generic_variables,
                )?
                else {
                    panic!("Tuple type converted to non-tuple type.");
                };
                let output_type = constructor_type.type_.borrow().instantiate(&type_variables);
                let Type::TypeUnion(TypeUnion {
                    id: _,
                    variants: variant_types,
                }) = &output_type
                else {
                    panic!("Constructor call for non-union type.")
                };
                let input_type = variant_types[constructor_type.index].clone();
                match input_type {
                    Some(type_) => {
                        if vec![type_.clone()] != types {
                            return Err(TypeCheckError::InvalidConstructorArguments {
                                id: constructor.id.clone(),
                                input_type: Some(type_),
                                arguments,
                            });
                        }
                    }
                    None => {
                        if !types.is_empty() {
                            return Err(TypeCheckError::InvalidConstructorArguments {
                                id: constructor.id.clone(),
                                input_type,
                                arguments,
                            });
                        }
                    }
                }
                TypedConstructorCall {
                    idx: constructor_type.index,
                    arguments,
                    output_type,
                }
                .into()
            }
            Expression::MatchExpression(MatchExpression { subject, blocks }) => {
                let subject = self.check_expression(*subject, context, generic_variables)?;
                let Type::TypeUnion(TypeUnion { id, variants }) = subject.type_() else {
                    return Err(TypeCheckError::NonUnionTypeMatchSubject(subject));
                };
                let variant_names = blocks
                    .iter()
                    .map(|block| {
                        block
                            .matches
                            .iter()
                            .map(|item| item.type_name.clone())
                            .collect_vec()
                    })
                    .concat();
                if let Err(UniqueError { duplicate }) = utils::check_unique(variant_names.iter()) {
                    return Err(TypeCheckError::DuplicatedName {
                        duplicate: duplicate.clone(),
                        reason: String::from("match block variant name"),
                    });
                }
                let Some(variant_lookup) = variant_names
                    .iter()
                    .map(|variant_name| {
                        self.constructors
                            .get(variant_name)
                            .map(|variant| (variant_name.clone(), variant))
                    })
                    .collect::<Option<HashMap<_, _>>>()
                else {
                    return Err(TypeCheckError::IncorrectVariants { blocks });
                };
                if variant_lookup.values().any(|variant| {
                    let Type::TypeUnion(TypeUnion { id: ref name, .. }) =
                        variant.type_.borrow().type_
                    else {
                        panic!("Constructor body has non union type.")
                    };
                    *name != id
                }) {
                    return Err(TypeCheckError::IncorrectVariants { blocks });
                }
                let constructor_indices = variant_lookup
                    .values()
                    .map(|constructor| constructor.index)
                    .sorted()
                    .collect_vec();
                if constructor_indices != (0..variants.len() as usize).collect_vec() {
                    return Err(TypeCheckError::IncorrectVariants { blocks });
                }
                let blocks = blocks
                    .into_iter()
                    .map(|block| {
                        let assignments = block
                            .matches
                            .iter()
                            .map(|item| {
                                match (
                                    &item.assignee,
                                    &variants[variant_lookup[&item.type_name].index],
                                ) {
                                    (Some(assignee), Some(type_)) => {
                                        Ok(Some((assignee.id.clone(), type_)))
                                    }
                                    (None, None) => Ok(None),
                                    (assignee, _) => Err(TypeCheckError::MismatchedVariant {
                                        type_: subject.type_(),
                                        variant_id: item.type_name.clone(),
                                        assignee: assignee.clone(),
                                    }),
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let assignee = if assignments.iter().all_equal() {
                            assignments.first().unwrap().clone()
                        } else {
                            None
                        };
                        let mut context = context.clone();
                        let variable = assignee.map(|(id, type_)| {
                            let typed_variable = TypedVariable::from(type_.clone());
                            context.insert(id, typed_variable.clone());
                            typed_variable
                        });
                        let match_items = block
                            .matches
                            .into_iter()
                            .map(|item| TypedMatchItem {
                                type_idx: variant_lookup[&item.type_name].index,
                                assignee: variable.clone(),
                            })
                            .collect_vec();
                        let block =
                            self.check_block(block.block, context, generic_variables.clone())?;
                        Ok(TypedMatchBlock {
                            block,
                            matches: match_items,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if let Err(block_types) = blocks
                    .iter()
                    .map(|block| block.block.type_())
                    .all_equal_value()
                {
                    match block_types {
                        None => panic!("Match statement with no blocks."),
                        Some(_) => {
                            let mut blocks = blocks;
                            let head = blocks.split_off(1);
                            let head = head.first().unwrap();
                            let head_type = head.block.type_();
                            for block in blocks {
                                if block.block.type_() != head_type {
                                    return Err(TypeCheckError::DifferingMatchBlockTypes(
                                        head.clone(),
                                        block,
                                    ));
                                }
                            }
                            panic!("Match blocks have different types but they all match the first type.")
                        }
                    }
                }
                TypedMatch {
                    subject: Box::new(subject),
                    blocks,
                }
                .into()
            }
        })
    }
    fn check_expressions(
        &self,
        expressions: Vec<Expression>,
        context: &TypeContext,
        generic_variables: &GenericVariables,
    ) -> Result<Vec<TypedExpression>, TypeCheckError> {
        expressions
            .into_iter()
            .map(|expression| self.check_expression(expression, context, generic_variables))
            .collect::<Result<_, _>>()
    }
    fn fn_signature(
        &self,
        fn_def: &FunctionDefinition,
        generic_variables: &GenericVariables,
    ) -> Result<TypeFn, TypeCheckError> {
        let FunctionDefinition {
            parameters,
            return_type,
            body: _,
        } = fn_def;
        let parameter_types = parameters
            .iter()
            .map(|typed_assignee| {
                TypeChecker::convert_ast_type(
                    typed_assignee.type_.clone(),
                    &self.type_definitions,
                    generic_variables,
                )
            })
            .collect::<Result<Vec<Type>, _>>()?;
        let return_type = TypeChecker::convert_ast_type(
            return_type.clone(),
            &self.type_definitions,
            generic_variables,
        )?;
        Ok(TypeFn(parameter_types, Box::new(return_type)))
    }
    fn check_block(
        &self,
        block: Block,
        context: TypeContext,
        generic_variables: GenericVariables,
    ) -> Result<TypedBlock, TypeCheckError> {
        let assignments = block
            .assignments
            .into_iter()
            .map(|assignment| match *assignment.expression {
                Expression::FunctionDefinition(fn_def) => (assignment.assignee, Right(fn_def)),
                expression => (assignment.assignee, Left(expression)),
            })
            .collect_vec();
        let (var_names, fn_names): (Vec<_>, Vec<_>) =
            assignments
                .iter()
                .partition_map(|(assignee, expression)| match expression {
                    Left(_) => Left(assignee.id()),
                    Right(_) => Right(assignee.id()),
                });
        match utils::check_unique(fn_names.iter()) {
            Ok(()) => {}
            Err(UniqueError { duplicate }) => {
                return Err(TypeCheckError::DuplicatedName {
                    duplicate: duplicate.clone(),
                    reason: String::from("function names"),
                })
            }
        }
        match utils::check_unique(fn_names.iter().chain(var_names.iter().unique())) {
            Ok(()) => {}
            Err(UniqueError { duplicate }) => {
                return Err(TypeCheckError::DuplicatedName {
                    duplicate: duplicate.clone(),
                    reason: String::from("assignment and function names"),
                })
            }
        }

        let mut new_context = context.clone();
        let mut fn_context = context;
        for assignment in &assignments {
            if let (assignee, Right(fn_def)) = assignment {
                let mut generic_variables = generic_variables.clone();
                generic_variables
                    .extend(GenericVariables::from(&assignee.generic_variables).into_iter());
                let type_ = self.fn_signature(&fn_def, &generic_variables)?;
                fn_context.insert(
                    assignee.id(),
                    TypedVariable {
                        variable: Variable::new(),
                        type_: ParametricType {
                            parameters: assignee
                                .generic_variables
                                .iter()
                                .map(|id| generic_variables[id].clone())
                                .collect(),
                            type_: type_.into(),
                        },
                    },
                );
            }
        }

        let mut statements = Vec::new();
        for (assignee, expression) in assignments {
            let mut generic_variables = generic_variables.clone();
            generic_variables
                .extend(GenericVariables::from(&assignee.generic_variables).into_iter());
            let id = assignee.id();
            let parameters = assignee
                .generic_variables
                .into_iter()
                .map(|id| (id.clone(), generic_variables[&id].clone()))
                .collect_vec();
            let statement = match expression {
                Left(expression) => {
                    let typed_expression =
                        self.check_expression(expression, &new_context, &generic_variables)?;
                    let assignment = TypedAssignment {
                        variable: TypedVariable {
                            variable: Variable::new(),
                            type_: ParametricType {
                                parameters: parameters
                                    .iter()
                                    .map(|(_, parameter)| parameter.clone())
                                    .collect(),
                                type_: typed_expression.type_(),
                            },
                        },
                        expression: ParametricExpression {
                            expression: typed_expression,
                            parameters,
                        },
                    };
                    new_context.insert(id.clone(), assignment.variable.clone());
                    fn_context.insert(id, assignment.variable.clone());
                    assignment.into()
                }
                Right(fn_def) => {
                    let TypedExpression::TypedLambdaDef(lambda_def) =
                        self.check_expression(fn_def.into(), &fn_context, &generic_variables)?
                    else {
                        panic!("Function def changed form");
                    };
                    let variable = &fn_context[&id];
                    new_context.insert(id, variable.clone());
                    TypedFnDef {
                        variable: variable.clone(),
                        parameters,
                        fn_: lambda_def,
                    }
                    .into()
                }
            };
            statements.push(statement);
        }

        let typed_expression =
            self.check_expression(*block.expression, &new_context, &generic_variables)?;
        let block = TypedBlock {
            statements,
            expression: Box::new(typed_expression),
        };
        Ok(block)
    }
    fn check_program(
        program: Program,
        context: &TypeContext,
    ) -> Result<TypedProgram, TypeCheckError> {
        let definitions = program.definitions;
        let (assignments, type_definitions): (Vec<_>, Vec<_>) = definitions
            .into_iter()
            .partition(|definition| matches!(definition, Definition::Assignment(_)));

        let assignments = assignments
            .into_iter()
            .map(|definition| {
                let Definition::Assignment(assignment) = definition else {
                    panic!("Program filtered only assignments.")
                };
                assignment.clone()
            })
            .collect_vec();
        let type_checker = TypeChecker::check_type_definitions(type_definitions)?;
        let program_block = Block {
            assignments,
            expression: Box::new(
                FunctionCall {
                    function: Box::new(
                        GenericVariable {
                            id: Id::from("main"),
                            type_instances: Vec::new(),
                        }
                        .into(),
                    ),
                    arguments: Vec::new(),
                }
                .into(),
            ),
        };
        let typed_block =
            type_checker.check_block(program_block, context.clone(), GenericVariables::new())?;
        if let Type::TypeFn(_) = typed_block.type_() {
            return Err(TypeCheckError::MainFunctionReturnsFunction {
                type_: typed_block.type_(),
            });
        }
        let TypedExpression::TypedFunctionCall(TypedFunctionCall {
            function,
            arguments,
        }) = *typed_block.expression
        else {
            panic!("Main function call changed form.")
        };
        if arguments.len() != 0 {
            panic!("Main function call changed form.")
        }
        let TypedExpression::TypedAccess(TypedAccess {
            variable,
            parameters: _,
        }) = *function
        else {
            panic!("Main function call changed form.")
        };
        Ok(TypedProgram {
            type_definitions: type_checker.type_definitions,
            main: variable,
            statements: typed_block.statements,
        })
    }
    pub fn type_check(program: Program) -> Result<TypedProgram, TypeCheckError> {
        DEFAULT_CONTEXT.with(|context| Self::check_program(program, context))
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        type_check_nodes::{ConstructorType, TYPE_UNIT},
        Assignee, Assignment, Block, Boolean, Constructor, ConstructorCall, ElementAccess,
        ExpressionBlock, FunctionCall, FunctionDefinition, GenericConstructor, GenericTypeVariable,
        IfExpression, Integer, MatchBlock, MatchExpression, MatchItem, ParametricAssignee,
        TypeItem, TypeVariable, TypedAssignee, Typename, Var, VariableAssignee, ATOMIC_TYPE_BOOL,
        ATOMIC_TYPE_INT,
    };

    use super::*;

    use test_case::test_case;

    #[test_case(
        Vec::new(),
        Some(TypeDefinitions::new());
        "empty definitions"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("i"),
                    generic_variables: Vec::new()
                },
                type_: ATOMIC_TYPE_INT.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("i"),
                Type::from(TypeUnion{id: Id::from("i"),variants: vec![
                Some(TYPE_INT)
            ]}))
        ]));
        "atomic opaque type definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("i"),
                    generic_variables: Vec::new()
                },
                type_: ATOMIC_TYPE_INT.into()
            }.into(),
            OpaqueTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("i"),
                    generic_variables: Vec::new()
                },
                type_: ATOMIC_TYPE_BOOL.into()
            }.into()
        ],
        None;
        "duplicate opaque type definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("i"),
                    generic_variables: Vec::new()
                },
                type_: ATOMIC_TYPE_INT.into()
            }.into(),
            OpaqueTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("i"),
                    generic_variables: Vec::new()
                },
                type_: ATOMIC_TYPE_INT.into()
            }.into()
        ],
        None;
        "duplicate opaque type name"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("int_or_bool"),
                    generic_variables: Vec::new()
                },
                items: vec![
                    TypeItem {
                        id: Id::from("Int"),
                        type_: Some(ATOMIC_TYPE_INT.into())
                    },
                    TypeItem {
                        id: Id::from("Bool"),
                        type_: Some(ATOMIC_TYPE_BOOL.into())
                    },
                ]
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("int_or_bool"),
                Type::from(TypeUnion{
                    id: Id::from("int_or_bool"),
                    variants: vec![
                        Some(TYPE_INT.into()),
                        Some(TYPE_BOOL.into())
                    ]
                })
            )
        ]));
        "basic union type definition"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("int_list"),
                    generic_variables: Vec::new()
                },
                items: vec![
                    TypeItem{
                        id: Id::from("Cons"),
                        type_: Some(Typename("int_list").into())
                    },
                    TypeItem{
                        id: Id::from("Nil"),
                        type_: None
                    },
                ]
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("int_list"),
                ({
                    let reference = Rc::new(RefCell::new(ParametricType::new()));
                    let union_type = Type::from(TypeUnion{id: Id::from("int_list"),variants: vec![
                        Some(Type::from(TypeInstantiation{reference: Rc::clone(&reference), instances: Vec::new()})),
                        None,
                    ]});
                    *reference.borrow_mut() = union_type.into();
                    reference
                })
            )
        ]));
        "recursive type definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("Int"),
                    generic_variables: Vec::new()
                },
                type_: ATOMIC_TYPE_INT.into()
            }.into(),
            OpaqueTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("Bool"),
                    generic_variables: Vec::new()
                },
                type_: ATOMIC_TYPE_BOOL.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("Int"),
                Type::from(TypeUnion{id: Id::from("Int"),variants: vec![
                    Some(TYPE_INT)
                ]})
            ),
            (
                Id::from("Bool"),
                Type::from(TypeUnion{id: Id::from("Bool"),variants: vec![
                    Some(TYPE_BOOL)
                ]})
            ),
        ]));
        "two type definitions"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("int"),
                    generic_variables: Vec::new()
                },
                type_: ATOMIC_TYPE_INT.into()
            }.into(),
        ],
        None;
        "additional int definition"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("bool"),
                    generic_variables: Vec::new()
                },
                items: vec![
                    TypeItem { id: Id::from("two"), type_: None},
                    TypeItem { id: Id::from("four"), type_: None},
                ]
            }.into()
        ],
        None;
        "additional bool definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("ii"),
                    generic_variables: Vec::new()
                },
                type_: TupleType{
                    types: vec![ATOMIC_TYPE_INT.into(),ATOMIC_TYPE_INT.into()]
                }.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("ii"),
                Type::from(TypeUnion{id: Id::from("ii"),variants: vec![
                    Some(Type::from(TypeTuple(vec![TYPE_INT, TYPE_INT])))
                ]})
            ),
        ]));
        "tuple type definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("i2b"),
                    generic_variables: Vec::new()
                },
                type_: FunctionType{
                    argument_types: vec![ATOMIC_TYPE_INT.into()],
                    return_type: Box::new(ATOMIC_TYPE_BOOL.into()),
                }.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("i2b"),
                Type::from(TypeUnion{id: Id::from("i2b"),variants: vec![
                    Some(Type::from(TypeFn(vec![TYPE_INT], Box::new(TYPE_BOOL))))
                ]})
            ),
        ]));
        "function type definition"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("u2u"),
                    generic_variables: Vec::new()
                },
                type_: FunctionType{
                    argument_types: Vec::new(),
                    return_type: Box::new(TupleType{types: Vec::new()}.into()),
                }.into()
            }.into()
        ],
        Some(TypeDefinitions::from([
            (
                Id::from("u2u"),
                Type::from(TypeFn(Vec::new(), Box::new(Type::from(TypeTuple(Vec::new())))))
            ),
        ]));
        "transparent function type definition"
    )]
    #[test_case(
        vec![
            EmptyTypeDefinition{id: Id::from("None")}.into()
        ],
        Some(
            TypeDefinitions::from([
                (
                    Id::from("None"),
                    Type::from(TypeUnion{id: Id::from("None"),variants: vec![
                        None
                    ]})
                )
            ])
        );
        "empty type definition"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("iint"),
                    generic_variables: Vec::new()
                },
                type_: ATOMIC_TYPE_INT.into()
            }.into(),
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("iiint"),
                    generic_variables: Vec::new()
                },
                type_: Typename("iint").into(),
            }.into(),
        ],
        Some(
            TypeDefinitions::from({
                let iint = Rc::new(RefCell::new(
                    Type::from(TypeUnion{id: Id::from("iint"),variants: vec![Some(TYPE_INT)]}).into()
                ));
                let iiint = Rc::new(RefCell::new(
                    Type::from(TypeUnion{id: Id::from("iiint"),variants: vec![
                        Some(Type::from(TypeInstantiation{reference: iint.clone(), instances: Vec::new()}))
                    ]}).into()
                ));
                [(Id::from("iint"), iint), (Id::from("iiint"), iiint)]
            })
        );
        "indirect type reference"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("left"),
                    generic_variables: Vec::new()
                },
                items: vec![
                    TypeItem{
                        id: Id::from("Right"),
                        type_: Some(
                            TupleType{
                                types: vec![
                                    Typename("right").into(),
                                    ATOMIC_TYPE_BOOL.into()
                                ]
                            }.into()
                        )
                    },
                    TypeItem{
                        id: Id::from("Incorrect"),
                        type_: None
                    }
                ]
            }.into(),
            UnionTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("right"),
                    generic_variables: Vec::new()
                },
                items: vec![
                    TypeItem{
                        id: Id::from("Left"),
                        type_: Some(Typename("left").into())
                    },
                    TypeItem{
                        id: Id::from("Correct"),
                        type_: None
                    }
                ]
            }.into(),
        ],
        Some(
            TypeDefinitions::from({
                let left = Rc::new(RefCell::new(ParametricType::new()));
                let right = Rc::new(RefCell::new(
                    Type::from(TypeUnion{id: Id::from("right"),variants: vec![
                        Some(
                            Type::from(TypeInstantiation{reference: left.clone(), instances: Vec::new()})
                        ),
                        None
                    ]}).into()
                ));
                *left.borrow_mut() = Type::from(TypeUnion{id: Id::from("left"),variants: vec![
                    Some(
                        Type::from(TypeTuple(vec![
                            Type::from(TypeInstantiation{reference: right.clone(), instances: Vec::new()}),
                            TYPE_BOOL
                        ]))
                    ),
                    None
                ]}).into();
                [(Id::from("left"), left), (Id::from("right"), right)]
            })
        );
        "mutually recursive types"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Left_Right"),
                    generic_variables: Vec::new()
                },
                items: vec![
                    TypeItem{
                        id: Id::from("left"),
                        type_: Some(ATOMIC_TYPE_BOOL.into())
                    },
                    TypeItem{
                        id: Id::from("left"),
                        type_: Some(ATOMIC_TYPE_BOOL.into())
                    }
                ]
            }.into(),
        ],
        None;
        "duplicate types in union type"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Left_Right"),
                    generic_variables: Vec::new()
                },
                items: vec![
                    TypeItem{
                        id: Id::from("left"),
                        type_: Some(ATOMIC_TYPE_BOOL.into())
                    },
                    TypeItem{
                        id: Id::from("left"),
                        type_: None
                    }
                ]
            }.into(),
        ],
        None;
        "duplicate names in union type"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("wrapper"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into()
        ],
        Some(
            TypeDefinitions::from(
                [(
                    Id::from("wrapper"),
                    {
                        let parameter = Rc::new(RefCell::new(None));
                        ParametricType{
                            type_: Type::from(TypeUnion{id: Id::from("wrapper"),variants: vec![
                                Some(Type::from(TypeVariable(parameter.clone())))
                            ]}),
                            parameters: vec![parameter]
                        }
                    }
                )]
            )
        );
        "opaque generic type test"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("transparent"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into()
        ],
        Some(
            TypeDefinitions::from(
                [(
                    Id::from("transparent"),
                    {
                        let parameter = Rc::new(RefCell::new(None));
                        ParametricType{
                            type_: Type::from(TypeVariable(parameter.clone())),
                            parameters: vec![parameter]
                        }
                    }
                )]
            )
        );
        "transparent generic type test"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Either"),
                    generic_variables: vec![String::from("T"), String::from("U")]
                },
                items: vec![
                    TypeItem {
                        id: String::from("Left"),
                        type_: Some(
                            Typename("T").into()
                        )
                    },
                    TypeItem {
                        id: String::from("Right"),
                        type_: Some(
                            Typename("U").into()
                        )
                    }
                ]
            }.into()
        ],
        Some(
            TypeDefinitions::from(
                [(
                    Id::from("Either"),
                    {
                        let left_parameter = Rc::new(RefCell::new(None));
                        let right_parameter = Rc::new(RefCell::new(None));
                        ParametricType{
                            type_: Type::from(TypeUnion{id: Id::from("Either"),variants: vec![
                                Some(Type::from(TypeVariable(left_parameter.clone()))),
                                Some(Type::from(TypeVariable(right_parameter.clone()))),
                            ]}),
                            parameters: vec![left_parameter, right_parameter]
                        }
                    }
                )]
            )
        );
        "union generic type test"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Zero"),
                    generic_variables: Vec::new()
                },
                type_: Typename("Unknown").into(),
            }.into()
        ],
        None;
        "unknown type name"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("One"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into(),
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Zero"),
                    generic_variables: vec![String::from("U")]
                },
                type_: Typename("T").into()
            }.into()
        ],
        None;
        "unknown type parameter"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("One"),
                    generic_variables: vec![String::from("T"), String::from("U"), String::from("T")]
                },
                type_: Typename("T").into()
            }.into(),
        ],
        None;
        "duplicate type parameter"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("One"),
                    generic_variables: vec![String::from("int")]
                },
                type_: Typename("T").into()
            }.into(),
        ],
        None;
        "invalid type parameter"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("One"),
                    generic_variables: vec![String::from("One")]
                },
                type_: Typename("One").into()
            }.into(),
        ],
        None;
        "type parameter same as name"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("U"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into(),
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("V"),
                    generic_variables: vec![String::from("U")]
                },
                type_: Typename("U").into()
            }.into()
        ],
        Some(
            TypeDefinitions::from(
                [
                    (
                        Id::from("U"),
                        {
                            let parameter = Rc::new(RefCell::new(None));
                            ParametricType{
                                type_: Type::from(TypeUnion{id: Id::from("U"),variants: vec![
                                    Some(Type::from(TypeVariable(parameter.clone())))
                                ]}),
                                parameters: vec![parameter]
                            }
                        }
                    ),
                    (
                        Id::from("V"),
                        {
                            let parameter = Rc::new(RefCell::new(None));
                            ParametricType{
                                type_: Type::from(TypeUnion{id: Id::from("V"),variants: vec![
                                    Some(Type::from(TypeVariable(parameter.clone())))
                                ]}),
                                parameters: vec![parameter]
                            }
                        }
                    ),
                ]
            )
        );
        "type parameter name override"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("generic_int"),
                    generic_variables: Vec::new()
                },
                type_: GenericType{
                    id: Id::from("wrapper"),
                    type_variables: vec![ATOMIC_TYPE_INT.into()]
                }.into()
            }.into(),
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("wrapper"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into()
        ],
        Some(
            TypeDefinitions::from({
                let parameter = Rc::new(RefCell::new(None));
                let wrapper = Rc::new(RefCell::new(ParametricType{
                    type_: Type::from(TypeUnion{id: Id::from("wrapper"),variants: vec![
                        Some(Type::from(TypeVariable(parameter.clone())))
                    ]}),
                    parameters: vec![parameter]
                }));
                let generic_int = Rc::new(RefCell::new(ParametricType{
                    type_: Type::from(TypeInstantiation{reference: wrapper.clone(), instances: vec![TYPE_INT]}),
                    parameters: Vec::new()
                }));
                [(Id::from("wrapper"), wrapper), (Id::from("generic_int"), generic_int)]
            })
        );
        "generic type instantiation"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("generic_int"),
                    generic_variables: Vec::new()
                },
                type_: GenericType{
                    id: Id::from("wrapper"),
                    type_variables: vec![]
                }.into()
            }.into(),
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("wrapper"),
                    generic_variables: vec![String::from("T")]
                },
                type_: Typename("T").into()
            }.into()
        ],
        None;
        "generic type instantiation wrong arguments"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("apply"),
                    generic_variables: vec![Id::from("T"), Id::from("U")]
                },
                type_: GenericType{
                    id: Id::from("T"),
                    type_variables: vec![Typename("U").into()]
                }.into()
            }.into(),
        ],
        None;
        "generic type parameter instantiation"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Pair"),
                    generic_variables: vec![Id::from("T"), Id::from("U")]
                },
                type_: TupleType{
                    types: vec![Typename("T").into(), Typename("U").into()]
                }.into()
            }.into(),
        ],
        Some(
            TypeDefinitions::from([(
                Id::from("Pair"),
                {
                    let left_parameter = Rc::new(RefCell::new(None));
                    let right_parameter = Rc::new(RefCell::new(None));
                    ParametricType{
                        parameters: vec![left_parameter.clone(), right_parameter.clone()],
                        type_: Type::from(TypeTuple(
                            vec![Type::from(TypeVariable(left_parameter)), Type::from(TypeVariable(right_parameter))]
                        ))
                    }
                }
            )])
        );
        "pair type"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Function"),
                    generic_variables: vec![Id::from("T"), Id::from("U")]
                },
                type_: FunctionType{
                    argument_types: vec![Typename("T").into()],
                    return_type: Box::new(Typename("U").into())
                }.into()
            }.into(),
        ],
        Some(
            TypeDefinitions::from([(
                Id::from("Function"),
                {
                    let argument_parameter = Rc::new(RefCell::new(None));
                    let return_parameter = Rc::new(RefCell::new(None));
                    ParametricType{
                        parameters: vec![argument_parameter.clone(), return_parameter.clone()],
                        type_: Type::from(TypeFn(
                            vec![Type::from(TypeVariable(argument_parameter))],
                            Box::new(Type::from(TypeVariable(return_parameter)))
                        ))
                    }
                }
            )])
        );
        "function type"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Tree"),
                    generic_variables: vec![Id::from("T")]
                },
                items: vec![
                    TypeItem {
                        id: Id::from("Node"),
                        type_: Some(TupleType {
                            types: vec![
                                Typename("T").into(),
                                GenericType{
                                    id: Id::from("Tree"),
                                    type_variables: vec![Typename("T").into()]
                                }.into(),
                                Typename("T").into()
                            ]
                        }.into())
                    },
                    TypeItem {
                        id: Id::from("Leaf"),
                        type_: None
                    }
                ]
            }.into(),
        ],
        Some(
            TypeDefinitions::from([(
                Id::from("Tree"),
                {
                    let parameter = Rc::new(RefCell::new(None));
                    let tree_type = Rc::new(RefCell::new(ParametricType{parameters: vec![parameter.clone()], type_: Type::new()}));
                    tree_type.borrow_mut().type_ = Type::from(TypeUnion{
                        id: Id::from("Tree"),
                        variants: vec![
                            Some(Type::from(TypeTuple(vec![
                                Type::from(TypeVariable(parameter.clone())),
                                Type::from(TypeInstantiation{
                                    reference: tree_type.clone(),
                                    instances: vec![Type::from(TypeVariable(parameter.clone()))]
                                }),
                                Type::from(TypeVariable(parameter.clone())),
                            ]))),
                            None
                        ]
                    });
                    tree_type
                }
            )])
        );
        "tree type"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("recursive"),
                    generic_variables: Vec::new()
                },
                type_: Typename("recursive").into(),
            }.into(),
        ],
        None;
        "recursive typealias"
    )]
    #[test_case(
        vec![
            OpaqueTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("Recursive"),
                    generic_variables: vec![Id::from("T")]
                },
                type_: GenericType{
                    id: Id::from("Recursive"),
                    type_variables: vec![Typename("T").into()]
                }.into(),
            }.into(),
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("recursive_alias"),
                    generic_variables: Vec::new()
                },
                type_: GenericType{
                    id: Id::from("Recursive"),
                    type_variables: vec![Typename("recursive_alias").into()]
                }.into(),
            }.into(),
        ],
        None;
        "indirectly recursive typealias"
    )]
    #[test_case(
        vec![
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("recursive1"),
                    generic_variables: Vec::new()
                },
                type_: Typename("recursive2").into(),
            }.into(),
            TransparentTypeDefinition{
                variable: GenericTypeVariable{
                    id: Id::from("recursive2"),
                    generic_variables: Vec::new()
                },
                type_: Typename("recursive1").into(),
            }.into(),
        ],
        None;
        "mutually recursive typealias"
    )]
    #[test_case(
        vec![
            UnionTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("int_list"),
                    generic_variables: Vec::new()
                },
                items: vec![
                    TypeItem{
                        id: Id::from("Cons"),
                        type_: Some(Typename("int_list").into())
                    },
                    TypeItem{
                        id: Id::from("Nil"),
                        type_: None
                    },
                ]
            }.into(),
            TransparentTypeDefinition {
                variable: GenericTypeVariable{
                    id: Id::from("int_list2"),
                    generic_variables: Vec::new()
                },
                type_: Typename("int_list").into()
            }.into()
        ],
        Some(TypeDefinitions::from(
            {
                let reference = Rc::new(RefCell::new(ParametricType::new()));
                let union_type = Type::from(TypeUnion{id: Id::from("int_list"), variants: vec![
                    Some(Type::from(TypeInstantiation{reference: Rc::clone(&reference), instances: Vec::new()})),
                    None,
                ]});
                *reference.borrow_mut() = union_type.into();
                let instantiation = Rc::new(RefCell::new(Type::from(TypeInstantiation{reference: reference.clone(), instances: Vec::new()}).into()));
                [
                    (Id::from("int_list"),reference),
                    (Id::from("int_list2"), instantiation)
                ]
            }
        ));
        "recursive type alias"
    )]
    fn test_check_type_definitions(
        definitions: Vec<Definition>,
        expected_result: Option<TypeDefinitions>,
    ) {
        let type_check_result = TypeChecker::check_type_definitions(definitions);
        match &(&type_check_result, expected_result) {
            (Ok(type_checker), Some(result)) => {
                assert_eq!(type_checker.type_definitions, result.clone());
            }
            (Err(msg), Some(_)) => {
                dbg!(msg);
                assert!(type_check_result.is_ok())
            }
            (Ok(type_checker), None) => {
                dbg!(type_checker);
                assert!(type_check_result.is_err())
            }
            (Err(_), None) => (),
        }
    }

    const ALPHA_TYPE: Lazy<Rc<RefCell<ParametricType>>> = Lazy::new(|| {
        let parameter = Rc::new(RefCell::new(None));
        Rc::new(RefCell::new(ParametricType {
            type_: Type::from(TypeVariable(parameter.clone())),
            parameters: vec![parameter],
        }))
    });
    thread_local! {pub static TYPE_DEFINITIONS: Lazy<TypeDefinitions> = Lazy::new(|| {
        TypeDefinitions::from([
            (
                Id::from("opaque_int"),
                Rc::new(RefCell::new(
                    Type::from(TypeUnion{id: Id::from("opaque_int"), variants: vec![Some(TYPE_INT)]}).into(),
                )),
            ),
            (
                Id::from("opaque_int_2"),
                Rc::new(RefCell::new(
                    Type::from(TypeUnion{id: Id::from("opaque_int_2"), variants: vec![Some(TYPE_INT)]}).into(),
                )),
            ),
            (
                Id::from("transparent_int"),
                Rc::new(RefCell::new(TYPE_INT.into())),
            ),
            (
                Id::from("transparent_int_2"),
                Rc::new(RefCell::new(TYPE_INT.into())),
            ),
            (
                Id::from("ii"),
                Rc::new(RefCell::new(Type::from(TypeTuple(vec![TYPE_INT, TYPE_INT])).into())),
            ),
            (Id::from("recursive"), {
                let reference = Rc::new(RefCell::new(ParametricType::new()));
                *reference.borrow_mut() = Type::from(TypeUnion{
                    id: Id::from("recursive"),
                    variants: vec![Some(Type::from(TypeInstantiation{reference: Rc::clone(&reference), instances: Vec::new()}))],
                })
                .into();
                reference
            }),
            (Id::from("List"), {
                let parameter = Rc::new(RefCell::new(None));
                let list_type = Rc::new(RefCell::new(ParametricType {
                    parameters: vec![parameter.clone()],
                    type_: Type::new(),
                }));
                list_type.borrow_mut().type_ = Type::from(TypeUnion{
                    id: Id::from("List"),
                    variants: vec![
                        Some(Type::from(TypeTuple(vec![
                            Type::from(TypeVariable(parameter.clone())),
                            Type::from(TypeInstantiation{
                                reference: list_type.clone(),
                                instances: vec![Type::from(TypeVariable(parameter.clone()))],
                            }),
                        ]))),
                        None,
                    ],
                });
                list_type
            }),
            (
                Id::from("Bull"),
                Rc::new(RefCell::new(
                    Type::from(TypeUnion{id: Id::from("Bull"), variants: vec![None, None]}).into(),
                )),
            ),
            (
                Id::from("Bul"),
                Rc::new(RefCell::new(
                    Type::from(TypeUnion{id: Id::from("Bul"), variants: vec![None, None]}).into(),
                )),
            ),
            (Id::from("Option"), {
                let parameter = Rc::new(RefCell::new(None));
                Rc::new(RefCell::new(ParametricType {
                    parameters: vec![parameter.clone()],
                    type_: Type::from(TypeUnion{
                        id: Id::from("Option"),
                        variants: vec![Some(Type::from(TypeVariable(parameter))), None],
                    }),
                }))
            }),
            (Id::from("Either"), {
                let left_parameter = Rc::new(RefCell::new(None));
                let right_parameter = Rc::new(RefCell::new(None));
                Rc::new(RefCell::new(ParametricType {
                    parameters: vec![left_parameter.clone(), right_parameter.clone()],
                    type_: Type::from(TypeUnion{
                        id: Id::from("Either"),
                        variants: vec![
                            Some(Type::from(TypeVariable(left_parameter))),
                            Some(Type::from(TypeVariable(right_parameter))),
                        ],
                    }),
                }))
            }),
        ])
    })}
    const TYPE_CONSTRUCTORS: Lazy<HashMap<Id, ConstructorType>> = Lazy::new(|| {
        HashMap::from([
            (
                Id::from("opaque_int"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("opaque_int")].clone()),
                    index: 0,
                },
            ),
            (
                Id::from("opaque_int_2"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("opaque_int_2")].clone()),
                    index: 0,
                },
            ),
            (
                Id::from("recursive"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("recursive")].clone()),
                    index: 0,
                },
            ),
            (
                Id::from("Cons"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("List")].clone()),
                    index: 0,
                },
            ),
            (
                Id::from("Nil"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("List")].clone()),
                    index: 1,
                },
            ),
            (
                Id::from("twoo"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("Bull")].clone()),
                    index: 0,
                },
            ),
            (
                Id::from("faws"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("Bull")].clone()),
                    index: 1,
                },
            ),
            (
                Id::from("two"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("Bul")].clone()),
                    index: 0,
                },
            ),
            (
                Id::from("faw"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("Bul")].clone()),
                    index: 1,
                },
            ),
            (
                Id::from("Some"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("Option")].clone()),
                    index: 0,
                },
            ),
            (
                Id::from("None"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("Option")].clone()),
                    index: 1,
                },
            ),
            (
                Id::from("Left"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("Either")].clone()),
                    index: 0,
                },
            ),
            (
                Id::from("Right"),
                ConstructorType {
                    type_: TYPE_DEFINITIONS
                        .with(|definitions| definitions[&Id::from("Either")].clone()),
                    index: 1,
                },
            ),
        ])
    });
    #[test_case(
        Integer{value: -5}.into(),
        Some(TYPE_INT),
        TypeContext::new();
        "type check integer"
    )]
    #[test_case(
        Boolean{value: true}.into(),
        Some(TYPE_BOOL),
        TypeContext::new();
        "type check boolean"
    )]
    #[test_case(
        TupleExpression{
            expressions: Vec::new()
        }.into(),
        Some(Type::from(TypeTuple(Vec::new()))),
        TypeContext::new();
        "type check empty tuple"
    )]
    #[test_case(
        TupleExpression{
            expressions: vec![
                Boolean{
                    value: true,
                }.into(),
                Integer{
                    value: -2,
                }.into(),
            ]
        }.into(),
        Some(Type::from(TypeTuple(vec![
            TYPE_BOOL.into(),
            TYPE_INT.into(),
        ]))),
        TypeContext::new();
        "type check flat tuple"
    )]
    #[test_case(
        TupleExpression{
            expressions: vec![
                TupleExpression{
                    expressions: Vec::new()
                }.into(),
                TupleExpression{
                    expressions: vec![
                        Boolean{
                            value: true,
                        }.into(),
                        Integer{
                            value: -2,
                        }.into(),
                    ]
                }.into()
            ]
        }.into(),
        Some(TypeTuple(vec![
            Type::from(TypeTuple(Vec::new())),
            Type::from(TypeTuple(vec![
                TYPE_BOOL.into(),
                TYPE_INT.into(),
            ]))
        ]).into()),
        TypeContext::new();
        "type check nested tuple"
    )]
    #[test_case(
        Var("a").into(),
        Some(TYPE_INT),
        TypeContext::from([
            (
                Id::from("a"),
                Type::from(TYPE_INT).into()
            )
        ]);
        "type check variable"
    )]
    #[test_case(
        Var("b").into(),
        None,
        TypeContext::from([
            (
                Id::from("a"),
                Type::from(TYPE_INT).into()
            )
        ]);
        "type check missing variable"
    )]
    #[test_case(
        TupleExpression{
            expressions: vec![
                Var("b").into(),
                Var("a").into(),
                Var("a").into(),
            ]
        }.into(),
        Some(Type::from(TypeTuple(vec![
            TYPE_BOOL.into(),
            TYPE_INT.into(),
            TYPE_INT.into(),
        ]))),
        TypeContext::from([
            (
                Id::from("a"),
                Type::from(TYPE_INT).into()
            ),
            (
                Id::from("b"),
                Type::from(TYPE_BOOL).into()
            )
        ]);
        "type check multiple variables"
    )]
    #[test_case(
        Var("f").into(),
        None,
        TypeContext::from([
            (
                Id::from("f"),
                {
                    let parameter = Rc::new(RefCell::new(None));
                    ParametricType{
                        type_: Type::from(TypeVariable(parameter.clone())),
                        parameters: vec![parameter]
                    }.into()
                }
            )
        ]);
        "type check wrong arguments"
    )]
    #[test_case(
        GenericVariable {
            id: Id::from("f"),
            type_instances: vec![ATOMIC_TYPE_INT.into()]
        }.into(),
        Some(Type::from(TypeInstantiation{reference: ALPHA_TYPE.clone(), instances: vec![TYPE_INT.into()]})),
        TypeContext::from([
            (
                Id::from("f"),
                ALPHA_TYPE.clone().into()
            )
        ]);
        "type check parametric type"
    )]
    #[test_case(
        ElementAccess{
            expression: Box::new(Integer{value:5}.into()),
            index: 0
        }.into(),
        None,
        TypeContext::new();
        "invalid type element access"
    )]
    #[test_case(
        ElementAccess{
            expression: Box::new(TupleExpression{
                expressions: vec![
                    Integer{value: 5}.into(),
                    Boolean{value: true}.into(),
                ]
            }.into()),
            index: 0
        }.into(),
        Some(TYPE_INT.into()),
        TypeContext::new();
        "flat element access"
    )]
    #[test_case(
        ElementAccess{
            expression: Box::new(TupleExpression{
                expressions: vec![
                    Integer{value: 5}.into(),
                    Boolean{value: true}.into(),
                ]
            }.into()),
            index: 2
        }.into(),
        None,
        TypeContext::new();
        "element access out of range"
    )]
    #[test_case(
        ElementAccess{
            expression: Box::new(ElementAccess {
                expression: Box::new(Var("a").into()),
                index: 0
            }.into()),
            index: 0
        }.into(),
        Some(TYPE_UNIT.into()),
        TypeContext::from([(
            Id::from("a"),
            Type::from(TypeTuple(
                vec![Type::from(TypeTuple(
                    vec![
                        TYPE_UNIT
                    ]
                ))]
            )).into()
        )]);
        "nested element access"
    )]
    #[test_case(
        Var("empty").into(),
        Some(Type::TypeUnion(
            TypeUnion{id: Id::from("Empty"),variants: vec![None].into()}
        )),
        TypeContext::from([(
            Id::from("empty"),
            Type::from(TypeUnion{id: Id::from("Empty"),variants: [(
                None
            )].into()}).into()
        )]);
        "variable with empty type"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Integer{value: 0}.into()),
            true_block: ExpressionBlock(Boolean{value: true}.into()),
            false_block: ExpressionBlock(Boolean{value: false}.into())
        }.into(),
        None,
        TypeContext::new();
        "if expression invalid condition"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: ExpressionBlock(Boolean{value: true}.into()),
            false_block: ExpressionBlock(Boolean{value: false}.into())
        }.into(),
        Some(TYPE_BOOL.into()),
        TypeContext::new();
        "if expression no assignments condition"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: ExpressionBlock(Integer{value: 8}.into()),
            false_block: ExpressionBlock(Boolean{value: false}.into())
        }.into(),
        None,
        TypeContext::new();
        "if expression different blocks"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: ExpressionBlock(Var("x").into()),
            false_block: ExpressionBlock(Boolean{value: false}.into())
        }.into(),
        None,
        TypeContext::new();
        "if expression invalid block"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: Block{
                assignments: vec![
                    Assignment {
                        assignee: VariableAssignee("x"),
                        expression: Box::new(Integer{value: -5}.into())
                    }
                ],
                expression: Box::new(Var("x").into())
            },
            false_block: ExpressionBlock(Integer{value: 5}.into())
        }.into(),
        Some(TYPE_INT.into()),
        TypeContext::new();
        "if expression variable in block"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: Block{
                assignments: vec![
                    Assignment {
                        assignee: VariableAssignee("x"),
                        expression: Box::new(Integer{value: -5}.into())
                    }
                ],
                expression: Box::new(Var("x").into())
            },
            false_block: ExpressionBlock(Integer{value: 5}.into())
        }.into(),
        Some(TYPE_INT.into()),
        TypeContext::from([(
            Id::from("x"),
            TYPE_BOOL.into()
        )]);
        "if expression variable shadowed in block"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: Block{
                assignments: vec![
                    Assignment {
                        assignee: VariableAssignee("x"),
                        expression: Box::new(Integer{value: -5}.into())
                    }
                ],
                expression: Box::new(Var("x").into())
            },
            false_block: ExpressionBlock(Var("x").into())
        }.into(),
        None,
        TypeContext::from([(
            Id::from("x"),
            TYPE_BOOL.into()
        )]);
        "if expression variable shadowed incorrectly block"
    )]
    #[test_case(
        IfExpression {
            condition: Box::new(Boolean{value: false}.into()),
            true_block: Block{
                assignments: vec![
                    Assignment {
                        assignee: VariableAssignee("x"),
                        expression: Box::new(Integer{value: -5}.into())
                    }
                ],
                expression: Box::new(Var("x").into())
            },
            false_block: ExpressionBlock(
                ElementAccess {
                    expression: Box::new(Var("x").into()),
                    index: 1
                }.into()
            ),
        }.into(),
        Some(TYPE_INT),
        TypeContext::from([(
            Id::from("x"),
            Type::from(TypeTuple(
                vec![
                    TYPE_BOOL,
                    TYPE_INT,
                ]
            )).into()
        )]);
        "if expression variable shadowed then accessed"
    )]
    #[test_case(
        FunctionDefinition {
            parameters: Vec::new(),
            return_type: TupleType{types: Vec::new()}.into(),
            body: ExpressionBlock(TupleExpression{expressions: Vec::new()}.into())
        }.into(),
        Some(Type::from(TypeFn(Vec::new(), Box::new(TYPE_UNIT)))),
        TypeContext::new();
        "unit function def"
    )]
    #[test_case(
        FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Id::from("y").into(),
                    type_: ATOMIC_TYPE_BOOL.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Var("x").into())
        }.into(),
        Some(Type::from(TypeFn(vec![TYPE_INT, TYPE_BOOL], Box::new(TYPE_INT)))),
        TypeContext::new();
        "arguments function def"
    )]
    #[test_case(
        FunctionCall {
            function: Box::new(Var("+").into()),
            arguments: vec![
                Integer{ value: 3}.into(),
                Integer{ value: 5}.into(),
            ],
        }.into(),
        Some(TYPE_INT),
        TypeContext::from([(
            Id::from("+"),
            Type::from(TypeFn(
                vec![TYPE_INT, TYPE_INT],
                Box::new(TYPE_INT)
            )).into()
        )]);
        "addition function call"
    )]
    #[test_case(
        FunctionCall {
            function: Box::new(Var("+").into()),
            arguments: vec![
                Boolean{ value: true}.into(),
                Integer{ value: 5}.into(),
            ],
        }.into(),
        None,
        TypeContext::from([(
            Id::from("+"),
            Type::from(TypeFn(
                vec![TYPE_INT, TYPE_INT],
                Box::new(TYPE_INT)
            )).into()
        )]);
        "addition function call wrong type"
    )]
    #[test_case(
        FunctionCall {
            function: Box::new(Var("+").into()),
            arguments: vec![
                Integer{ value: 3}.into(),
                Integer{ value: 5}.into(),
            ],
        }.into(),
        Some(TYPE_INT),
        TypeContext::from([(
            Id::from("+"),
            Type::from(TypeFn(
                vec![
                    Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("transparent_int")].clone()), instances: Vec::new()}),
                    Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("transparent_int")].clone()), instances: Vec::new()})
                ],
                Box::new(Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("transparent_int")].clone()), instances: Vec::new()})))
            ).into()
        )]);
        "addition function call with aliases"
    )]
    #[test_case(
        ConstructorCall {
            constructor: Constructor("opaque_int"),
            arguments: vec![
                Integer{ value: 5}.into(),
            ],
        }.into(),
        Some(Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("opaque_int")].clone()), instances: Vec::new()})),
        TypeContext::new();
        "constructor call"
    )]
    #[test_case(
        ConstructorCall {
            constructor: Constructor("opaque_int"),
            arguments: vec![
                Boolean{ value: true}.into(),
            ],
        }.into(),
        None,
        TypeContext::new();
        "constructor call wrong type arguments"
    )]
    #[test_case(
        ConstructorCall {
            constructor: Constructor("opaque_int"),
            arguments: Vec::new(),
        }.into(),
        None,
        TypeContext::new();
        "constructor call wrong number arguments"
    )]
    #[test_case(
        ConstructorCall {
            constructor: GenericConstructor{
                id: Id::from("opaque_int"),
                type_instances: vec![ATOMIC_TYPE_INT.into()]
            },
            arguments: vec![
                Integer{ value: 5}.into(),
            ],
        }.into(),
        None,
        TypeContext::new();
        "constructor call wrong type parameters"
    )]
    #[test_case(
        ConstructorCall {
            constructor: GenericConstructor{
                id: Id::from("Nil"),
                type_instances: vec![ATOMIC_TYPE_INT.into()]
            },
            arguments: Vec::new(),
        }.into(),
        Some(TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("List")].borrow().instantiate(&vec![TYPE_INT]))),
        TypeContext::new();
        "constructor call output generic"
    )]
    #[test_case(
        ConstructorCall {
            constructor: GenericConstructor{
                id: Id::from("Cons"),
                type_instances: vec![ATOMIC_TYPE_INT.into()]
            },
            arguments: vec![
                TupleExpression {
                    expressions: vec![
                        Integer{value: 3}.into(),
                        ConstructorCall {
                            constructor: GenericConstructor{
                                id: Id::from("Nil"),
                                type_instances: vec![ATOMIC_TYPE_INT.into()]
                            },
                            arguments: Vec::new(),
                        }.into(),
                    ]
                }.into()
            ],
        }.into(),
        Some(TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("List")].borrow().instantiate(&vec![TYPE_INT]))),
        TypeContext::new();
        "constructor call generic"
    )]
    #[test_case(
        ConstructorCall {
            constructor: GenericConstructor{
                id: Id::from("Empty"),
                type_instances: vec![ATOMIC_TYPE_INT.into()]
            },
            arguments: vec![],
        }.into(),
        None,
        TypeContext::new();
        "constructor call non-existant"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("random_bull").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("twoo"),
                            assignee: None
                        },
                        MatchItem {
                            type_name: Id::from("faws"),
                            assignee: None
                        }
                    ],
                    block: ExpressionBlock(TupleExpression{expressions: Vec::new()}.into())
                }
            ]
        }.into(),
        Some(TYPE_UNIT),
        TypeContext::from([(
            Id::from("random_bull"),
            TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("Bull")].clone()).into()
        )]);
        "basic match expression"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("random_bull").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("twoo"),
                            assignee: None
                        },
                    ],
                    block: ExpressionBlock(TupleExpression{expressions: Vec::new()}.into())
                },
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("faws"),
                            assignee: None
                        }
                    ],
                    block: ExpressionBlock(TupleExpression{expressions: Vec::new()}.into())
                }
            ]
        }.into(),
        Some(TYPE_UNIT),
        TypeContext::from([(
            Id::from("random_bull"),
            TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Bull")].clone()).into()
        )]);
        "split match expression"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("random_bull").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("two"),
                            assignee: None
                        },
                        MatchItem {
                            type_name: Id::from("faw"),
                            assignee: None
                        }
                    ],
                    block: ExpressionBlock(TupleExpression{expressions: Vec::new()}.into())
                }
            ]
        }.into(),
        None,
        TypeContext::from([(
            Id::from("random_bull"),
            TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Bull")].clone()).into()
        )]);
        "match equivalent types"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("random_bull").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("twoo"),
                            assignee: None
                        },
                    ],
                    block: ExpressionBlock(Boolean{ value: true }.into())
                },
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("faws"),
                            assignee: None
                        }
                    ],
                    block: ExpressionBlock(Integer{ value: 4 }.into())
                }
            ]
        }.into(),
        None,
        TypeContext::from([(
            Id::from("random_bull"),
            TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Bull")].clone()).into()
        )]);
        "differing match blocks"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Boolean {value: true}.into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("True"),
                            assignee: None
                        },
                    ],
                    block: ExpressionBlock(TupleExpression{expressions: Vec::new()}.into())
                },
            ]
        }.into(),
        None,
        TypeContext::new();
        "non-union type"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("random_bull").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("twoo"),
                            assignee: None
                        },
                    ],
                    block: ExpressionBlock(TupleExpression{expressions: Vec::new()}.into())
                },
            ]
        }.into(),
        None,
        TypeContext::from([(
            Id::from("random_bull"),
            TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Bull")].clone()).into()
        )]);
        "non-exhaustive matches"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("random_bull").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("faws"),
                            assignee: Some(Assignee {
                                id: Id::from("x")
                            })
                        },
                    ],
                    block: ExpressionBlock(TupleExpression{expressions: Vec::new()}.into())
                },
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("twoo"),
                            assignee: None
                        },
                    ],
                    block: ExpressionBlock(TupleExpression{expressions: Vec::new()}.into())
                },
            ]
        }.into(),
        None,
        TypeContext::from([(
            Id::from("random_bull"),
            TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Bull")].clone()).into()
        )]);
        "empty match assignee"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("x").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("Some"),
                            assignee: Some(Assignee {
                                id: Id::from("y")
                            })
                        },
                    ],
                    block: ExpressionBlock(GenericVariable{id:Id::from("y"), type_instances: Vec::new()}.into())
                },
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("None"),
                            assignee: None
                        },
                    ],
                    block: ExpressionBlock(Integer{value: 3}.into())
                },
            ]
        }.into(),
        Some(TYPE_INT),
        TypeContext::from([(
            Id::from("x"),
            Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Option")].clone()),instances: vec![TYPE_INT]}).into()
        )]);
        "valid match assignment"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("x").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("Some"),
                            assignee: None
                        },
                    ],
                    block: ExpressionBlock(Integer{value: -3}.into())
                },
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("None"),
                            assignee: None
                        },
                    ],
                    block: ExpressionBlock(Integer{value: 3}.into())
                },
            ]
        }.into(),
        None,
        TypeContext::from([(
            Id::from("x"),
            Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Option")].clone()),instances: vec![TYPE_INT]}).into()
        )]);
        "missing variant assignee"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("x").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("Some"),
                            assignee: Some(Assignee {
                                id: Id::from("y")
                            })
                        },
                    ],
                    block: ExpressionBlock(GenericVariable{id:Id::from("y"), type_instances: Vec::new()}.into())
                },
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("None"),
                            assignee: None
                        },
                    ],
                    block: ExpressionBlock(GenericVariable{id:Id::from("y"), type_instances: Vec::new()}.into())
                },
            ]
        }.into(),
        None,
        TypeContext::from([(
            Id::from("x"),
            Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Option")].clone()),instances: vec![TYPE_INT]}).into()
        )]);
        "match out-of-scope variable"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("x").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("Some"),
                            assignee: Some(Assignee {
                                id: Id::from("y")
                            })
                        },
                        MatchItem {
                            type_name: Id::from("None"),
                            assignee: None
                        },
                    ],
                    block: ExpressionBlock(GenericVariable{id:Id::from("y"), type_instances: Vec::new()}.into())
                },
            ]
        }.into(),
        None,
        TypeContext::from([(
            Id::from("x"),
            Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Option")].clone()),instances: vec![TYPE_INT]}).into()
        )]);
        "match partially-used variable"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("x").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("Left"),
                            assignee: Some(Assignee {
                                id: Id::from("y")
                            })
                        },
                        MatchItem {
                            type_name: Id::from("Right"),
                            assignee: Some(Assignee {
                                id: Id::from("y")
                            })
                        },
                    ],
                    block: ExpressionBlock(GenericVariable{id:Id::from("y"), type_instances: Vec::new()}.into())
                },
            ]
        }.into(),
        Some(TYPE_INT),
        TypeContext::from([(
            Id::from("x"),
            Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Either")].clone()),instances: vec![TYPE_INT,Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("transparent_int")].clone()),instances: Vec::new()})]}).into()
        )]);
        "match same type variable"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("x").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("Left"),
                            assignee: Some(Assignee {
                                id: Id::from("y")
                            })
                        },
                        MatchItem {
                            type_name: Id::from("Right"),
                            assignee: Some(Assignee {
                                id: Id::from("y")
                            })
                        },
                    ],
                    block: ExpressionBlock(GenericVariable{id:Id::from("y"), type_instances: Vec::new()}.into())
                },
            ]
        }.into(),
        None,
        TypeContext::from([(
            Id::from("x"),
            Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Either")].clone()),instances: vec![TYPE_INT,Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("opaque_int")].clone()),instances: Vec::new()})]}).into()
        )]);
        "match different type variables"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("x").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("Left"),
                            assignee: Some(Assignee {
                                id: Id::from("y")
                            })
                        },
                        MatchItem {
                            type_name: Id::from("Right"),
                            assignee: Some(Assignee {
                                id: Id::from("z")
                            })
                        },
                    ],
                    block: ExpressionBlock(GenericVariable{id:Id::from("y"), type_instances: Vec::new()}.into())
                },
            ]
        }.into(),
        None,
        TypeContext::from([(
            Id::from("x"),
            Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Either")].clone()),instances: vec![TYPE_INT,Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("transparent_int")].clone()),instances: Vec::new()})]}).into()
        )]);
        "different variable names"
    )]
    #[test_case(
        MatchExpression {
            subject: Box::new(Var("x").into()),
            blocks: vec![
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("Some"),
                            assignee: Some(Assignee {
                                id: Id::from("y")
                            })
                        },
                    ],
                    block: ExpressionBlock(
                        MatchExpression {
                            subject: Box::new(Var("y").into()),
                            blocks: vec![
                                MatchBlock {
                                    matches: vec![
                                        MatchItem {
                                            type_name: Id::from("Left"),
                                            assignee: Some(Assignee {
                                                id: Id::from("y")
                                            })
                                        },
                                    ],
                                    block: ExpressionBlock(Var("y").into())
                                },
                                MatchBlock {
                                    matches: vec![
                                        MatchItem {
                                            type_name: Id::from("Right"),
                                            assignee: Some(Assignee {
                                                id: Id::from("r")
                                            })
                                        },
                                    ],
                                    block: ExpressionBlock(
                                        FunctionCall {
                                            function: Box::new(Var("*").into()),
                                            arguments: vec![Var("y").into(), Var("r").into()]
                                        }.into()
                                    )
                                },
                            ]
                        }.into(),
                    )
                },
                MatchBlock {
                    matches: vec![
                        MatchItem {
                            type_name: Id::from("None"),
                            assignee: None
                        },
                    ],
                    block: ExpressionBlock(Integer{value: 0}.into())
                }
            ]
        }.into(),
        Some(TYPE_INT),
        TypeContext::from([
            (
                Id::from("x"),
                Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Option")].clone()),instances: vec![Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Either")].clone()),instances: vec![TYPE_INT,TYPE_BOOL]}).into()]}).into()
            ),
            (
                Id::from("*"),
                Type::from(TypeFn(vec![Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&String::from("Either")].clone()),instances: vec![TYPE_INT,TYPE_BOOL]}).into(),TYPE_BOOL], Box::new(TYPE_INT))).into()
            ),
        ]);
        "nested match"
    )]
    fn test_check_expressions(
        expression: Expression,
        expected_type: Option<Type>,
        context: TypeContext,
    ) {
        let type_checker = TypeChecker {
            type_definitions: TYPE_DEFINITIONS
                .with(|definitions| TypeDefinitions::from(definitions.0.clone())),
            constructors: TYPE_CONSTRUCTORS.clone(),
        };
        let type_check_result =
            type_checker.check_expression(expression, &context, &GenericVariables::new());
        match expected_type {
            Some(type_) => match &type_check_result {
                Ok(typed_expression) => {
                    assert!(Type::equality(&typed_expression.type_(), &type_))
                }
                Err(msg) => {
                    dbg!(msg);
                    assert!(&type_check_result.is_ok());
                }
            },
            None => {
                if type_check_result.is_ok() {
                    dbg!(&type_check_result);
                }
                assert!(&type_check_result.is_err());
            }
        }
    }

    #[test_case(
        ExpressionBlock(Boolean{value: true}.into()),
        Some(TYPE_BOOL),
        TypeContext::new();
        "block no assignments"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("x"),
                    expression: Box::new(Boolean{value: true}.into())
                }
            ],
            expression: Box::new(Boolean{value: true}.into())
        },
        Some(TYPE_BOOL),
        TypeContext::new();
        "block unused assignment"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("x"),
                    expression: Box::new(Boolean{value: true}.into())
                }
            ],
            expression: Box::new(Var("x").into())
        },
        Some(TYPE_BOOL),
        TypeContext::new();
        "block used assignment"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("x"),
                    expression: Box::new(Integer{value: 3}.into())
                },
                Assignment{
                    assignee: VariableAssignee("y"),
                    expression: Box::new(Var("x").into())
                },
            ],
            expression: Box::new(Var("y").into())
        },
        Some(TYPE_INT),
        TypeContext::new();
        "block multiple assignments"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("x"),
                    expression: Box::new(Integer{value: 3}.into())
                },
                Assignment{
                    assignee: VariableAssignee("x"),
                    expression: Box::new(Boolean{value: true}.into())
                },
            ],
            expression: Box::new(Var("x").into())
        },
        Some(TYPE_BOOL),
        TypeContext::new();
        "block duplicate assignments"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("f"),
                    expression: Box::new(FunctionDefinition {
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(Integer{value: 3}.into())
                    }.into())
                },
                Assignment{
                    assignee: VariableAssignee("f"),
                    expression: Box::new(FunctionDefinition {
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(Integer{value: -3}.into())
                    }.into())
                },
            ],
            expression: Box::new(Var("f").into())
        },
        None,
        TypeContext::new();
        "block duplicate fn assignments"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("f"),
                    expression: Box::new(FunctionDefinition {
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(Integer{value: 3}.into())
                    }.into())
                },
                Assignment{
                    assignee: VariableAssignee("f"),
                    expression: Box::new(Integer{value: -3}.into())
                },
            ],
            expression: Box::new(Var("f").into())
        },
        None,
        TypeContext::new();
        "block variable and fn assignments"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("x"),
                    expression: Box::new(Integer{value: 3}.into())
                },
                Assignment{
                    assignee: VariableAssignee("f"),
                    expression: Box::new(FunctionDefinition {
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(Var("x").into())
                    }.into())
                },
            ],
            expression: Box::new(Var("f").into())
        },
        Some(TypeFn(
            Vec::new(),
            Box::new(TYPE_INT.into())
        ).into()),
        TypeContext::new();
        "fn use closure"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("f"),
                    expression: Box::new(FunctionDefinition {
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(Var("x").into())
                    }.into())
                },
                Assignment{
                    assignee: VariableAssignee("x"),
                    expression: Box::new(Integer{value: 3}.into())
                },
            ],
            expression: Box::new(Var("f").into())
        },
        None,
        TypeContext::new();
        "fn invalid closure"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("x"),
                    expression: Box::new(Integer{value: 3}.into())
                },
                Assignment{
                    assignee: VariableAssignee("f"),
                    expression: Box::new(FunctionDefinition {
                        parameters: vec![
                            TypedAssignee{
                                assignee: Id::from("x").into(),
                                type_: ATOMIC_TYPE_BOOL.into()
                            },
                        ],
                        return_type: ATOMIC_TYPE_BOOL.into(),
                        body: ExpressionBlock(Var("x").into())
                    }.into())
                },
            ],
            expression: Box::new(Var("f").into())
        },
        Some(TypeFn(
            vec![TYPE_BOOL.into()],
            Box::new(TYPE_BOOL.into())
        ).into()),
        TypeContext::new();
        "fn shadowed closure"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("foo"),
                    expression: Box::new(FunctionDefinition {
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_BOOL.into(),
                        body: ExpressionBlock(FunctionCall{
                            function: Box::new(Var("bar").into()),
                            arguments: Vec::new()
                        }.into())
                    }.into())
                },
                Assignment{
                    assignee: VariableAssignee("bar"),
                    expression: Box::new(FunctionDefinition {
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_BOOL.into(),
                        body: ExpressionBlock(FunctionCall{
                            function: Box::new(Var("foo").into()),
                            arguments: Vec::new()
                        }.into())
                    }.into())
                },
            ],
            expression: Box::new(Var("foo").into())
        },
        Some(TypeFn(
            Vec::new(),
            Box::new(TYPE_BOOL.into())
        ).into()),
        TypeContext::new();
        "mutually recursive fns"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("foo"),
                    expression: Box::new(FunctionDefinition {
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_BOOL.into(),
                        body: ExpressionBlock(Boolean{value:true}.into())
                    }.into())
                },
                Assignment{
                    assignee: VariableAssignee("x"),
                    expression: Box::new(FunctionCall{
                        function: Box::new(Var("foo").into()),
                        arguments: Vec::new()
                    }.into())
                },
            ],
            expression: Box::new(Var("x").into())
        },
        Some(TYPE_BOOL.into()),
        TypeContext::new();
        "user defined fn call"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("x"),
                    expression: Box::new(FunctionCall{
                        function: Box::new(Var("foo").into()),
                        arguments: Vec::new()
                    }.into())
                },
                Assignment{
                    assignee: VariableAssignee("foo"),
                    expression: Box::new(FunctionDefinition {
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_BOOL.into(),
                        body: ExpressionBlock(Boolean{value:true}.into())
                    }.into())
                },
            ],
            expression: Box::new(Var("x").into())
        },
        None,
        TypeContext::new();
        "call before definition"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("f"),
                    expression: Box::new(FunctionDefinition {
                        parameters: vec![
                            TypedAssignee{
                                assignee: Id::from("x").into(),
                                type_: ATOMIC_TYPE_BOOL.into()
                            },
                        ],
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: Block{
                            assignments: vec![
                                Assignment{
                                    assignee: VariableAssignee("x"),
                                    expression: Box::new(Integer{value: 3}.into())
                                },
                            ],
                            expression: Box::new(Var("x").into())
                        }
                    }.into())
                },
            ],
            expression: Box::new(Var("f").into())
        },
        Some(TypeFn(
            vec![TYPE_BOOL.into()],
            Box::new(TYPE_INT.into())
        ).into()),
        TypeContext::new();
        "fn shadowed arg"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment{
                    assignee: VariableAssignee("y"),
                    expression: Box::new(Var("x").into())
                },
                Assignment{
                    assignee: VariableAssignee("x"),
                    expression: Box::new(Integer{value: 3}.into())
                },
            ],
            expression: Box::new(Var("y").into())
        },
        None,
        TypeContext::new();
        "block flipped assignments"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Id::from("y").into(),
                    type_: ATOMIC_TYPE_BOOL.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Var("z").into())
        }.into()),
        None,
        TypeContext::new();
        "function invalid block"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Id::from("y").into(),
                    type_: ATOMIC_TYPE_BOOL.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Var("y").into())
        }.into()),
        None,
        TypeContext::new();
        "function incorrect return type"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: ATOMIC_TYPE_BOOL.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Integer{value: 5}.into())
        }.into()),
        None,
        TypeContext::new();
        "function duplicate parameter"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: ParametricAssignee{
                        assignee: Assignee{
                            id: Id::from("g")
                        },
                        generic_variables: vec![Id::from("T"), Id::from("U")]
                    },
                    expression: Box::new(
                        FunctionDefinition {
                            parameters: vec![
                                TypedAssignee{
                                    assignee: Id::from("f").into(),
                                    type_: FunctionType{
                                        argument_types: vec![
                                            GenericType{
                                                id: Id::from("T"),
                                                type_variables: Vec::new()
                                            }.into()
                                        ],
                                        return_type: Box::new(GenericType{
                                            id: Id::from("U"),
                                            type_variables: Vec::new()
                                        }.into())
                                    }.into(),
                                },
                                TypedAssignee{
                                    assignee: Id::from("x").into(),
                                    type_: GenericType{
                                        id: Id::from("T"),
                                        type_variables: Vec::new()
                                    }.into()
                                },
                            ],
                            return_type: GenericType{
                                id: Id::from("U"),
                                type_variables: Vec::new()
                            }.into(),
                            body: ExpressionBlock(FunctionCall{
                                function: Box::new(Var("f").into()),
                                arguments: vec![Var("x").into()]
                            }.into())
                        }.into()
                    )
                }
            ],
            expression: Box::new(GenericVariable{
                id: Id::from("g"),
                type_instances: vec![
                    ATOMIC_TYPE_INT.into(),
                    ATOMIC_TYPE_BOOL.into()
                ]
            }.into())
        },
        Some(Type::from(TypeFn(
            vec![
                Type::from(TypeFn(vec![TYPE_INT],Box::new(TYPE_BOOL))),
                TYPE_INT
            ],
            Box::new(TYPE_BOOL),
        ))),
        TypeContext::new();
        "function matched generics"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: ParametricAssignee{
                        assignee: Assignee{
                            id: Id::from("g")
                        },
                        generic_variables: vec![Id::from("T"), Id::from("U")]
                    },
                    expression: Box::new(
                        FunctionDefinition {
                            parameters: vec![
                                TypedAssignee{
                                    assignee: Id::from("f").into(),
                                    type_: FunctionType{
                                        argument_types: vec![
                                            GenericType{
                                                id: Id::from("T"),
                                                type_variables: Vec::new()
                                            }.into()
                                        ],
                                        return_type: Box::new(GenericType{
                                            id: Id::from("U"),
                                            type_variables: Vec::new()
                                        }.into())
                                    }.into(),
                                },
                                TypedAssignee{
                                    assignee: Id::from("x").into(),
                                    type_: GenericType{
                                        id: Id::from("T"),
                                        type_variables: Vec::new()
                                    }.into()
                                },
                            ],
                            return_type: GenericType{
                                id: Id::from("U"),
                                type_variables: Vec::new()
                            }.into(),
                            body: ExpressionBlock(Var("x").into())
                        }.into()
                    )
                }
            ],
            expression: Box::new(GenericVariable{
                id: Id::from("g"),
                type_instances: vec![
                    ATOMIC_TYPE_INT.into(),
                    ATOMIC_TYPE_BOOL.into()
                ]
            }.into())
        },
        None,
        TypeContext::new();
        "function mismatched generics"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: Typename("opaque_int").into()
                },
            ],
            return_type: Typename("opaque_int").into(),
            body: ExpressionBlock(Var("x").into()),
        }.into()),
        Some(Type::from(TypeFn(
            vec![Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("opaque_int")].clone()), instances: Vec::new()})],
            Box::new(Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("opaque_int")].clone()), instances: Vec::new()}))
        ))),
        TypeContext::new();
        "opaque type reference"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: Typename("transparent_int").into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Var("x").into()),
        }.into()),
        Some(Type::from(TypeFn(
            vec![Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("transparent_int")].clone()), instances: Vec::new()})],
            Box::new(TYPE_INT)
        ))),
        TypeContext::new();
        "transparent type reference"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: Typename("transparent_int").into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(Var("x").into()),
        }.into()),
        Some(Type::from(TypeFn(
            vec![Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("transparent_int")].clone()), instances: Vec::new()})],
            Box::new(Type::from(TypeInstantiation{reference: TYPE_DEFINITIONS.with(|definitions| definitions[&Id::from("transparent_int_2")].clone()), instances: Vec::new()}))
        ))),
        TypeContext::new();
        "double transparent type reference"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: Typename("ii").into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(ElementAccess{
                expression: Box::new(Var("x").into()),
                index: 0
            }.into()),
        }.into()),
        Some(Type::from(TypeFn(
            vec![Type::from(TypeTuple(vec![TYPE_INT, TYPE_INT]))],
            Box::new(TYPE_INT)
        ))),
        TypeContext::new();
        "transparent type usage"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition {
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: Typename("recursive").into()
                },
            ],
            return_type: Typename("recursive").into(),
            body: ExpressionBlock(ElementAccess{
                expression: Box::new(Var("x").into()),
                index: 0
            }.into()),
        }.into()),
        None,
        TypeContext::new();
        "invalid recursive type usage"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition{
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Id::from("y").into(),
                    type_: ATOMIC_TYPE_INT.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(FunctionCall {
                function: Box::new(Var("+").into()),
                arguments: vec![
                    Var("x").into(),
                    Var("y").into(),
                ],
            }.into())
        }.into()),
        Some(Type::from(TypeFn(
            vec![TYPE_INT, TYPE_INT],
            Box::new(TYPE_INT)
        ))),
        TypeContext::from([(
            Id::from("+"),
            Type::from(TypeFn(
                vec![TYPE_INT, TYPE_INT],
                Box::new(TYPE_INT)
            )).into()
        )]);
        "add function definition"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition{
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: ATOMIC_TYPE_INT.into()
                },
                TypedAssignee{
                    assignee: Id::from("y").into(),
                    type_: ATOMIC_TYPE_BOOL.into()
                },
            ],
            return_type: ATOMIC_TYPE_INT.into(),
            body: ExpressionBlock(FunctionCall {
                function: Box::new(Var("+").into()),
                arguments: vec![
                    Var("x").into(),
                    Var("y").into(),
                ],
            }.into())
        }.into()),
        None,
        TypeContext::from([(
            Id::from("+"),
            Type::from(TypeFn(
                vec![TYPE_INT, TYPE_INT],
                Box::new(TYPE_INT)
            )).into()
        )]);
        "add invalid function definition"
    )]
    #[test_case(
        ExpressionBlock(FunctionDefinition{
            parameters: vec![
                TypedAssignee{
                    assignee: Id::from("x").into(),
                    type_: Typename("opaque_int").into()
                },
            ],
            return_type: Typename("opaque_int_2").into(),
            body: ExpressionBlock(Var("x").into())
        }.into()),
        None,
        TypeContext::new();
        "mixed opaque types"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: ParametricAssignee {
                        assignee: Id::from("x").into(),
                        generic_variables: vec![Id::from("T")]
                    },
                    expression: Box::new(Integer {value: -12}.into())
                },
            ],
            expression: Box::new(
                GenericVariable {
                    id: Id::from("x"),
                    type_instances: vec![
                        ATOMIC_TYPE_INT.into()
                    ]
                }.into()
            )
        },
        Some(TYPE_INT),
        TypeContext::new();
        "generic instantiation"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: ParametricAssignee {
                        assignee: Id::from("x").into(),
                        generic_variables: vec![Id::from("T")]
                    },
                    expression: Box::new(Integer {value: -12}.into())
                },
            ],
            expression: Box::new(
                GenericVariable {
                    id: Id::from("x"),
                    type_instances: vec![
                        ATOMIC_TYPE_INT.into(),
                        ATOMIC_TYPE_BOOL.into()
                    ]
                }.into()
            )
        },
        None,
        TypeContext::new();
        "extra variable generic instantiation"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: ParametricAssignee {
                        assignee: Id::from("id").into(),
                        generic_variables: vec![Id::from("T")]
                    },
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Id::from("x").into(),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(Var("x").into())
                    }.into())
                },
            ],
            expression: Box::new(
                FunctionCall {
                    function: Box::new(GenericVariable{
                        id: Id::from("id"),
                        type_instances: vec![ATOMIC_TYPE_INT.into()]
                    }.into()),
                    arguments: vec![Integer{value: 5}.into()]
                }.into()
            )
        },
        Some(TYPE_INT),
        TypeContext::new();
        "generic function instantiation"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: ParametricAssignee {
                        assignee: Id::from("id").into(),
                        generic_variables: vec![Id::from("T")]
                    },
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Id::from("x").into(),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(Var("x").into())
                    }.into())
                },
                Assignment {
                    assignee: ParametricAssignee {
                        assignee: Id::from("id_").into(),
                        generic_variables: vec![Id::from("U")]
                    },
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Id::from("x").into(),
                                type_: Typename("U").into(),
                            }
                        ],
                        return_type: Typename("U").into(),
                        body: ExpressionBlock(FunctionCall {
                            function: Box::new(GenericVariable{
                                id: Id::from("id"),
                                type_instances: vec![Typename("U").into()]
                            }.into()),
                            arguments: vec![Var("x").into()]
                        }.into())
                    }.into())
                },
            ],
            expression: Box::new(
                FunctionCall {
                    function: Box::new(GenericVariable{
                        id: Id::from("id_"),
                        type_instances: vec![ATOMIC_TYPE_INT.into()]
                    }.into()),
                    arguments: vec![Integer{value: 5}.into()]
                }.into()
            )
        },
        Some(TYPE_INT),
        TypeContext::new();
        "used generic function"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: ParametricAssignee {
                        assignee: Id::from("id").into(),
                        generic_variables: vec![Id::from("T")]
                    },
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Id::from("x").into(),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: Block{
                            assignments: vec![
                                Assignment {
                                    assignee: ParametricAssignee {
                                        assignee: Id::from("hold").into(),
                                        generic_variables: vec![Id::from("U")]
                                    },
                                    expression: Box::new(FunctionDefinition{
                                        parameters: vec![
                                            TypedAssignee {
                                                assignee: Id::from("y").into(),
                                                type_: Typename("U").into(),
                                            }
                                        ],
                                        return_type: Typename("T").into(),
                                        body: ExpressionBlock(Var("x").into())
                                    }.into())
                                },
                            ],
                            expression: Box::new(FunctionCall {
                                function: Box::new(GenericVariable{
                                    id: Id::from("hold"),
                                    type_instances: vec![ATOMIC_TYPE_BOOL.into()]
                                }.into()),
                                arguments: vec![
                                    Boolean{value: false}.into()
                                ]
                            }.into())
                        }
                    }.into())
                },
            ],
            expression: Box::new(
                FunctionCall {
                    function: Box::new(GenericVariable{
                        id: Id::from("id"),
                        type_instances: vec![ATOMIC_TYPE_INT.into()]
                    }.into()),
                    arguments: vec![Integer{value: 5}.into()]
                }.into()
            )
        },
        Some(TYPE_INT),
        TypeContext::new();
        "nested generic function instantiation"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: ParametricAssignee {
                        assignee: Id::from("id").into(),
                        generic_variables: vec![Id::from("T")]
                    },
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Id::from("x").into(),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(Var("x").into())
                    }.into())
                },
            ],
            expression: Box::new(
                FunctionCall {
                    function: Box::new(Var("&").into()),
                    arguments: vec![
                        FunctionCall {
                            function: Box::new(GenericVariable{
                                id: Id::from("id"),
                                type_instances: vec![ATOMIC_TYPE_INT.into()]
                            }.into()),
                            arguments: vec![Integer{value: 5}.into()]
                        }.into(),
                        FunctionCall {
                            function: Box::new(GenericVariable{
                                id: Id::from("id"),
                                type_instances: vec![ATOMIC_TYPE_BOOL.into()]
                            }.into()),
                            arguments: vec![Boolean{value: false}.into()]
                        }.into()
                    ]
                }.into()
            )
        },
        Some(TYPE_INT),
        TypeContext::from([(
            Id::from("&"),
            Type::from(TypeFn(vec![TYPE_INT, TYPE_BOOL], Box::new(TYPE_INT))).into()
        )]);
        "reused generic function"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: ParametricAssignee {
                        assignee: Id::from("apply").into(),
                        generic_variables: vec![Id::from("T"), Id::from("U")]
                    },
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Id::from("f").into(),
                                type_: FunctionType{
                                    argument_types: vec![Typename("T").into()],
                                    return_type: Box::new(Typename("U").into())
                                }.into(),
                            },
                            TypedAssignee {
                                assignee: Id::from("x").into(),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("U").into(),
                        body: ExpressionBlock(FunctionCall {
                            function: Box::new(Var("f").into()),
                            arguments: vec![Var("x").into()]
                        }.into())
                    }.into())
                },
            ],
            expression: Box::new(
                GenericVariable{
                    id: Id::from("apply"),
                    type_instances: vec![ATOMIC_TYPE_INT.into(), ATOMIC_TYPE_BOOL.into()]
                }.into()
            )
        },
        Some(Type::from(TypeFn(
            vec![
                Type::from(TypeFn(vec![TYPE_INT], Box::new(TYPE_BOOL))),
                TYPE_INT
            ],
            Box::new(TYPE_BOOL)
        ))),
        TypeContext::new();
        "compound generic function"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: ParametricAssignee {
                        assignee: Id::from("extra").into(),
                        generic_variables: vec![Id::from("T"), Id::from("U")]
                    },
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Id::from("x").into(),
                                type_: Typename("T").into(),
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(Var("x").into())
                    }.into())
                },
            ],
            expression: Box::new(
                GenericVariable{
                    id: Id::from("extra"),
                    type_instances: vec![ATOMIC_TYPE_INT.into(), ATOMIC_TYPE_BOOL.into()]
                }.into()
            )
        },
        Some(Type::from(TypeFn(
            vec![
                TYPE_INT
            ],
            Box::new(TYPE_INT)
        ))),
        TypeContext::new();
        "dual generic function"
    )]
    #[test_case(
        Block {
            assignments: vec![
                Assignment {
                    assignee: ParametricAssignee {
                        assignee: Id::from("first").into(),
                        generic_variables: vec![Id::from("T"), Id::from("U")]
                    },
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Id::from("x").into(),
                                type_: TupleType{
                                    types: vec![Typename("T").into(), Typename("U").into()],
                                }.into()
                            },
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(ElementAccess {
                            expression: Box::new(Var("x").into()),
                            index: 0
                        }.into())
                    }.into())
                },
            ],
            expression: Box::new(
                GenericVariable{
                    id: Id::from("first"),
                    type_instances: vec![ATOMIC_TYPE_INT.into(), ATOMIC_TYPE_BOOL.into()]
                }.into()
            )
        },
        Some(Type::from(TypeFn(
            vec![
                Type::from(TypeTuple(vec![TYPE_INT, TYPE_BOOL])),
            ],
            Box::new(TYPE_INT)
        ))),
        TypeContext::new();
        "tuple generic function"
    )]
    fn test_check_blocks(block: Block, expected_type: Option<Type>, context: TypeContext) {
        let type_checker = TypeChecker {
            type_definitions: TYPE_DEFINITIONS
                .with(|definitions| TypeDefinitions::from(definitions.0.clone())),
            constructors: TYPE_CONSTRUCTORS.clone(),
        };
        let type_check_result = type_checker.check_block(block, context, GenericVariables::new());
        match expected_type {
            Some(type_) => match &type_check_result {
                Ok(typed_block) => {
                    dbg!(&typed_block);
                    dbg!(typed_block.type_());
                    dbg!(&type_);
                    assert!(Type::equality(&typed_block.type_(), &type_))
                }
                Err(msg) => {
                    dbg!(msg);
                    assert!(&type_check_result.is_ok());
                }
            },
            None => {
                if type_check_result.is_ok() {
                    dbg!(&type_check_result);
                }
                assert!(&type_check_result.is_err());
            }
        }
    }

    #[test]
    fn test_valid_constructor_list() {
        let type_definitions = vec![
            UnionTypeDefinition {
                variable: GenericTypeVariable {
                    id: Id::from("Tree"),
                    generic_variables: vec![Id::from("T")],
                },
                items: vec![
                    TypeItem {
                        id: Id::from("Node"),
                        type_: Some(
                            TupleType {
                                types: vec![
                                    Typename("T").into(),
                                    GenericType {
                                        id: Id::from("Tree"),
                                        type_variables: vec![Typename("T").into()],
                                    }
                                    .into(),
                                    Typename("T").into(),
                                ],
                            }
                            .into(),
                        ),
                    },
                    TypeItem {
                        id: Id::from("Leaf"),
                        type_: None,
                    },
                ],
            }
            .into(),
            UnionTypeDefinition {
                variable: GenericTypeVariable {
                    id: Id::from("Empty"),
                    generic_variables: Vec::new(),
                },
                items: Vec::new(),
            }
            .into(),
            OpaqueTypeDefinition {
                variable: GenericTypeVariable {
                    id: Id::from("opaque_int"),
                    generic_variables: Vec::new(),
                },
                type_: ATOMIC_TYPE_INT.into(),
            }
            .into(),
            OpaqueTypeDefinition {
                variable: GenericTypeVariable {
                    id: Id::from("opaque_opaque_int"),
                    generic_variables: Vec::new(),
                },
                type_: Typename("opaque_int").into(),
            }
            .into(),
            OpaqueTypeDefinition {
                variable: GenericTypeVariable {
                    id: Id::from("int_tree"),
                    generic_variables: Vec::new(),
                },
                type_: GenericType {
                    id: Id::from("Tree"),
                    type_variables: vec![ATOMIC_TYPE_INT.into()],
                }
                .into(),
            }
            .into(),
        ];
        let expected_constructors = HashMap::from([
            ("Tree", vec!["Node", "Leaf"]),
            ("Empty", vec![]),
            ("opaque_int", vec!["opaque_int"]),
            ("opaque_opaque_int", vec!["opaque_opaque_int"]),
            ("int_tree", vec!["int_tree"]),
        ]);
        let Ok(type_checker) = TypeChecker::check_type_definitions(type_definitions) else {
            panic!("Invalid type checker definition");
        };
        assert_eq!(
            type_checker.constructors,
            expected_constructors
                .into_iter()
                .map(|(type_name, constructors)| constructors
                    .into_iter()
                    .enumerate()
                    .map(|(i, id)| (
                        Id::from(id),
                        ConstructorType {
                            type_: type_checker.type_definitions[&Id::from(type_name)].clone(),
                            index: i
                        }
                    ))
                    .collect_vec())
                .concat()
                .into_iter()
                .collect::<HashMap<_, _>>()
        )
    }

    #[test]
    fn test_invalid_constructor_list() {
        let type_definitions = vec![
            UnionTypeDefinition {
                variable: GenericTypeVariable {
                    id: Id::from("Tree"),
                    generic_variables: vec![Id::from("T")],
                },
                items: vec![
                    TypeItem {
                        id: Id::from("Node"),
                        type_: Some(
                            TupleType {
                                types: vec![
                                    Typename("T").into(),
                                    GenericType {
                                        id: Id::from("Tree"),
                                        type_variables: vec![Typename("T").into()],
                                    }
                                    .into(),
                                    Typename("T").into(),
                                ],
                            }
                            .into(),
                        ),
                    },
                    TypeItem {
                        id: Id::from("Leaf"),
                        type_: None,
                    },
                ],
            }
            .into(),
            EmptyTypeDefinition {
                id: Id::from("Leaf"),
            }
            .into(),
        ];
        let result = TypeChecker::check_type_definitions(type_definitions);
        assert!(result.is_err())
    }

    #[test_case(
        Program{
            definitions: Vec::new()
        },
        Err(()),
        TypeContext::new();
        "empty program"
    )]
    #[test_case(
        Program{
            definitions: vec![
                Assignment{
                    assignee: VariableAssignee("main"),
                    expression: Box::new(FunctionDefinition{
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(Integer{value: 0}.into())
                    }.into())
                }.into()
            ]
        },
        Ok(()),
        TypeContext::new();
        "basic program"
    )]
    #[test_case(
        Program{
            definitions: vec![
                Assignment{
                    assignee: VariableAssignee("main"),
                    expression: Box::new(FunctionDefinition{
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(
                            FunctionCall {
                                arguments: vec![
                                    Integer{value: 1}.into(),
                                    Integer{value: -1}.into()
                                ],
                                function: Box::new(Var("+").into())
                            }.into()
                        )
                    }.into())
                }.into()
            ]
        },
        Ok(()),
        TypeContext::from([(
            Id::from("+"),
            Type::from(TypeFn(
                vec![TYPE_INT, TYPE_INT],
                Box::new(TYPE_INT)
            )).into()
        )]);
        "basic using context"
    )]
    #[test_case(
        Program{
            definitions: vec![
                Assignment{
                    assignee: VariableAssignee("main"),
                    expression: Box::new(FunctionDefinition{
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(
                            FunctionCall {
                                arguments: vec![
                                    Integer{value: 1}.into(),
                                    Integer{value: -1}.into()
                                ],
                                function: Box::new(Var("+").into())
                            }.into()
                        )
                    }.into())
                }.into()
            ]
        },
        Err(()),
        TypeContext::new();
        "basic outside of context"
    )]
    #[test_case(
        Program{
            definitions: vec![
                OpaqueTypeDefinition {
                    variable: GenericTypeVariable {
                        id: Id::from("opaque_int"),
                        generic_variables: Vec::new()
                    },
                    type_: ATOMIC_TYPE_INT.into()
                }.into(),
                Assignment{
                    assignee: VariableAssignee("main"),
                    expression: Box::new(FunctionDefinition{
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(
                            ConstructorCall {
                                arguments: vec![
                                    Integer{value: -1}.into(),
                                ],
                                constructor: GenericConstructor {
                                    id: Id::from("opaque_int"),
                                    type_instances: Vec::new()
                                }
                            }.into()
                        )
                    }.into())
                }.into()
            ]
        },
        Err(()),
        TypeContext::new();
        "opaque type definition usage"
    )]
    #[test_case(
        Program{
            definitions: vec![
                OpaqueTypeDefinition {
                    variable: GenericTypeVariable {
                        id: Id::from("opaque_int"),
                        generic_variables: Vec::new()
                    },
                    type_: ATOMIC_TYPE_INT.into()
                }.into(),
                TransparentTypeDefinition {
                    variable: GenericTypeVariable {
                        id: Id::from("transparent_int"),
                        generic_variables: Vec::new()
                    },
                    type_: ATOMIC_TYPE_INT.into()
                }.into(),
                Assignment{
                    assignee: VariableAssignee("main"),
                    expression: Box::new(FunctionDefinition{
                        parameters: Vec::new(),
                        return_type: Typename("transparent_int").into(),
                        body: Block{
                            assignments: vec![
                                Assignment {
                                    assignee: VariableAssignee("x"),
                                    expression: Box::new(ConstructorCall {
                                        arguments: vec![
                                            Integer{value: -1}.into(),
                                        ],
                                        constructor: GenericConstructor {
                                            id: Id::from("opaque_int"),
                                            type_instances: Vec::new()
                                        }
                                    }.into())
                                },
                            ],
                            expression: Box::new(MatchExpression {
                                subject: Box::new(Var("x").into()),
                                blocks: vec![
                                    MatchBlock {
                                        matches: vec![
                                            MatchItem {
                                                type_name: Id::from("opaque_int"),
                                                assignee: Some(Assignee{
                                                    id: Id::from("x")
                                                })
                                            },
                                        ],
                                        block: ExpressionBlock(Var("x").into())
                                    }
                                ]
                            }.into()),
                        }
                    }.into())
                }.into()
            ]
        },
        Ok(()),
        TypeContext::new();
        "type definition match expression"
    )]
    #[test_case(
        Program{
            definitions: vec![
                UnionTypeDefinition {
                    variable: GenericTypeVariable{
                        id: Id::from("Maybe"),
                        generic_variables: vec![Id::from("T"), Id::from("U")]
                    },
                    items: vec![
                        TypeItem {
                            id: Id::from("Left"),
                            type_: Some(Typename("T").into())
                        },
                        TypeItem {
                            id: Id::from("Right"),
                            type_: Some(Typename("U").into())
                        }
                    ]
                }.into(),
                Assignment{
                    assignee: VariableAssignee("main"),
                    expression: Box::new(FunctionDefinition{
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: Block{
                            assignments: vec![
                                Assignment {
                                    assignee: VariableAssignee("x"),
                                    expression: Box::new(ConstructorCall {
                                        arguments: vec![
                                            Integer{value: 0}.into(),
                                        ],
                                        constructor: GenericConstructor {
                                            id: Id::from("Left"),
                                            type_instances: vec![
                                                ATOMIC_TYPE_INT.into(),
                                                ATOMIC_TYPE_BOOL.into()
                                            ]
                                        }
                                    }.into())
                                },
                            ],
                            expression: Box::new(MatchExpression {
                                subject: Box::new(Var("x").into()),
                                blocks: vec![
                                    MatchBlock {
                                        matches: vec![
                                            MatchItem {
                                                type_name: Id::from("Left"),
                                                assignee: Some(Assignee{
                                                    id: Id::from("x")
                                                })
                                            },
                                        ],
                                        block: ExpressionBlock(Var("x").into())
                                    },
                                    MatchBlock {
                                        matches: vec![
                                            MatchItem {
                                                type_name: Id::from("Right"),
                                                assignee: Some(Assignee{
                                                    id: Id::from("x")
                                                })
                                            },
                                        ],
                                        block: ExpressionBlock(IfExpression{
                                            condition: Box::new(Var("x").into()),
                                            true_block: ExpressionBlock(Integer{value: 1}.into()),
                                            false_block: ExpressionBlock(Integer{value: -1}.into()),
                                        }.into())
                                    }
                                ]
                            }.into()),
                        }
                    }.into())
                }.into()
            ]
        },
        Ok(()),
        TypeContext::new();
        "union type instantiation"
    )]
    #[test_case(
        Program{
            definitions: vec![
                Assignment{
                    assignee: VariableAssignee("main"),
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Assignee { id: Id::from("x") },
                                type_: TupleType { types: Vec::new() }.into()
                            }
                        ],
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(
                            Integer{value: 1}.into(),
                        )
                    }.into())
                }.into()
            ]
        },
        Err(()),
        TypeContext::new();
        "too many arguments"
    )]
    #[test_case(
        Program{
            definitions: vec![
                Assignment{
                    assignee: ParametricAssignee {
                        assignee: Assignee {
                            id: Id::from("main")
                        },
                        generic_variables: vec![Id::from("T")]
                    },
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![],
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(
                            Integer{value: 1}.into(),
                        )
                    }.into())
                }.into()
            ]
        },
        Err(()),
        TypeContext::new();
        "parametric main"
    )]
    #[test_case(
        Program{
            definitions: vec![
                Assignment{
                    assignee: VariableAssignee("main"),
                    expression: Box::new(FunctionDefinition{
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(FunctionCall{
                            function: Box::new(GenericVariable{
                                id: Id::from("identity"),
                                type_instances: vec![ATOMIC_TYPE_INT.into()]
                            }.into()),
                            arguments: vec![
                                Integer{ value: 11 }.into()
                            ]
                        }.into())
                    }.into())
                }.into(),
                Assignment{
                    assignee: ParametricAssignee{
                        assignee: Assignee { id: Id::from("identity") },
                        generic_variables: vec![Id::from("T")]
                    },
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Assignee {
                                    id: Id::from("x")
                                },
                                type_: Typename("T").into()
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(Var("x").into())
                    }.into())
                }.into()
            ]
        },
        Ok(()),
        TypeContext::new();
        "function type instantiation"
    )]
    #[test_case(
        Program{
            definitions: vec![
                Assignment{
                    assignee: VariableAssignee("main"),
                    expression: Box::new(FunctionDefinition{
                        parameters: Vec::new(),
                        return_type: FunctionType{
                            argument_types: vec![ATOMIC_TYPE_INT.into()],
                            return_type: Box::new(ATOMIC_TYPE_INT.into()),
                        }.into(),
                        body: ExpressionBlock(GenericVariable{
                            id: Id::from("identity"),
                            type_instances: vec![ATOMIC_TYPE_INT.into()]
                        }.into())
                    }.into())
                }.into(),
                Assignment{
                    assignee: ParametricAssignee{
                        assignee: Assignee { id: Id::from("identity") },
                        generic_variables: vec![Id::from("T")]
                    },
                    expression: Box::new(FunctionDefinition{
                        parameters: vec![
                            TypedAssignee {
                                assignee: Assignee {
                                    id: Id::from("x")
                                },
                                type_: Typename("T").into()
                            }
                        ],
                        return_type: Typename("T").into(),
                        body: ExpressionBlock(Var("x").into())
                    }.into())
                }.into()
            ]
        },
        Err(()),
        TypeContext::new();
        "returning function from main"
    )]
    fn test_program(program: Program, result: Result<(), ()>, context: TypeContext) {
        let type_check_result = TypeChecker::check_program(program, &context);
        match (type_check_result.clone(), result) {
            (Ok(program), Err(())) => {
                dbg!(program);
                assert!(type_check_result.is_err())
            }
            (Err(msg), Ok(())) => {
                dbg!(msg);
                assert!(type_check_result.is_ok())
            }
            _ => (),
        }
    }

    #[test_case(
        Program{
            definitions: vec![
                Assignment{
                    assignee: VariableAssignee("main"),
                    expression: Box::new(FunctionDefinition{
                        parameters: Vec::new(),
                        return_type: ATOMIC_TYPE_INT.into(),
                        body: ExpressionBlock(FunctionCall{
                            function: Box::new(GenericVariable{
                                id: Id::from("+"),
                                type_instances: Vec::new()
                            }.into()),
                            arguments: vec![
                                Integer{ value: -1 }.into(),
                                Integer{ value: 2 }.into(),
                            ]
                        }.into())
                    }.into())
                }.into(),
            ]
        },
        Ok(());
        "default operator usage"
    )]
    fn test_default_program(program: Program, result: Result<(), ()>) {
        let type_check_result = TypeChecker::type_check(program);
        match (type_check_result.clone(), result) {
            (Ok(program), Err(())) => {
                dbg!(program);
                assert!(type_check_result.is_err())
            }
            (Err(msg), Ok(())) => {
                dbg!(msg);
                assert!(type_check_result.is_ok())
            }
            _ => (),
        }
    }
}
