use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    rc::Rc,
};

use crate::intermediate_nodes::*;
use itertools::{zip_eq, Itertools};
use type_checker::*;

type Scope = HashMap<(Variable, Vec<Type>), IntermediateValue>;
type History = HashMap<IntermediateExpression, IntermediateValue>;
type Uninstantiated = HashMap<Variable, TypedStatement>;
type TypeDefs = HashMap<Type, Rc<RefCell<IntermediateType>>>;
type MemoryMap = HashMap<Location, Vec<IntermediateExpression>>;

pub struct Lowerer {
    scope: Scope,
    history: History,
    uninstantiated: Uninstantiated,
    type_defs: TypeDefs,
    statements: Vec<IntermediateStatement>,
    memory: MemoryMap,
}

impl Lowerer {
    pub fn new() -> Self {
        let mut lowerer = Lowerer {
            scope: Scope::new(),
            history: History::new(),
            uninstantiated: Uninstantiated::new(),
            type_defs: TypeDefs::new(),
            statements: Vec::new(),
            memory: MemoryMap::new(),
        };
        let scope = DEFAULT_CONTEXT.with(|context| {
            Scope::from_iter(context.iter().map(|(id, var)| {
                let type_ = lowerer.lower_type(&var.type_.type_);
                let variable = var.variable.clone();
                ((variable, Vec::new()), BuiltInFn(id.clone(), type_).into())
            }))
        });
        lowerer.scope = scope;
        lowerer
    }
    fn update_memory(&mut self, location: Location, expression: IntermediateExpression) {
        let values = self.memory.entry(location).or_insert(Vec::new());
        values.push(expression);
    }
    fn get_cached_value(
        &mut self,
        intermediate_expression: IntermediateExpression,
    ) -> IntermediateValue {
        if let Some(cached) = self.history.get(&intermediate_expression) {
            return cached.clone();
        }
        let assignment: IntermediateAssignment = intermediate_expression.clone().into();
        self.update_memory(assignment.location.clone(), assignment.expression.clone());
        self.statements.push(assignment.clone().into());
        let value: IntermediateValue = assignment.location.into();
        self.history.insert(intermediate_expression, value.clone());
        value
    }

    fn lower_expression(&mut self, expression: TypedExpression) -> IntermediateValue {
        match expression {
            TypedExpression::Integer(integer) => IntermediateBuiltIn::Integer(integer).into(),
            TypedExpression::Boolean(boolean) => IntermediateBuiltIn::Boolean(boolean).into(),
            TypedExpression::TypedTuple(tuple) => self.lower_tuple(tuple),
            TypedExpression::TypedElementAccess(element_access) => {
                self.lower_element_access(element_access)
            }
            TypedExpression::TypedAccess(access) => self.lower_access(access),
            TypedExpression::TypedFunctionCall(fn_call) => self.lower_fn_call(fn_call),
            TypedExpression::TypedLambdaDef(fn_def) => self.lower_fn_def(fn_def),
            TypedExpression::TypedConstructorCall(ctor_call) => self.lower_ctor_call(ctor_call),
            TypedExpression::TypedIf(if_) => self.lower_if(if_),
            TypedExpression::TypedMatch(match_) => self.lower_match(match_),
        }
    }
    fn lower_tuple(&mut self, TypedTuple { expressions }: TypedTuple) -> IntermediateValue {
        let intermediate_expressions = self.lower_expressions(expressions);
        let intermediate_expression = IntermediateTupleExpression(intermediate_expressions).into();
        self.get_cached_value(intermediate_expression)
    }
    fn lower_element_access(
        &mut self,
        TypedElementAccess { expression, index }: TypedElementAccess,
    ) -> IntermediateValue {
        let intermediate_value = self.lower_expression(*expression);
        let intermediate_expression = IntermediateElementAccess {
            value: intermediate_value,
            idx: index,
        }
        .into();
        self.get_cached_value(intermediate_expression)
    }
    fn lower_access(
        &mut self,
        TypedAccess {
            variable,
            parameters,
        }: TypedAccess,
    ) -> IntermediateValue {
        if !self
            .scope
            .contains_key(&(variable.variable.clone(), parameters.clone()))
        {
            let uninstantiated = &self.uninstantiated[&variable.variable];
            let (location, expression) = self
                .add_placeholder_assignment(uninstantiated.clone(), Some(parameters.clone()))
                .unwrap();

            self.perform_assignment(location, expression);
        };
        self.scope[&(variable.variable, parameters)].clone()
    }
    fn lower_fn_call(
        &mut self,
        TypedFunctionCall {
            function,
            arguments,
        }: TypedFunctionCall,
    ) -> IntermediateValue {
        let intermediate_function = self.lower_expression(*function);
        let intermediate_args = self.lower_expressions(arguments);
        let intermediate_expression = IntermediateFnCall {
            fn_: intermediate_function,
            args: intermediate_args,
        };
        self.get_cached_value(intermediate_expression.into())
    }
    fn lower_fn_def(
        &mut self,
        TypedLambdaDef {
            parameters,
            body,
            return_type,
        }: TypedLambdaDef,
    ) -> IntermediateValue {
        let variables = parameters
            .iter()
            .map(|variable| variable.variable.clone())
            .collect::<Vec<_>>();
        let args = parameters
            .iter()
            .map(|variable| IntermediateArg::from(self.lower_type(&variable.type_.type_)))
            .collect::<Vec<_>>();
        for (variable, arg) in zip_eq(&variables, &args) {
            self.update_memory(arg.location.clone(), arg.clone().into());
            self.scope
                .insert((variable.clone(), Vec::new()), arg.clone().into());
        }
        let (statements, return_value) = self.lower_block(body, false);
        let intermediate_expression = IntermediateLambda {
            args,
            statements,
            ret: (return_value, self.lower_type(&return_type)),
        }
        .into();
        self.get_cached_value(intermediate_expression)
    }
    fn lower_ctor_call(
        &mut self,
        TypedConstructorCall {
            idx,
            output_type,
            arguments,
        }: TypedConstructorCall,
    ) -> IntermediateValue {
        let IntermediateType::IntermediateUnionType(lower_type) = self.lower_type(&output_type)
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
    fn lower_if(
        &mut self,
        TypedIf {
            condition,
            true_block,
            false_block,
        }: TypedIf,
    ) -> IntermediateValue {
        let lower_condition = self.lower_expression(*condition);
        let lower_true_block = self.lower_block(true_block, true);
        let lower_false_block = self.lower_block(false_block, true);
        let (value, statements) = self.merge_blocks(vec![lower_true_block, lower_false_block]);
        let [ref true_branch, ref false_branch] = statements[..] else {
            panic!("Number of branches changed size.")
        };
        self.statements.push(
            IntermediateIfStatement {
                condition: lower_condition,
                branches: (true_branch.clone(), false_branch.clone()),
            }
            .into(),
        );
        value
    }
    fn lower_match(&mut self, TypedMatch { subject, blocks }: TypedMatch) -> IntermediateValue {
        let lower_subject = self.lower_expression(*subject);
        let matches = BTreeMap::from_iter(blocks.into_iter().flat_map(|block| {
            block
                .matches
                .into_iter()
                .map(move |match_| (match_.type_idx, (match_.assignee, block.block.clone())))
        }));
        let keys = matches.keys().cloned().sorted();
        if Iterator::ne(0..keys.len(), keys) {
            panic!("Match blocks are not exhaustive.");
        }
        let all_assignees: HashMap<Variable, ParametricType> = HashMap::from_iter(
            matches
                .values()
                .filter_map(|(assignee, _)| assignee.clone())
                .map(|assignee| (assignee.variable, assignee.type_)),
        );
        let args: HashMap<Variable, IntermediateArg> =
            HashMap::from_iter(all_assignees.into_iter().map(|(variable, type_)| {
                let arg = IntermediateArg::from(self.lower_type(&type_.type_));
                self.update_memory(arg.location.clone(), arg.clone().into());
                self.scope
                    .insert((variable.clone(), Vec::new()), arg.clone().into());
                (variable, arg)
            }));
        let (args, blocks): (Vec<_>, Vec<_>) = matches
            .into_values()
            .map(|(assignee, block)| {
                (
                    assignee.map(|assignee| args[&assignee.variable].clone()),
                    self.lower_block(block, true),
                )
            })
            .unzip();
        let (value, statements) = self.merge_blocks(blocks);
        let branches = args
            .into_iter()
            .zip(statements.into_iter())
            .map(|(arg, statements)| IntermediateMatchBranch {
                target: arg,
                statements: statements,
            })
            .collect();
        self.statements.push(
            IntermediateMatchStatement {
                subject: lower_subject,
                branches,
            }
            .into(),
        );
        value
    }
    fn lower_expressions(&mut self, expressions: Vec<TypedExpression>) -> Vec<IntermediateValue> {
        expressions
            .into_iter()
            .map(|expression| self.lower_expression(expression))
            .collect()
    }
    fn merge_blocks(
        &mut self,
        blocks: Vec<(Vec<IntermediateStatement>, IntermediateValue)>,
    ) -> (IntermediateValue, Vec<Vec<IntermediateStatement>>) {
        let result_location = Location::new();
        let statements = blocks
            .into_iter()
            .map(|(mut statements, value)| {
                let assignment = IntermediateAssignment {
                    expression: value.into(),
                    location: result_location.clone(),
                };
                self.update_memory(assignment.location.clone(), assignment.expression.clone());
                statements.push(assignment.into());
                statements
            })
            .collect();
        (result_location.into(), statements)
    }
    fn lower_block(
        &mut self,
        block: TypedBlock,
        history_access: bool,
    ) -> (Vec<IntermediateStatement>, IntermediateValue) {
        let statements = self.statements.clone();
        let history = self.history.clone();
        self.statements = Vec::new();
        if !history_access {
            self.history = History::new();
        }
        self.lower_statements(block.statements);
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
            Type::TypeAtomic(atomic) => atomic.clone().into(),
            Type::TypeUnion(TypeUnion {
                id: _,
                variants: types,
            }) => Type::from(TypeUnion {
                id: String::new(),
                variants: types
                    .iter()
                    .map(|type_| type_.as_ref().map(|type_| self.clear_names(&type_)))
                    .collect(),
            }),
            Type::TypeInstantiation(TypeInstantiation {
                reference: type_,
                instances: types,
            }) => Type::TypeInstantiation(TypeInstantiation {
                reference: type_.clone(),
                instances: clear_names(types),
            }),
            Type::TypeTuple(TypeTuple(types)) => Type::from(TypeTuple(clear_names(types))),
            Type::TypeFn(TypeFn(args, ret)) => {
                Type::TypeFn(TypeFn(clear_names(args), Box::new(self.clear_names(&*ret))))
            }
            Type::TypeVariable(TypeVariable(var)) => Type::from(TypeVariable(var.clone())),
        }
    }
    fn remove_wasted_allocations_from_expression(
        &mut self,
        expression: IntermediateExpression,
    ) -> IntermediateExpression {
        match expression {
            IntermediateExpression::IntermediateValue(value) => match value.clone() {
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
            IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args,
                statements,
                ret,
            }) => IntermediateLambda {
                args,
                statements: self.remove_wasted_allocations_from_statements(statements),
                ret: (self.remove_wasted_allocations_from_value(ret.0), ret.1),
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
            IntermediateValue::IntermediateArg(arg) => arg.into(),
            IntermediateValue::IntermediateMemory(location) => {
                let expressions = self.memory.get(&location);
                if expressions.map(Vec::len) == Some(1) {
                    let expressions = expressions.unwrap();
                    let expression = expressions[0].clone();
                    match expression {
                        IntermediateExpression::IntermediateValue(value) => {
                            self.remove_wasted_allocations_from_value(value)
                        }
                        _ => location.into(),
                    }
                } else {
                    location.into()
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
            IntermediateStatement::IntermediateAssignment(assignment) => {
                let IntermediateAssignment {
                    expression,
                    location,
                } = assignment;
                if matches!(&expression, IntermediateExpression::IntermediateValue(_))
                    && self.memory.get(&location).map(Vec::len) == Some(1)
                {
                    return None;
                }
                let condensed_expression =
                    self.remove_wasted_allocations_from_expression(expression.clone());
                let expression = condensed_expression;
                Some(IntermediateStatement::IntermediateAssignment(
                    IntermediateAssignment {
                        location,
                        expression: expression,
                    }
                    .into(),
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
            IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                subject,
                branches,
            }) => Some(
                IntermediateMatchStatement {
                    subject: self.remove_wasted_allocations_from_value(subject),
                    branches: branches
                        .into_iter()
                        .map(|IntermediateMatchBranch { target, statements }| {
                            IntermediateMatchBranch {
                                target,
                                statements: self
                                    .remove_wasted_allocations_from_statements(statements),
                            }
                        })
                        .collect(),
                }
                .into(),
            ),
            IntermediateStatement::IntermediateFnDef(IntermediateFnDef(value)) => {
                Some(IntermediateFnDef(self.remove_wasted_allocations_from_value(value)).into())
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
        let type_ = self.clear_names(type_);
        let lower_type = self.lower_type_internal(&type_, HashSet::new());
        lower_type
    }
    fn lower_type_internal(
        &mut self,
        type_: &Type,
        mut visited_references: HashSet<*mut IntermediateType>,
    ) -> IntermediateType {
        match type_ {
            Type::TypeAtomic(TypeAtomic(atomic)) => atomic.clone().into(),
            Type::TypeUnion(TypeUnion { id: _, variants }) => {
                let type_ = self.clear_names(&Type::from(TypeUnion {
                    id: String::new(),
                    variants: variants.clone(),
                }));
                let lower_type =
                    |this: &mut Self, visited_references: HashSet<*mut IntermediateType>| {
                        IntermediateUnionType(
                            variants
                                .iter()
                                .map(|type_: &Option<Type>| {
                                    type_.as_ref().map(|type_| {
                                        this.lower_type_internal(type_, visited_references.clone())
                                    })
                                })
                                .collect(),
                        )
                        .into()
                    };
                match self.type_defs.entry(type_.clone()) {
                    std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                        visited_references.insert(occupied_entry.get().as_ptr());
                        lower_type(self, visited_references)
                    }
                    std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                        let reference =
                            Rc::new(RefCell::new(IntermediateUnionType(Vec::new()).into()));
                        vacant_entry.insert(reference.clone());
                        visited_references.insert(reference.as_ptr());
                        let lower_type = lower_type(self, visited_references);
                        *reference.clone().borrow_mut() = lower_type.clone();
                        lower_type
                    }
                }
            }
            Type::TypeInstantiation(TypeInstantiation {
                reference: type_,
                instances: params,
            }) => {
                let instantiation = self.clear_names(&type_.borrow().instantiate(params));
                match self.type_defs.entry(instantiation.clone()) {
                    std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                        if visited_references.contains(&occupied_entry.get().as_ptr()) {
                            IntermediateType::Reference(occupied_entry.get().clone())
                        } else {
                            visited_references.insert(occupied_entry.get().as_ptr());
                            occupied_entry.get().borrow().clone()
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                        let reference =
                            Rc::new(RefCell::new(IntermediateUnionType(Vec::new()).into()));
                        vacant_entry.insert(reference.clone());
                        visited_references.insert(reference.as_ptr());
                        let lower_type =
                            self.lower_type_internal(&instantiation, visited_references);
                        *reference.clone().borrow_mut() = lower_type;
                        self.type_defs[&instantiation].borrow().clone()
                    }
                }
            }
            Type::TypeTuple(TypeTuple(types)) => {
                IntermediateTupleType(self.lower_types_internal(types, visited_references)).into()
            }
            Type::TypeFn(TypeFn(args, ret)) => IntermediateFnType(
                self.lower_types_internal(args, visited_references.clone()),
                Box::new(self.lower_type_internal(&*ret, visited_references)),
            )
            .into(),
            Type::TypeVariable(TypeVariable(_)) => panic!("Attempt to lower type variable."),
        }
    }
    pub fn lower_types_internal(
        &mut self,
        types: &Vec<Type>,
        visited_references: HashSet<*mut IntermediateType>,
    ) -> Vec<IntermediateType> {
        types
            .iter()
            .map(|type_| self.lower_type_internal(type_, visited_references.clone()))
            .collect()
    }
    fn add_placeholder_assignment(
        &mut self,
        statement: TypedStatement,
        parameters: Option<Vec<Type>>,
    ) -> Option<(Location, TypedExpression)> {
        let variable = statement.variable();
        if parameters.is_none() && variable.type_.parameters.len() > 0 {
            self.uninstantiated.insert(variable.variable, statement);
            return None;
        }
        let parameters = parameters.unwrap_or(Vec::new());
        let placeholder = Location::new();
        self.scope.insert(
            (variable.variable.clone(), parameters.clone()),
            placeholder.clone().into(),
        );
        let expression = match statement {
            TypedStatement::TypedAssignment(TypedAssignment {
                variable: _,
                expression,
            }) => expression.instantiate(&parameters),
            TypedStatement::TypedFnDef(TypedFnDef {
                variable: _,
                parameters: params,
                fn_,
            }) => {
                let expression = ParametricExpression {
                    expression: fn_.into(),
                    parameters: params,
                };
                self.statements
                    .push(IntermediateFnDef(placeholder.clone().into()).into());
                expression.instantiate(&parameters)
            }
        };
        dbg!(&expression);
        Some((placeholder, expression))
    }
    fn perform_assignment(&mut self, placeholder: Location, expression: TypedExpression) {
        let value = self.lower_expression(expression);
        self.update_memory(placeholder, value.into());
    }
    fn lower_statements(&mut self, statements: Vec<TypedStatement>) {
        let expressions = statements
            .into_iter()
            .filter_map(|statement| self.add_placeholder_assignment(statement, None))
            .collect::<Vec<_>>();
        for (placeholder, expression) in expressions {
            self.perform_assignment(placeholder, expression);
        }
    }
    fn lower_program(&mut self, program: TypedProgram) -> IntermediateProgram {
        self.lower_statements(program.statements);
        let main = self.scope[&(program.main.variable, Vec::new())].clone();
        IntermediateProgram {
            statements: self.remove_wasted_allocations_from_statements(self.statements.clone()),
            main: self.remove_wasted_allocations_from_value(main),
            types: self.type_defs.values().cloned().collect(),
        }
    }
    pub fn lower(program: TypedProgram) -> IntermediateProgram {
        let mut lowerer = Lowerer::new();
        lowerer.lower_program(program)
    }
}

#[cfg(test)]
mod tests {

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
            let value: IntermediateAssignment = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(Vec::new())).into();
            (value.location.clone().into(), vec![value.into()])
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
            let value: IntermediateAssignment = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                vec![
                    IntermediateBuiltIn::Integer(Integer { value: 3 }).into(),
                    IntermediateBuiltIn::Boolean(Boolean { value: false }).into(),
                ]
            )).into();
            (value.location.clone().into(), vec![value.into()])
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
            let inner1: IntermediateAssignment = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(Vec::new()).into()).into();
            let inner3: IntermediateAssignment = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                vec![
                    IntermediateBuiltIn::Boolean(Boolean { value: true }).into(),
                ]
            )).into();
            let outer: IntermediateAssignment = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                vec![
                    inner1.location.clone().into(),
                    IntermediateBuiltIn::Integer(Integer { value: 1 }).into(),
                    inner3.location.clone().into(),
                ]
            )).into();
            (outer.location.clone().into(), vec![inner1.into(), inner3.into(), outer.into()])
        };
        "nested tuple"
    )]
    #[test_case(
        TypedFunctionCall{
            function: Box::new(
                TypedAccess {
                    variable: TypedVariable {
                        variable: DEFAULT_CONTEXT.with(|context| context.get(&Id::from("+")).unwrap().variable.clone()),
                        type_: Type::from(TypeFn(vec![TYPE_INT, TYPE_INT], Box::new(TYPE_INT))).into(),
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
            let memory: IntermediateAssignment = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: BuiltInFn(
                        Id::from("+"),
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
            (memory.location.clone().into(), vec![memory.into()])
        };
        "operator call"
    )]
    #[test_case(
        {
            let parameters = vec![
                TYPE_INT.into(),
                TYPE_BOOL.into()
            ];
            TypedLambdaDef{
                parameters: parameters.clone(),
                return_type: Box::new(TYPE_INT),
                body: TypedBlock{
                    statements: Vec::new(),
                    expression: Box::new(TypedAccess{
                        variable: parameters[0].clone().into(),
                        parameters: Vec::new()
                    }.into())
                }
            }.into()
        },
        {
            let args = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::BOOL).into(),
            ];
            let memory: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: args.clone(),
                statements: Vec::new(),
                ret: (args[0].clone().into(), AtomicTypeEnum::INT.into())
            }).into();
            (memory.location.clone().into(), vec![memory.into()])
        };
        "projection fn def"
    )]
    #[test_case(
        {
            let arg: TypedVariable = Type::from(TypeTuple(vec![TYPE_INT, TYPE_BOOL])).into();
            TypedLambdaDef{
                parameters: vec![arg.clone()],
                return_type: Box::new(TYPE_BOOL),
                body: TypedBlock{
                    statements: Vec::new(),
                    expression: Box::new(TypedElementAccess{
                        expression: Box::new(TypedAccess{
                            variable: arg.into(),
                            parameters: Vec::new()
                        }.into()),
                        index: 1
                    }.into())
                }
            }.into()
        },
        {
            let arg: IntermediateArg = IntermediateType::from(IntermediateTupleType(vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::BOOL).into()
            ])).into();
            let result: IntermediateAssignment = IntermediateExpression::from(IntermediateElementAccess{
                value: arg.clone().into(),
                idx: 1
            }).into();
            let memory: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: vec![arg.clone()],
                statements: vec![
                    result.clone().into()
                ],
                ret: (
                    result.location.into(),
                    AtomicTypeEnum::BOOL.into()
                )
            }).into();
            (memory.location.clone().into(), vec![memory.into()])
        };
        "element access"
    )]
    #[test_case(
        {
            let parameters = vec![
                Type::from(TypeFn(
                    vec![
                        TYPE_INT,
                    ],
                    Box::new(TYPE_INT)
                )).into(),
                TYPE_INT.into(),
            ];
            let y: TypedVariable = TYPE_INT.into();
            let z: TypedVariable = TYPE_INT.into();
            TypedLambdaDef{
                parameters: parameters.clone(),
                return_type: Box::new(TYPE_INT),
                body: TypedBlock{
                    statements: vec![
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
                        }.into(),
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
                        }.into()
                    ],
                    expression: Box::new(TypedAccess{
                        variable: z.clone(),
                        parameters: Vec::new()
                    }.into())
                }
            }.into()
        },
        {
            let args: Vec<IntermediateArg> = vec![
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            let call1: IntermediateAssignment = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: args[0].clone().into(),
                args: vec![args[1].clone().into()]
            }).into();
            let call2: IntermediateAssignment = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: args[0].clone().into(),
                args: vec![call1.location.clone().into()]
            }).into();
            let fn_def: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: args.clone(),
                statements: vec![
                    call1.into(),
                    call2.clone().into(),
                ],
                ret: (call2.location.into(), AtomicTypeEnum::INT.into())
            }).into();
            (fn_def.location.clone().into(), vec![fn_def.into()])
        };
        "double apply fn def"
    )]
    #[test_case(
        TypedConstructorCall{
            idx: 0,
            output_type: Type::from(TypeUnion{
                id: Id::from("Bull"),
                variants: vec![
                    None,
                    None
                ],
            }),
            arguments: Vec::new()
        }.into(),
        {
            let memory: IntermediateAssignment = IntermediateExpression::from(
                IntermediateCtorCall{
                    idx: 0,
                    data: None,
                    type_: IntermediateUnionType(vec![None, None]).into()
                }
            ).into();
            (
                memory.location.clone().into(),
                vec![memory.into()]
            )
        };
        "data-free constructor"
    )]
    #[test_case(
        TypedConstructorCall{
            idx: 1,
            output_type: Type::from(TypeUnion{
                id: Id::from("Option_Int"),
                variants: vec![
                    None,
                    Some(TYPE_INT),
                ],
            }),
            arguments: vec![
                Integer{value: 8}.into()
            ]
        }.into(),
        {
            let memory: IntermediateAssignment = IntermediateExpression::from(
                IntermediateCtorCall{
                    idx: 1,
                    data: Some(IntermediateBuiltIn::from(Integer{value: 8}).into()),
                    type_: IntermediateUnionType(vec![None, Some(AtomicTypeEnum::INT.into())]).into()
                }
            ).into();
            (
                memory.location.clone().into(),
                vec![memory.into()]
            )
        };
        "data-value constructor"
    )]
    #[test_case(
        {
            let reference = Rc::new(RefCell::new(ParametricType::new()));
            let list_int_type = Type::from(TypeUnion{
                id: Id::from("list_int"),
                variants: vec![
                    Some(Type::from(TypeTuple(vec![
                        TYPE_INT,
                        Type::from(TypeInstantiation{reference: Rc::clone(&reference), instances: Vec::new()}),
                    ]))),
                    None,
                ]
            });
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
                    IntermediateType::Reference(reference.clone().into())
                ]).into()),
                None
            ]);
            let list_int_type = IntermediateType::from(union_type.clone());
            *reference.borrow_mut() = list_int_type.clone();
            let nil: IntermediateAssignment = IntermediateExpression::from(
                IntermediateCtorCall{
                    idx: 0,
                    data: None,
                    type_: union_type.clone()
                }
            ).into();
            let tuple: IntermediateAssignment = IntermediateExpression::from(
                IntermediateTupleExpression(
                    vec![
                        IntermediateBuiltIn::from(Integer{value: -8}).into(),
                        nil.location.clone().into()
                    ]
                )
            ).into();
            let head: IntermediateAssignment = IntermediateExpression::from(
                IntermediateCtorCall{
                    idx: 1,
                    data: Some(tuple.location.clone().into()),
                    type_: union_type
                }
            ).into();
            (
                head.location.clone().into(),
                vec![
                    nil.into(),
                    tuple.into(),
                    head.into()
                ]
            )
        };
        "recursive constructor"
    )]
    #[test_case(
        {
            let arg = TypedVariable::from(TYPE_BOOL);
            TypedLambdaDef{
                parameters: vec![arg.clone()],
                body: TypedBlock{
                    statements: Vec::new(),
                    expression: Box::new(TypedIf{
                        condition: Box::new(
                            TypedAccess{
                                variable: arg.into(),
                                parameters: Vec::new()
                            }.into(),
                        ),
                        true_block: TypedBlock {
                            statements: Vec::new(),
                            expression: Box::new(
                                Integer{
                                    value: 1
                                }.into()
                            )
                        },
                        false_block: TypedBlock {
                            statements: Vec::new(),
                            expression: Box::new(
                                Integer{
                                    value: 0
                                }.into()
                            )
                        },
                    }.into())
                },
                return_type: Box::new(TYPE_INT)
            }.into()
        },
        {
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            let return_address: IntermediateAssignment = IntermediateValue::from(IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT))).into();
            let memory: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: vec![arg.clone()],
                statements: vec![
                    IntermediateIfStatement{
                        condition: arg.into(),
                        branches: (
                            vec![
                                IntermediateAssignment{
                                    location: return_address.location.clone(),
                                    expression: IntermediateBuiltIn::from(Integer{value: 1}).into()
                                }.into(),
                            ],
                            vec![
                                IntermediateAssignment{
                                    location: return_address.location.clone(),
                                    expression: IntermediateBuiltIn::from(Integer{value: 0}).into()
                                }.into()
                            ]
                        )
                    }.into()
                ],
                ret: (return_address.location.clone().into(), AtomicTypeEnum::INT.into())
            }).into();
            (
                memory.location.clone().into(),
                vec![memory.into()]
            )
        };
        "if statement"
    )]
    #[test_case(
        {
            let arg = TypedVariable::from(TYPE_INT);
            let y = TypedVariable::from(TYPE_INT);
            TypedLambdaDef{
                parameters: vec![arg.clone()],
                body: TypedBlock{
                    statements: vec![
                        TypedAssignment{
                            variable: y.clone(),
                            expression: TypedExpression::from(TypedFunctionCall{
                                function: Box::new(
                                    TypedAccess {
                                        variable: TypedVariable {
                                            variable: DEFAULT_CONTEXT.with(|context| context.get(&Id::from("++")).unwrap().variable.clone()),
                                            type_: Type::from(TypeFn(vec![TYPE_INT], Box::new(TYPE_INT))).into(),
                                        },
                                        parameters :Vec::new()
                                    }.into()
                                ),
                                arguments: vec![
                                    TypedAccess{
                                        variable: arg.clone().into(),
                                        parameters: Vec::new()
                                    }.into()
                                ]
                            }).into()
                        }.into()
                    ],
                    expression: Box::new(TypedIf{
                        condition: Box::new(
                            TypedFunctionCall{
                                function: Box::new(
                                    TypedAccess {
                                        variable: TypedVariable {
                                            variable: DEFAULT_CONTEXT.with(|context| context.get(&Id::from(">")).unwrap().variable.clone()),
                                            type_: Type::from(TypeFn(vec![TYPE_INT,TYPE_INT], Box::new(TYPE_BOOL))).into(),
                                        },
                                        parameters :Vec::new()
                                    }.into()
                                ),
                                arguments: vec![
                                    TypedAccess{
                                        variable: y.clone().into(),
                                        parameters: Vec::new()
                                    }.into(),
                                    Integer{
                                        value: 0
                                    }.into()
                                ]
                            }.into()
                        ),
                        true_block: TypedBlock {
                            statements: Vec::new(),
                            expression: Box::new(
                                TypedFunctionCall{
                                    function: Box::new(
                                        TypedAccess {
                                            variable: TypedVariable {
                                                variable: DEFAULT_CONTEXT.with(|context| context.get(&Id::from("++")).unwrap().variable.clone()),
                                                type_: Type::from(TypeFn(vec![TYPE_INT], Box::new(TYPE_INT))).into(),
                                            },
                                            parameters :Vec::new()
                                        }.into()
                                    ),
                                    arguments: vec![
                                        TypedAccess{
                                            variable: y.clone().into(),
                                            parameters: Vec::new()
                                        }.into()
                                    ]
                                }.into()
                            )
                        },
                        false_block: TypedBlock {
                            statements: Vec::new(),
                            expression: Box::new(
                                TypedFunctionCall{
                                    function: Box::new(
                                        TypedAccess {
                                            variable: TypedVariable {
                                                variable: DEFAULT_CONTEXT.with(|context| context.get(&Id::from("++")).unwrap().variable.clone()),
                                                type_: Type::from(TypeFn(vec![TYPE_INT], Box::new(TYPE_INT))).into(),
                                            },
                                            parameters :Vec::new()
                                        }.into()
                                    ),
                                    arguments: vec![
                                        TypedAccess{
                                            variable: arg.clone().into(),
                                            parameters: Vec::new()
                                        }.into()
                                    ]
                                }.into()
                            )
                        },
                    }.into())
                },
                return_type: Box::new(TYPE_INT)
            }.into()
        },
        {
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let return_address: IntermediateAssignment = IntermediateValue::from(IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT))).into();
            let y: IntermediateAssignment = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: BuiltInFn(
                        Id::from("++"),
                        IntermediateFnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ).into(),
                args: vec![
                    arg.clone().into()
                ]
            }).into();
            let c: IntermediateAssignment = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: BuiltInFn(
                        Id::from(">"),
                        IntermediateFnType(
                            vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::BOOL.into())
                        ).into()
                    ).into(),
                args: vec![
                    y.location.clone().into(),
                    IntermediateBuiltIn::from(Integer{value: 0}).into()
                ]
            }).into();
            let z: IntermediateAssignment = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: BuiltInFn(
                        Id::from("++"),
                        IntermediateFnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ).into(),
                args: vec![
                    y.location.clone().into()
                ]
            }).into();
            let memory: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: vec![arg.clone()],
                statements: vec![
                    y.clone().into(),
                    c.clone().into(),
                    IntermediateIfStatement{
                        condition: c.location.into(),
                        branches: (
                            vec![
                                z.clone().into(),
                                IntermediateAssignment{
                                    location: return_address.location.clone(),
                                    expression: IntermediateValue::from(z.location).into()
                                }.into(),
                            ],
                            vec![
                                IntermediateAssignment{
                                    location: return_address.location.clone(),
                                    expression: IntermediateValue::from(y.location).into()
                                }.into()
                            ]
                        )
                    }.into()
                ],
                ret: (return_address.location.clone().into(), AtomicTypeEnum::INT.into())
            }).into();
            (
                memory.location.clone().into(),
                vec![memory.into()]
            )
        };
        "if statement using scope"
    )]
    #[test_case(
        {
            let arg = TypedVariable::from(Type::from(TypeUnion{id: Id::from("Bull"),variants: vec![None,None]}));
            TypedLambdaDef{
                parameters: vec![arg.clone()],
                body: TypedBlock{
                    statements: Vec::new(),
                    expression: Box::new(TypedMatch{
                        subject: Box::new(
                            TypedAccess{
                                variable: arg.into(),
                                parameters: Vec::new()
                            }.into(),
                        ),
                        blocks: vec![
                            TypedMatchBlock{
                                matches: vec![
                                    TypedMatchItem {
                                        type_idx: 0,
                                        assignee: None
                                    }
                                ],
                                block: TypedBlock {
                                    statements: Vec::new(),
                                    expression: Box::new(
                                        Integer{
                                            value: 1
                                        }.into()
                                    )
                                }
                            },
                            TypedMatchBlock{
                                matches: vec![
                                    TypedMatchItem {
                                        type_idx: 1,
                                        assignee: None
                                    }
                                ],
                                block: TypedBlock {
                                    statements: Vec::new(),
                                    expression: Box::new(
                                        Integer{
                                            value: 0
                                        }.into()
                                    )
                                }
                            }
                        ],
                    }.into())
                },
                return_type: Box::new(TYPE_INT)
            }.into()
        },
        {
            let arg: IntermediateArg = IntermediateType::from(IntermediateUnionType(vec![None,None])).into();
            let return_address: IntermediateAssignment = IntermediateValue::from(IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT))).into();
            let memory: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: vec![arg.clone()],
                statements: vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: return_address.location.clone(),
                                        expression: IntermediateBuiltIn::from(Integer{value: 1}).into()
                                    }.into(),
                                ]
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: return_address.location.clone(),
                                        expression: IntermediateBuiltIn::from(Integer{value: 0}).into()
                                    }.into(),
                                ]
                            },
                        ]
                    }.into()
                ],
                ret: (return_address.location.clone().into(), AtomicTypeEnum::INT.into())
            }).into();
            (
                memory.location.clone().into(),
                vec![memory.into()]
            )
        };
        "match statement no values"
    )]
    #[test_case(
        {
            let arg = TypedVariable::from(Type::from(TypeUnion{id: Id::from("Either"),variants: vec![Some(TYPE_INT),Some(TYPE_INT)]}));
            let var = TypedVariable::from(TYPE_INT);
            TypedLambdaDef{
                parameters: vec![arg.clone()],
                body: TypedBlock{
                    statements: Vec::new(),
                    expression: Box::new(TypedMatch{
                        subject: Box::new(
                            TypedAccess{
                                variable: arg.into(),
                                parameters: Vec::new()
                            }.into(),
                        ),
                        blocks: vec![
                            TypedMatchBlock{
                                matches: vec![
                                    TypedMatchItem {
                                        type_idx: 0,
                                        assignee: Some(var.clone())
                                    },
                                    TypedMatchItem {
                                        type_idx: 1,
                                        assignee: Some(var.clone())
                                    }
                                ],
                                block: TypedBlock {
                                    statements: Vec::new(),
                                    expression: Box::new(
                                        TypedAccess {
                                            variable: var.into(),
                                            parameters: Vec::new()
                                        }.into()
                                    )
                                }
                            },
                        ],
                    }.into())
                },
                return_type: Box::new(TYPE_INT)
            }.into()
        },
        {
            let arg: IntermediateArg = IntermediateType::from(IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into())])).into();
            let return_address: IntermediateAssignment = IntermediateValue::from(IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT))).into();
            let var: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let memory: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: vec![arg.clone()],
                statements: vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(var.clone()),
                                statements: vec![
                                    IntermediateAssignment{
                                        location: return_address.location.clone(),
                                        expression: var.clone().into()
                                    }.into(),
                                ]
                            },
                            IntermediateMatchBranch{
                                target: Some(var.clone()),
                                statements: vec![
                                    IntermediateAssignment{
                                        location: return_address.location.clone(),
                                        expression: var.clone().into()
                                    }.into(),
                                ]
                            },
                        ]
                    }.into()
                ],
                ret: (return_address.location.clone().into(), AtomicTypeEnum::INT.into())
            }).into();
            (
                memory.location.clone().into(),
                vec![memory.into()]
            )
        };
        "match statement same value"
    )]
    #[test_case(
        {
            let arg = TypedVariable::from(Type::from(TypeUnion{id: Id::from("Option"),variants: vec![Some(TYPE_INT),None]}));
            let var = TypedVariable::from(TYPE_INT);
            TypedLambdaDef{
                parameters: vec![arg.clone()],
                body: TypedBlock{
                    statements: Vec::new(),
                    expression: Box::new(TypedMatch{
                        subject: Box::new(
                            TypedAccess{
                                variable: arg.into(),
                                parameters: Vec::new()
                            }.into(),
                        ),
                        blocks: vec![
                            TypedMatchBlock{
                                matches: vec![
                                    TypedMatchItem {
                                        type_idx: 1,
                                        assignee: None
                                    },
                                ],
                                block: TypedBlock {
                                    statements: Vec::new(),
                                    expression: Box::new(
                                        Integer{value: 0}.into()
                                    )
                                }
                            },
                            TypedMatchBlock{
                                matches: vec![
                                    TypedMatchItem {
                                        type_idx: 0,
                                        assignee: Some(var.clone())
                                    },
                                ],
                                block: TypedBlock {
                                    statements: Vec::new(),
                                    expression: Box::new(
                                        TypedAccess {
                                            variable: var.into(),
                                            parameters: Vec::new()
                                        }.into()
                                    )
                                }
                            },
                        ],
                    }.into())
                },
                return_type: Box::new(TYPE_INT)
            }.into()
        },
        {
            let arg: IntermediateArg = IntermediateType::from(IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),None])).into();
            let return_address: IntermediateAssignment = IntermediateValue::from(IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT))).into();
            let var: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let memory: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: vec![arg.clone()],
                statements: vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(var.clone()),
                                statements: vec![
                                    IntermediateAssignment{
                                        location: return_address.location.clone(),
                                        expression: var.clone().into()
                                    }.into(),
                                ]
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: return_address.location.clone(),
                                        expression: IntermediateBuiltIn::from(Integer{value: 0}).into()
                                    }.into(),
                                ]
                            },
                        ]
                    }.into()
                ],
                ret: (return_address.location.clone().into(), AtomicTypeEnum::INT.into())
            }).into();
            (
                memory.location.clone().into(),
                vec![memory.into()]
            )
        };
        "match statement value and default"
    )]
    fn test_lower_expression(
        expression: TypedExpression,
        value_statements: (IntermediateValue, Vec<IntermediateStatement>),
    ) {
        let (value, statements) = value_statements;
        let expected_fn: IntermediateExpression = IntermediateLambda {
            args: Vec::new(),
            statements,
            ret: (value, IntermediateTupleType(Vec::new()).into()),
        }
        .into();
        let mut lowerer = Lowerer::new();
        let value = lowerer.lower_expression(expression);
        let efficient_value = lowerer.remove_wasted_allocations_from_value(value);
        let efficient_statements =
            lowerer.remove_wasted_allocations_from_statements(lowerer.statements.clone());
        let efficient_fn = IntermediateLambda {
            args: Vec::new(),
            statements: efficient_statements,
            ret: (efficient_value, IntermediateTupleType(Vec::new()).into()),
        };
        assert!(ExpressionEqualityChecker::equal(
            &expected_fn,
            &efficient_fn.into()
        ))
    }

    #[test]
    fn test_projection_equalities() {
        let p0 = {
            let args = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: args.clone(),
                statements: Vec::new(),
                ret: (args[0].clone().into(), AtomicTypeEnum::INT.into()),
            })
        };
        let p1 = {
            let args = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: args.clone(),
                statements: Vec::new(),
                ret: (args[1].clone().into(), AtomicTypeEnum::INT.into()),
            })
        };
        let q0 = {
            let args = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: args.clone(),
                statements: Vec::new(),
                ret: (args[0].clone().into(), AtomicTypeEnum::INT.into()),
            })
        };
        let q1 = {
            let args = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: args.clone(),
                statements: Vec::new(),
                ret: (args[1].clone().into(), AtomicTypeEnum::INT.into()),
            })
        };

        assert!(ExpressionEqualityChecker::equal(&p0, &q0));
        assert!(ExpressionEqualityChecker::equal(&p1, &q1));
        assert!(!ExpressionEqualityChecker::equal(&p0, &p1));
        assert!(!ExpressionEqualityChecker::equal(&q0, &q1));
        assert!(!ExpressionEqualityChecker::equal(&p0, &q1));
        assert!(!ExpressionEqualityChecker::equal(&p1, &q0));
    }

    #[test_case(
        Type::from(TypeAtomic(AtomicTypeEnum::INT)),
        |_| AtomicTypeEnum::INT.into();
        "int"
    )]
    #[test_case(
        Type::from(TypeAtomic(AtomicTypeEnum::BOOL)),
        |_| AtomicTypeEnum::BOOL.into();
        "bool"
    )]
    #[test_case(
        Type::from(TypeTuple(Vec::new())),
        |_| IntermediateTupleType(Vec::new()).into();
        "empty tuple"
    )]
    #[test_case(
        Type::from(TypeTuple(vec![
            Type::from(TypeAtomic(AtomicTypeEnum::INT)),
            Type::from(TypeAtomic(AtomicTypeEnum::BOOL)),
        ])),
        |_| IntermediateTupleType(vec![
            AtomicTypeEnum::INT.into(),
            AtomicTypeEnum::BOOL.into(),
        ]).into();
        "flat tuple"
    )]
    #[test_case(
        Type::from(TypeTuple(vec![
            Type::from(TypeTuple(vec![
                Type::from(TypeAtomic(AtomicTypeEnum::INT)),
                Type::from(TypeAtomic(AtomicTypeEnum::BOOL)),
            ])),
            Type::from(TypeTuple(Vec::new())),
        ])),
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
        Type::from(TypeUnion{id: Id::from("Bull"),variants:  vec![None, None]}),
        |_| {
            IntermediateUnionType(vec![None, None]).into()
        };
        "bull correct"
    )]
    #[test_case(
        Type::from(TypeUnion{
            id: Id::from("LR"),
            variants: vec![
                Some(TYPE_INT),
                Some(TYPE_BOOL),
            ]
        }),
        |_| {
            IntermediateUnionType(vec![
                Some(AtomicTypeEnum::INT.into()),
                Some(AtomicTypeEnum::BOOL.into()),
            ]).into()
        };
        "left right"
    )]
    #[test_case(
        Type::from(TypeFn(
            Vec::new(),
            Box::new(Type::from(TypeTuple(Vec::new())))
        )),
        |_| {
            IntermediateFnType(
                Vec::new(),
                Box::new(IntermediateTupleType(Vec::new()).into())
            ).into()
        };
        "unit function"
    )]
    #[test_case(
        Type::from(TypeFn(
            vec![
                TYPE_INT,
                TYPE_INT,
            ],
            Box::new(TYPE_INT)
        )),
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
                type_: Type::from(TypeFn(
                    vec![
                        Type::from(TypeVariable(parameter.clone())),
                    ],
                    Box::new(Type::from(TypeVariable(parameter))),
                ))
            }));
            Type::from(TypeInstantiation{reference: type_, instances: vec![TYPE_INT]})
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
        Type::from(TypeFn(
            vec![
                Type::from(TypeFn(
                    vec![
                        TYPE_INT,
                    ],
                    Box::new(TYPE_BOOL)
                )),
                TYPE_INT,
            ],
            Box::new(TYPE_BOOL)
        )),
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
            let union_type = Type::from(TypeUnion{
                id: Id::from("nat"),
                variants: vec![
                    Some(Type::from(TypeInstantiation{reference: Rc::clone(&reference), instances: Vec::new()})),
                    None,
                ]}
            );
            *reference.borrow_mut() = union_type.clone().into();
            union_type
        },
        |type_defs| {
            assert_eq!(type_defs.len(), 1);
            let reference = type_defs.values().cloned().collect::<Vec<_>>()[0].clone();
            IntermediateUnionType(vec![
                Some(IntermediateType::Reference(reference.into())),
                None
            ]).into()
        };
        "nat"
    )]
    #[test_case(
        {
            let reference = Rc::new(RefCell::new(ParametricType::new()));
            let union_type = Type::from(TypeUnion{
                id: Id::from("list_int"),
                variants: vec![
                    Some(Type::from(TypeTuple(vec![
                        TYPE_INT,
                        Type::from(TypeInstantiation{reference: Rc::clone(&reference), instances: Vec::new()}),
                    ]))),
                    None,
                ]
            });
            *reference.borrow_mut() = union_type.clone().into();
            union_type
        },
        |type_defs| {
            assert_eq!(type_defs.len(), 1);
            let reference = type_defs.values().cloned().collect::<Vec<_>>()[0].clone();
            IntermediateUnionType(vec![
                Some(IntermediateTupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    IntermediateType::Reference(reference.into())
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
                type_: Type::from(TypeTuple(parameters.iter().map(|parameter| Type::from(TypeVariable(parameter.clone()))).collect())),
            }));
            Type::from(TypeInstantiation{reference: pair, instances: vec![TYPE_INT, TYPE_BOOL]})
        },
        |_| IntermediateTupleType(vec![
            AtomicTypeEnum::INT.into(),
            AtomicTypeEnum::BOOL.into(),
        ]).into();
        "instantiated pair int bool"
    )]
    #[test_case(
        {
            let parameter = Rc::new(RefCell::new(None));
            let type_ = Rc::new(RefCell::new(ParametricType {
                parameters: vec![parameter.clone()],
                type_: Type::from(TypeVariable(parameter)),
            }));
            Type::from(TypeInstantiation{reference: type_, instances: vec![TYPE_INT]})
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
            Type::from(TypeInstantiation{reference: list_type.clone(), instances: vec![TYPE_INT]})
        },
        |type_defs| {
            assert_eq!(type_defs.len(), 1);
            let reference = type_defs.values().cloned().collect::<Vec<_>>()[0].clone();
            IntermediateUnionType(vec![
                Some(IntermediateTupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    IntermediateType::Reference(reference.into())
                ]).into()),
                None
            ]).into()
        };
        "instantiated list int"
    )]
    #[test_case(
        {
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
            Type::from(TypeTuple(vec![
                Type::from(TypeInstantiation{reference: list_type.clone(), instances: vec![TYPE_BOOL]}),
                Type::from(TypeInstantiation{reference: list_type.clone(), instances: vec![TYPE_BOOL]}),
            ]))
        },
        |type_defs| {
            assert_eq!(type_defs.len(), 1);
            let reference = type_defs.values().cloned().collect::<Vec<_>>()[0].clone();
            IntermediateTupleType(vec![
                IntermediateUnionType(vec![
                    Some(IntermediateTupleType(vec![
                        AtomicTypeEnum::BOOL.into(),
                        IntermediateType::Reference(reference.clone().into())
                    ]).into()),
                    None
                ]).into(),
                IntermediateUnionType(vec![
                    Some(IntermediateTupleType(vec![
                        AtomicTypeEnum::BOOL.into(),
                        IntermediateType::Reference(reference.into())
                    ]).into()),
                    None
                ]).into()
            ]).into()
        };
        "instantiated list bool tuple"
    )]
    fn test_lower_type(type_: Type, expected_gen: impl Fn(&TypeDefs) -> IntermediateType) {
        let mut lowerer = Lowerer::new();
        let type_ = lowerer.lower_type(&type_);
        let expected = expected_gen(&lowerer.type_defs);
        assert_eq!(type_, expected);
    }

    #[ignore]
    #[test]
    fn test_blowup_type() {
        let parameter = Rc::new(RefCell::new(None));
        let blowup_type = Rc::new(RefCell::new(ParametricType {
            parameters: vec![parameter.clone()],
            type_: Type::new(),
        }));
        blowup_type.borrow_mut().type_ = Type::from(TypeUnion {
            id: Id::from("List"),
            variants: vec![Some(Type::from(TypeInstantiation {
                reference: blowup_type.clone(),
                instances: vec![Type::from(TypeTuple(vec![
                    Type::from(TypeVariable(parameter.clone())),
                    Type::from(TypeVariable(parameter.clone())),
                ]))],
            }))],
        });
        let type_ = Type::from(TypeInstantiation {
            reference: blowup_type.clone(),
            instances: vec![TYPE_INT],
        });

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
        list_type.borrow_mut().type_ = Type::from(TypeUnion {
            id: Id::from("List"),
            variants: vec![
                Some(Type::from(TypeTuple(vec![
                    Type::from(TypeVariable(parameter.clone())),
                    Type::from(TypeInstantiation {
                        reference: list_type.clone(),
                        instances: vec![Type::from(TypeVariable(parameter.clone()))],
                    }),
                ]))),
                None,
            ],
        });
        let list_bool = Type::from(TypeInstantiation {
            reference: list_type.clone(),
            instances: vec![TYPE_BOOL],
        });
        let list_int = Type::from(TypeInstantiation {
            reference: list_type.clone(),
            instances: vec![TYPE_INT],
        });
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
                    }.into()
                ],
                vec![
                    (
                        x.variable,
                        IntermediateBuiltIn::Integer(Integer { value: 5 }).into(),
                    )
                ]
            )
        };
        "simple assignment"
    )]
    #[test_case(
        {
            let x: TypedVariable = Type::from(TypeTuple(vec![TYPE_INT, TYPE_BOOL])).into();
            let value = IntermediateTupleExpression(
                vec![
                    IntermediateBuiltIn::Integer(Integer { value: 3 }).into(),
                    IntermediateBuiltIn::Boolean(Boolean { value: false }).into(),
                ]
            ).into();
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
                    }.into()
                ],
                vec![
                    (
                        x.variable,
                        value
                    )
                ]
            )
        };
        "compound assignment"
    )]
    #[test_case(
        {
            let x: TypedVariable = TYPE_INT.into();
            let y: TypedVariable = TYPE_BOOL.into();
            let value = IntermediateTupleExpression(
                vec![
                    IntermediateBuiltIn::Integer(Integer { value: 3 }).into(),
                    IntermediateBuiltIn::Boolean(Boolean { value: false }).into(),
                ]
            ).into();
            (
                vec![
                    TypedAssignment {
                        variable: x.clone(),
                        expression: TypedExpression::from(Integer { value: 3 }).into()
                    }.into(),
                    TypedAssignment {
                        variable: y.clone(),
                        expression: TypedExpression::from(TypedTuple{
                            expressions: vec![
                                TypedAccess{
                                    variable: x.clone(),
                                    parameters: Vec::new()
                                }.into(),
                                Boolean{value: false}.into()
                            ]
                        }).into(),
                    }.into()
                ],
                vec![
                    (
                        x.variable,
                        IntermediateBuiltIn::Integer(Integer { value: 3 }).into(),
                    ),
                    (
                        y.variable,
                        value
                    )
                ]
            )
        };
        "dual assignment"
    )]
    #[test_case(
        {
            let f: TypedVariable = Type::from(TypeFn(vec![TYPE_INT], Box::new(TYPE_INT))).into();
            let argument: TypedVariable = TYPE_INT.into();
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let fn_ = IntermediateLambda{
                args: vec![arg.clone()],
                statements: Vec::new(),
                ret: (arg.clone().into(), AtomicTypeEnum::INT.into())
            }.into();
            (
                vec![
                    TypedFnDef {
                        variable: f.clone(),
                        parameters: Vec::new(),
                        fn_: TypedLambdaDef{
                            parameters: vec![argument.clone()],
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock{
                                statements: Vec::new(),
                                expression: Box::new(TypedAccess{
                                    variable: argument.clone().into(),
                                    parameters: Vec::new()
                                }.into())
                            }
                        }.into()
                    }.into()
                ],
                vec![
                    (
                        f.variable,
                        fn_
                    )
                ]
            )
        };
        "simple fn def"
    )]
    #[test_case(
        {
            let f: TypedVariable = Type::from(TypeFn(Vec::new(), Box::new(TYPE_INT))).into();
            let y: TypedVariable = TYPE_INT.into();
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let fn_: IntermediateAssignment = IntermediateExpression::from(IntermediateLambda{
                args: vec![arg.clone()],
                statements: Vec::new(),
                ret: (arg.clone().into(), AtomicTypeEnum::INT.into())
            }).into();
            let value = IntermediateFnCall{
                fn_: fn_.location.clone().into(),
                args: vec![IntermediateBuiltIn::Integer(Integer { value: 11 }).into()]
            }.into();
            let parameter: TypedVariable = TYPE_INT.into();
            (
                vec![
                    TypedFnDef {
                        variable: f.clone(),
                        parameters: Vec::new(),
                        fn_: TypedLambdaDef{
                            parameters: vec![parameter.clone()],
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock{
                                statements: Vec::new(),
                                expression: Box::new(TypedAccess{
                                    variable: parameter.clone().into(),
                                    parameters: Vec::new()
                                }.into())
                            }
                        }.into()
                    }.into(),
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
                    }.into()
                ],
                vec![
                    (
                        f.variable,
                        fn_.expression
                    ),
                    (
                        y.variable,
                        value
                    )
                ]
            )
        };
        "user-defined fn call"
    )]
    #[test_case(
        {
            let foo: TypedVariable = Type::from(TypeFn(Vec::new(), Box::new(TYPE_INT))).into();
            let y: TypedVariable = TYPE_INT.into();
            let mut fn_: IntermediateAssignment = IntermediateValue::from(IntermediateArg::from(
                IntermediateType::from(IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())))
            )).into();
            let recursive_call: IntermediateAssignment = IntermediateExpression::IntermediateFnCall(
                IntermediateFnCall{
                    fn_: fn_.location.clone().into(),
                    args: Vec::new()
                }
            ).into();
            fn_.expression = IntermediateExpression::from(IntermediateLambda{
                args: Vec::new(),
                statements: vec![
                    recursive_call.clone().into()
                ],
                ret: (recursive_call.location.into(), AtomicTypeEnum::INT.into())
            }).into();
            let value = IntermediateFnCall{
                fn_: fn_.location.clone().into(),
                args: Vec::new()
            }.into();
            (
                vec![
                    TypedFnDef {
                        variable: foo.clone(),
                        parameters: Vec::new(),
                        fn_: TypedLambdaDef{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock{
                                statements: Vec::new(),
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
                        }
                    }.into(),
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
                    }.into()
                ],
                vec![
                    (
                        foo.variable,
                        fn_.expression
                    ),
                    (
                        y.variable,
                        value
                    )
                ]
            )
        };
        "recursive fn call"
    )]
    #[test_case(
        {
            let a: TypedVariable = Type::from(TypeFn(Vec::new(), Box::new(TYPE_BOOL))).into();
            let b: TypedVariable = Type::from(TypeFn(Vec::new(), Box::new(TYPE_BOOL))).into();
            let mut a_fn: IntermediateAssignment = IntermediateValue::from(IntermediateArg::from(
                IntermediateType::from(IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::BOOL.into())))
            )).into();
            let mut b_fn: IntermediateAssignment = IntermediateValue::from(IntermediateArg::from(
                IntermediateType::from(IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::BOOL.into())))
            )).into();
            let a_call: IntermediateAssignment = IntermediateExpression::IntermediateFnCall(
                IntermediateFnCall{
                    fn_: a_fn.location.clone().into(),
                    args: Vec::new()
                }
            ).into();
            let b_call: IntermediateAssignment = IntermediateExpression::IntermediateFnCall(
                IntermediateFnCall{
                    fn_: b_fn.location.clone().into(),
                    args: Vec::new()
                }
            ).into();
            a_fn.expression = IntermediateExpression::IntermediateLambda(IntermediateLambda{
                args: Vec::new(),
                statements: vec![
                    b_call.clone().into()
                ],
                ret: (b_call.location.into(), AtomicTypeEnum::BOOL.into())
            }).into();
            b_fn.expression = IntermediateExpression::IntermediateLambda(IntermediateLambda{
                args: Vec::new(),
                statements: vec![
                    a_call.clone().into()
                ],
                ret: (a_call.location.into(), AtomicTypeEnum::BOOL.into())
            }).into();
            (
                vec![
                    TypedFnDef {
                        variable: a.clone(),
                        parameters: Vec::new(),
                        fn_: TypedLambdaDef{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_BOOL),
                            body: TypedBlock{
                                statements: Vec::new(),
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
                        }.into()
                    }.into(),
                    TypedFnDef {
                        variable: b.clone(),
                        parameters: Vec::new(),
                        fn_: TypedLambdaDef{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_BOOL),
                            body: TypedBlock{
                                statements: Vec::new(),
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
                        }.into()
                    }.into(),
                ],
                vec![
                    (
                        a.variable,
                        a_fn.expression
                    ),
                    (
                        b.variable,
                        b_fn.expression
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
            type_: Type::from(TypeFn(
                vec![
                    Type::from(TypeVariable(parameter.clone())),
                ],
                Box::new(Type::from(TypeVariable(parameter.clone()))),
            ))
        };
        let id: TypedVariable = id_type.clone().into();
        let id_int: TypedVariable = id_type.instantiate(&vec![TYPE_INT]).into();
        let id_bool: TypedVariable = id_type.instantiate(&vec![TYPE_BOOL]).into();
        let id_bool2: TypedVariable = id_type.instantiate(&vec![TYPE_BOOL]).into();
        let x = TypedVariable {
            variable: Variable::new(),
            type_: Type::from(TypeVariable(parameter.clone())).into(),
        };
        let int_arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
        let bool_arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
        let id_int_fn: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
            args: vec![int_arg.clone()],
            statements: Vec::new(),
            ret: (int_arg.into(), AtomicTypeEnum::INT.into())
        }).into();
        let id_bool_fn: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
            args: vec![bool_arg.clone()],
            statements: Vec::new(),
            ret: (bool_arg.into(), AtomicTypeEnum::BOOL.into())
        }).into();
        (
            vec![
                TypedAssignment{
                    variable: id.clone(),
                    expression: ParametricExpression {
                        expression: TypedLambdaDef{
                            parameters: vec![x.clone()],
                            return_type: Box::new(TypeVariable(parameter.clone()).into()),
                            body: TypedBlock{
                                statements: Vec::new(),
                                expression: Box::new(TypedAccess{
                                    variable: x.clone(),
                                    parameters: Vec::new()
                                }.into())
                            }
                        }.into(),
                        parameters: vec![(String::from("T"),parameter.clone())]
                    },
                }.into(),
                TypedAssignment{
                    variable: id_int.clone(),
                    expression: ParametricExpression{
                        expression: TypedAccess{
                            variable: id.clone(),
                            parameters: vec![TYPE_INT]
                        }.into(),
                        parameters: Vec::new()
                    }
                }.into(),
                TypedAssignment{
                    variable: id_bool.clone(),
                    expression: ParametricExpression{
                        expression: TypedAccess{
                            variable: id.clone(),
                            parameters: vec![TYPE_BOOL]
                        }.into(),
                        parameters: Vec::new()
                    }
                }.into(),
                TypedAssignment{
                    variable: id_bool2.clone(),
                    expression: ParametricExpression{
                        expression: TypedAccess{
                            variable: id.clone(),
                            parameters: vec![TYPE_BOOL]
                        }.into(),
                        parameters: Vec::new()
                    }
                }.into()
            ],
            vec![
                (
                    id_int.variable,
                    id_int_fn.expression
                ),
                (
                    id_bool.variable,
                    id_bool_fn.expression.clone()
                ),
                (
                    id_bool2.variable,
                    id_bool_fn.expression
                ),
            ]
        )
        };
        "parametric identity lambda"
    )]
    #[test_case(
        {
        let parameter = Rc::new(RefCell::new(None));
        let id_type = ParametricType {
            parameters: vec![parameter.clone()],
            type_: Type::from(TypeFn(
                vec![
                    Type::from(TypeVariable(parameter.clone())),
                ],
                Box::new(Type::from(TypeVariable(parameter.clone()))),
            ))
        };
        let id: TypedVariable = id_type.clone().into();
        let id_int: TypedVariable = id_type.instantiate(&vec![TYPE_INT]).into();
        let id_bool: TypedVariable = id_type.instantiate(&vec![TYPE_BOOL]).into();
        let id_bool2: TypedVariable = id_type.instantiate(&vec![TYPE_BOOL]).into();
        let x = TypedVariable {
            variable: Variable::new(),
            type_: Type::from(TypeVariable(parameter.clone())).into(),
        };
        let int_arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
        let bool_arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
        let id_int_fn: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
            args: vec![int_arg.clone()],
            statements: Vec::new(),
            ret: (int_arg.into(), AtomicTypeEnum::INT.into())
        }).into();
        let id_bool_fn: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
            args: vec![bool_arg.clone()],
            statements: Vec::new(),
            ret: (bool_arg.into(), AtomicTypeEnum::BOOL.into())
        }).into();
        (
            vec![
                TypedFnDef{
                    variable: id.clone(),
                    fn_: TypedLambdaDef{
                        parameters: vec![x.clone()],
                        return_type: Box::new(TypeVariable(parameter.clone()).into()),
                        body: TypedBlock{
                            statements: Vec::new(),
                            expression: Box::new(TypedAccess{
                                variable: x.clone(),
                                parameters: Vec::new()
                            }.into())
                        }
                    },
                    parameters: vec![(String::from("T"),parameter.clone())]
                }.into(),
                TypedAssignment{
                    variable: id_int.clone(),
                    expression: ParametricExpression{
                        expression: TypedAccess{
                            variable: id.clone(),
                            parameters: vec![TYPE_INT]
                        }.into(),
                        parameters: Vec::new()
                    }
                }.into(),
                TypedAssignment{
                    variable: id_bool.clone(),
                    expression: ParametricExpression{
                        expression: TypedAccess{
                            variable: id.clone(),
                            parameters: vec![TYPE_BOOL]
                        }.into(),
                        parameters: Vec::new()
                    }
                }.into(),
                TypedAssignment{
                    variable: id_bool2.clone(),
                    expression: ParametricExpression{
                        expression: TypedAccess{
                            variable: id.clone(),
                            parameters: vec![TYPE_BOOL]
                        }.into(),
                        parameters: Vec::new()
                    }
                }.into()
            ],
            vec![
                (
                    id_int.variable,
                    id_int_fn.expression
                ),
                (
                    id_bool.variable,
                    id_bool_fn.expression.clone()
                ),
                (
                    id_bool2.variable,
                    id_bool_fn.expression
                ),
            ]
        )
        };
        "parametric identity fn def"
    )]
    fn test_lower_statements(
        statements_scope: (Vec<TypedStatement>, Vec<(Variable, IntermediateExpression)>),
    ) {
        let (statements, expected_scope) = statements_scope;
        let mut lowerer = Lowerer::new();
        lowerer.lower_statements(statements);
        lowerer.statements =
            lowerer.remove_wasted_allocations_from_statements(lowerer.statements.clone());
        let flat_scope: HashMap<(Variable, Vec<Type>), IntermediateValue> = lowerer
            .scope
            .clone()
            .into_iter()
            .map(|(k, v)| (k, v.clone()))
            .collect::<HashMap<_, _>>();
        let mut tuples = (Vec::new(), Vec::new());
        for (k, e) in expected_scope {
            let value =
                lowerer.remove_wasted_allocations_from_value(flat_scope[&(k, Vec::new())].clone());
            let expression = match value {
                IntermediateValue::IntermediateMemory(location) => lowerer
                    .remove_wasted_allocations_from_expression(
                        lowerer.memory[&location][0].clone(),
                    ),
                v => v.into(),
            };
            dbg!(&expression, &e);
            assert!(ExpressionEqualityChecker::equal(&expression, &e));
            tuples.0.push(expression);
            tuples.1.push(e);
        }
        let transform = |expressions: Vec<IntermediateExpression>| {
            let mut statements = Vec::new();
            let mut values = Vec::new();
            for expression in expressions {
                let assignment: IntermediateAssignment = expression.into();
                values.push(assignment.location.clone().into());
                statements.push(assignment.into());
            }
            let assignment: IntermediateAssignment =
                IntermediateExpression::from(IntermediateTupleExpression(values)).into();
            let value = assignment.location.clone().into();
            statements.push(assignment.into());

            IntermediateLambda {
                args: Vec::new(),
                statements,
                ret: (value, IntermediateTupleType(Vec::new()).into()),
            }
        };
        assert!(ExpressionEqualityChecker::equal(
            &transform(tuples.0).into(),
            &transform(tuples.1).into()
        ))
    }

    #[test_case(
        {
            let main: TypedVariable = ParametricType {
                type_: Type::from(TypeFn(Vec::new(), Box::new(TYPE_INT))),
                parameters: Vec::new()
            }.into();
            TypedProgram {
                type_definitions: TypeDefinitions::new(),
                statements: vec![
                    TypedAssignment{
                        variable: main.clone(),
                        expression: TypedExpression::from(TypedLambdaDef{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock {
                                statements: Vec::new(),
                                expression: Box::new(Integer{value:0}.into())
                            }
                        }).into()
                    }.into()
                ],
                main
            }
        },
        {
            let main: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: (IntermediateBuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into())
            }).into();
            IntermediateProgram{
                statements: vec![
                    main.clone().into()
                ],
                types: Vec::new(),
                main: main.location.into()
            }
        };
        "return 0"
    )]
    #[test_case(
        {
            let main: TypedVariable = ParametricType {
                type_: Type::from(TypeFn(Vec::new(), Box::new(TYPE_INT))),
                parameters: Vec::new()
            }.into();
            let parameter = Rc::new(RefCell::new(None));
            let type_definitions:TypeDefinitions = [(
                Id::from("Option"),
                ParametricType {
                    parameters: vec![parameter.clone()],
                    type_: Type::from(TypeUnion{
                        id: Id::from("Option"),
                        variants: vec![
                            Some(Type::from(TypeVariable(parameter))),
                            None
                        ]
                    })
                }
            )].into();
            let var: TypedVariable = ParametricType {
                type_: TYPE_INT,
                parameters: Vec::new()
            }.into();
            let x: TypedVariable = ParametricType {
                type_: TYPE_INT,
                parameters: Vec::new()
            }.into();
            TypedProgram {
                type_definitions: type_definitions.clone(),
                statements: vec![
                    TypedAssignment {
                        expression: ParametricExpression{
                            parameters: Vec::new(),
                            expression: TypedMatch{
                                subject: Box::new(
                                    TypedConstructorCall{
                                        idx: 1,
                                        output_type: Type::from(TypeInstantiation{reference: type_definitions.get(&Id::from("Option")).unwrap().clone(), instances: vec![TYPE_INT]}),
                                        arguments: vec![Integer{value:1}.into()]
                                    }.into(),
                                ),
                                blocks: vec![
                                    TypedMatchBlock{
                                        matches: vec![
                                            TypedMatchItem {
                                                type_idx: 1,
                                                assignee: None
                                            },
                                        ],
                                        block: TypedBlock {
                                            statements: Vec::new(),
                                            expression: Box::new(
                                                Integer{value: 0}.into()
                                            )
                                        }
                                    },
                                    TypedMatchBlock{
                                        matches: vec![
                                            TypedMatchItem {
                                                type_idx: 0,
                                                assignee: Some(var.clone())
                                            },
                                        ],
                                        block: TypedBlock {
                                            statements: Vec::new(),
                                            expression: Box::new(
                                                TypedAccess {
                                                    variable: var.into(),
                                                    parameters: Vec::new()
                                                }.into()
                                            )
                                        }
                                    },
                                ],
                            }.into()
                        },
                        variable: x.clone()
                    }.into(),
                    TypedAssignment{
                        variable: main.clone(),
                        expression: TypedExpression::from(TypedLambdaDef{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock {
                                statements: Vec::new(),
                                expression: Box::new(TypedAccess{
                                    variable: x,
                                    parameters: Vec::new(),
                                }.into())
                            }
                        }).into()
                    }.into()
                ],
                main
            }
        },
        {
            let ctor: IntermediateAssignment = IntermediateExpression::from(IntermediateCtorCall{
                idx: 1,
                data: Some(IntermediateBuiltIn::from(Integer{value: 1}).into()),
                type_: IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),None])
            }).into();
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let location = Location::new();
            let main: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: (location.clone().into(), AtomicTypeEnum::INT.into())
            }).into();
            IntermediateProgram{
                statements: vec![
                    ctor.clone().into(),
                    IntermediateMatchStatement{
                        subject: ctor.location.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(arg.clone()),
                                statements: vec![
                                    IntermediateAssignment{
                                        location: location.clone(),
                                        expression: arg.clone().into()
                                    }.into(),
                                ]
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment{
                                        location: location.clone(),
                                        expression: IntermediateBuiltIn::from(Integer{value: 0}).into()
                                    }.into(),
                                ]
                            },
                        ]
                    }.into(),
                    main.clone().into()
                ],
                main: main.location.into(),
                types: vec![
                    Rc::new(RefCell::new(IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),None]).into()))
                ]
            }
        };
        "union type usage"
    )]
    #[test_case(
        {
            let main: TypedVariable = ParametricType {
                type_: Type::from(TypeFn(Vec::new(), Box::new(TYPE_INT))),
                parameters: Vec::new()
            }.into();
            let parameter = Rc::new(RefCell::new(None));
            let type_variable = Type::from(TypeVariable(parameter.clone()));
            let arg: TypedVariable = ParametricType{
                parameters: Vec::new(),
                type_: type_variable.clone()
            }.into();
            let id: TypedVariable = ParametricType{
                parameters: vec![parameter.clone()],
                type_: Type::from(TypeFn(vec![type_variable.clone()],Box::new(type_variable.clone())))
            }.into();
            TypedProgram {
                type_definitions: TypeDefinitions::new(),
                statements: vec![
                    TypedAssignment {
                        expression: ParametricExpression{
                            parameters: vec![(Id::from("T"), parameter.clone())],
                            expression: TypedLambdaDef{
                                parameters: vec![
                                    arg.clone()
                                ],
                                return_type: Box::new(type_variable.clone()),
                                body: TypedBlock {
                                    statements: Vec::new(),
                                    expression: Box::new(TypedAccess{
                                        variable: arg,
                                        parameters: Vec::new()
                                    }.into())
                                }
                            }.into()
                        },
                        variable: id.clone()
                    }.into(),
                    TypedAssignment{
                        variable: main.clone(),
                        expression: TypedExpression::from(TypedLambdaDef{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock {
                                statements: Vec::new(),
                                expression: Box::new(
                                    TypedFunctionCall{
                                        function: Box::new(TypedAccess{
                                            variable: id,
                                            parameters: vec![TYPE_INT],
                                        }.into()),
                                        arguments: vec![
                                            Integer{value: 0}.into()
                                        ]
                                    }.into()
                                )
                            }
                        }).into()
                    }.into()
                ],
                main
            }
        },
        {
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let id_int: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: vec![arg.clone()],
                statements: Vec::new(),
                ret: (arg.into(), AtomicTypeEnum::INT.into())
            }).into();
            let fn_call: IntermediateAssignment = IntermediateExpression::from(IntermediateFnCall{
                args: vec![IntermediateBuiltIn::from(Integer{value: 0}).into()],
                fn_: id_int.location.clone().into()
            }).into();
            let main: IntermediateAssignment = IntermediateExpression::IntermediateLambda(IntermediateLambda {
                args: Vec::new(),
                statements: vec![
                    id_int.into(),
                    fn_call.clone().into()
                ],
                ret: (fn_call.location.into(), AtomicTypeEnum::INT.into())
            }).into();
            IntermediateProgram{
                statements: vec![
                    main.clone().into()
                ],
                types: Vec::new(),
                main: main.location.into()
            }
        };
        "parametric variable"
    )]
    fn test_lower_program(program: TypedProgram, expected: IntermediateProgram) {
        let mut lowerer = Lowerer::new();
        let lower_program = lowerer.lower_program(program);
        let lower_fn: IntermediateExpression = IntermediateLambda {
            args: Vec::new(),
            statements: lower_program.statements,
            ret: (lower_program.main, IntermediateTupleType(Vec::new()).into()),
        }
        .into();
        let expected_fn: IntermediateExpression = IntermediateLambda {
            args: Vec::new(),
            statements: expected.statements,
            ret: (expected.main, IntermediateTupleType(Vec::new()).into()),
        }
        .into();
        assert!(ExpressionEqualityChecker::equal(&lower_fn, &expected_fn));
        assert_eq!(lower_program.types, expected.types)
    }
}
