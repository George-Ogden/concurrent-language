use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    rc::Rc,
};

use crate::{
    intermediate_nodes::*, AllocationState, Assignment, AtomicType, Await, BuiltIn,
    ClosureInstantiation, ConstructorCall, Declaration, ElementAccess, Expression, FnCall, FnDef,
    FnType, Id, IfStatement, MachineType, MatchBranch, MatchStatement, Memory, Name, Statement,
    TupleExpression, TupleType, TypeDef, UnionType, Value,
};
use itertools::{zip_eq, Either, Itertools};
use once_cell::sync::Lazy;
use type_checker::*;

type Scope = HashMap<(Variable, Vec<Type>), IntermediateMemory>;
type History = HashMap<IntermediateExpression, IntermediateValue>;
type Uninstantiated = HashMap<Variable, ParametricExpression>;
type TypeDefs = HashMap<Type, Rc<RefCell<IntermediateType>>>;
type VisitedReferences = HashSet<*mut IntermediateType>;
type MemoryMap = HashMap<Location, Vec<Rc<RefCell<IntermediateExpression>>>>;
type ReferenceNames = HashMap<*mut IntermediateType, MachineType>;
type MemoryIds = HashMap<Location, Memory>;
type ArgIds = HashMap<*mut IntermediateType, Memory>;
type ValueScope = HashMap<IntermediateValue, Value>;
type TypeLookup = HashMap<IntermediateUnionType, UnionType>;
type FnDefs = Vec<FnDef>;

const OPERATOR_NAMES: Lazy<HashMap<Name, Name>> = Lazy::new(|| {
    HashMap::from_iter(
        [
            ("+", "Plus__BuiltIn"),
            ("-", "Minus__BuiltIn"),
            ("*", "Multiply__BuiltIn"),
            ("/", "Divide__BuiltIn"),
            ("**", "Exponentiate__BuiltIn"),
            ("%", "Modulo__BuiltIn"),
            ("<<", "Left_Shift__BuiltIn"),
            (">>", "Right_Shift__BuiltIn"),
            ("<=>", "Spaceship__BuiltIn"),
            ("&", "Bitwise_And__BuiltIn"),
            ("|", "Bitwise_Or__BuiltIn"),
            ("^", "Bitwise_Xor__BuiltIn"),
            ("++", "Increment__BuiltIn"),
            ("--", "Decrement__BuiltIn"),
            ("<", "Comparison_LT__BuiltIn"),
            ("<=", "Comparison_LE__BuiltIn"),
            (">", "Comparison_GT__BuiltIn"),
            (">=", "Comparison_GE__BuiltIn"),
            ("==", "Comparison_EQ__BuiltIn"),
            ("!=", "Comparison_NE__BuiltIn"),
        ]
        .into_iter()
        .map(|(op, name)| (Name::from(op), Name::from(name))),
    )
});

struct Lowerer {
    scope: Scope,
    history: History,
    uninstantiated: Uninstantiated,
    type_defs: TypeDefs,
    statements: Vec<IntermediateStatement>,
    visited_references: VisitedReferences,
    memory: MemoryMap,
    reference_names: ReferenceNames,
    memory_ids: MemoryIds,
    arg_ids: ArgIds,
    lazy_vals: ValueScope,
    non_lazy_vals: ValueScope,
    type_lookup: TypeLookup,
    fn_defs: FnDefs,
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
            memory: MemoryMap::new(),
            reference_names: ReferenceNames::new(),
            memory_ids: MemoryIds::new(),
            arg_ids: ArgIds::new(),
            lazy_vals: ValueScope::new(),
            non_lazy_vals: ValueScope::new(),
            type_lookup: TypeLookup::new(),
            fn_defs: FnDefs::new(),
        };
        let scope = DEFAULT_CONTEXT.with(|context| {
            Scope::from_iter(context.iter().map(|(id, var)| {
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
        for memory in scope.values() {
            lowerer.update_memory(&memory);
        }
        lowerer.scope = scope;
        lowerer
    }
    fn update_memory(&mut self, memory: &IntermediateMemory) {
        let values = self
            .memory
            .entry(memory.location.clone())
            .or_insert(Vec::new());
        values.push(memory.expression.clone());
    }
    fn get_cached_value(
        &mut self,
        intermediate_expression: IntermediateExpression,
    ) -> IntermediateValue {
        if let Some(cached) = self.history.get(&intermediate_expression) {
            return cached.clone();
        }
        let memory: IntermediateMemory = intermediate_expression.clone().into();
        self.update_memory(&memory);
        self.statements
            .push(IntermediateStatement::Assignment(memory.clone()));
        let value: IntermediateValue = memory.location.into();
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
            TypedExpression::TypedFunctionDefinition(fn_def) => self.lower_fn_def(fn_def),
            TypedExpression::TypedConstructorCall(ctor_call) => self.lower_ctor_call(ctor_call),
            TypedExpression::TypedIf(if_) => self.lower_if(if_),
            TypedExpression::TypedMatch(match_) => self.lower_match(match_),
            TypedExpression::PartiallyTypedFunctionDefinition(_) => {
                panic!("All function definitions should be fully typed.")
            }
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
            let (expression, placeholder) = self
                .add_placeholder_assignment(
                    TypedAssignment {
                        variable: TypedVariable {
                            variable: variable.variable.clone(),
                            type_: variable.type_,
                        },
                        expression: uninstantiated.clone(),
                    },
                    Some(parameters.clone()),
                )
                .unwrap();
            self.perform_assignment(expression, placeholder);
        };
        self.scope[&(variable.variable, parameters)]
            .clone()
            .location
            .into()
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
        TypedFunctionDefinition {
            parameters,
            body,
            return_type: _,
        }: TypedFunctionDefinition,
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
            let memory = arg.clone().into();
            self.update_memory(&memory);
            self.scope.insert((variable.clone(), Vec::new()), memory);
        }
        let (statements, return_value) = self.lower_block(body, false);
        let intermediate_expression = IntermediateFnDef {
            args: args,
            statements: statements,
            return_value: return_value,
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
                let memory = arg.clone().into();
                self.update_memory(&memory);
                self.scope.insert((variable.clone(), Vec::new()), memory);
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
                let memory = IntermediateMemory {
                    expression: Rc::new(RefCell::new(value.into())),
                    location: result_location.clone(),
                };
                self.update_memory(&memory);
                statements.push(memory.into());
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
                IntermediateValue::IntermediateArg(arg) => {
                    self.remove_wasted_allocations_from_expression(arg.into())
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
                args,
                statements,
                return_value,
            }) => IntermediateFnDef {
                args,
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
            IntermediateValue::IntermediateArg(arg) => arg.into(),
            IntermediateValue::IntermediateMemory(location) => {
                let expressions = self.memory.get(&location);
                if expressions.map(Vec::len) == Some(1) {
                    let expressions = expressions.unwrap();
                    let expression = expressions[0].clone();
                    match expression.clone().borrow().clone() {
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
            IntermediateStatement::Assignment(assignment) => {
                let IntermediateMemory {
                    expression,
                    location,
                } = assignment;
                if matches!(
                    &*expression.clone().borrow(),
                    IntermediateExpression::IntermediateValue(_)
                ) && self.memory.get(&location).map(Vec::len) == Some(1)
                {
                    return None;
                }
                let condensed_expression = self
                    .remove_wasted_allocations_from_expression(expression.clone().borrow().clone());
                *expression.borrow_mut() = condensed_expression;
                Some(IntermediateStatement::Assignment(
                    IntermediateMemory {
                        location,
                        expression: expression.clone(),
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
                            IntermediateType::Reference(occupied_entry.get().clone())
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
                        self.type_defs[&instantiation].borrow().clone()
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
        let placeholder: IntermediateMemory = IntermediateArg::from(lower_type).into();
        self.scope
            .insert((variable.variable.clone(), parameters), placeholder.clone());
        Some((expression, placeholder))
    }
    fn perform_assignment(&mut self, expression: TypedExpression, placeholder: IntermediateMemory) {
        let value = self.lower_expression(expression);
        *placeholder.expression.borrow_mut() = value.into();
        self.update_memory(&placeholder);
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
    fn lower_program(&mut self, program: TypedProgram) -> IntermediateProgram {
        self.lower_assignments(program.assignments);
        let main = self.scope[&(program.main.variable, Vec::new())]
            .clone()
            .location
            .into();
        IntermediateProgram {
            statements: self.remove_wasted_allocations_from_statements(self.statements.clone()),
            main: self.remove_wasted_allocations_from_value(main),
        }
    }

    fn expression_type(&self, expression: &IntermediateExpression) -> IntermediateType {
        match expression {
            IntermediateExpression::IntermediateValue(value) => self.value_type(value),
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                idx: _,
                data: _,
                type_,
            }) => type_.clone().into(),
            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args,
                statements: _,
                return_value,
            }) => IntermediateFnType(
                args.iter()
                    .map(|arg| self.value_type(&arg.clone().into()))
                    .collect(),
                Box::new(self.value_type(return_value)),
            )
            .into(),
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args: _ }) => {
                let IntermediateType::IntermediateFnType(IntermediateFnType(_, return_type)) =
                    self.value_type(fn_)
                else {
                    panic!("Calling function with non-function type.")
                };
                *return_type
            }
            IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                values,
            )) => {
                IntermediateTupleType(values.iter().map(|value| self.value_type(value)).collect())
                    .into()
            }
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx,
            }) => {
                let IntermediateType::IntermediateTupleType(IntermediateTupleType(types)) =
                    self.value_type(value)
                else {
                    panic!("Accessing tuple with non-tuple type.")
                };
                types[*idx].clone()
            }
        }
    }
    fn value_type(&self, value: &IntermediateValue) -> IntermediateType {
        match value {
            IntermediateValue::IntermediateBuiltIn(IntermediateBuiltIn::Integer(_)) => {
                AtomicTypeEnum::INT.into()
            }
            IntermediateValue::IntermediateBuiltIn(IntermediateBuiltIn::Boolean(_)) => {
                AtomicTypeEnum::BOOL.into()
            }
            IntermediateValue::IntermediateBuiltIn(IntermediateBuiltIn::BuiltInFn(_, type_)) => {
                type_.clone()
            }
            IntermediateValue::IntermediateMemory(location) => {
                let expressions = &self.memory[&location];
                let types = expressions
                    .iter()
                    .map(|expression| self.expression_type(&expression.borrow().clone()));
                if !types.clone().all_equal() {
                    panic!("Expressions have different types.");
                }
                types.collect_vec().first().unwrap().clone()
            }
            IntermediateValue::IntermediateArg(IntermediateArg(rc)) => rc.borrow().clone(),
        }
    }

    fn compile_type(&self, type_: &IntermediateType) -> MachineType {
        match type_ {
            IntermediateType::AtomicType(AtomicType(atomic_type_enum)) => {
                atomic_type_enum.clone().into()
            }
            IntermediateType::IntermediateTupleType(IntermediateTupleType(types)) => {
                TupleType(self.compile_types(types)).into()
            }
            IntermediateType::IntermediateFnType(IntermediateFnType(arg_types, ret_type)) => {
                FnType(
                    self.compile_types(arg_types)
                        .into_iter()
                        .map(|type_| MachineType::Lazy(Box::new(type_)))
                        .collect(),
                    Box::new(MachineType::Lazy(Box::new(self.compile_type(&*ret_type)))),
                )
                .into()
            }
            IntermediateType::IntermediateUnionType(union_type) => {
                self.type_lookup[union_type].clone().into()
            }
            IntermediateType::Reference(reference) => {
                match self.reference_names.get(&reference.as_ptr()) {
                    Some(type_) => MachineType::Reference(Box::new(type_.clone())),
                    None => self.compile_type(&reference.borrow().clone()),
                }
            }
        }
    }
    fn compile_types(&self, types: &Vec<IntermediateType>) -> Vec<MachineType> {
        types.iter().map(|type_| self.compile_type(type_)).collect()
    }
    fn compile_type_defs(&mut self, types: Vec<Rc<RefCell<IntermediateType>>>) -> Vec<TypeDef> {
        let types = types
            .into_iter()
            .filter_map(|type_| {
                let IntermediateType::IntermediateUnionType(union_type) = type_.borrow().clone()
                else {
                    return None;
                };
                Some((type_.as_ptr(), union_type))
            })
            .collect_vec();
        for (i, (ptr, _)) in types.iter().enumerate() {
            self.reference_names
                .insert(*ptr, MachineType::NamedType(format!("T{i}")));
        }
        types
            .into_iter()
            .enumerate()
            .map(|(i, (_, IntermediateUnionType(types)))| {
                let union_type = IntermediateUnionType(types.clone());
                let constructors = types
                    .into_iter()
                    .enumerate()
                    .map(|(j, type_)| {
                        (
                            format!("T{i}C{j}"),
                            type_.as_ref().map(|type_| self.compile_type(type_)),
                        )
                    })
                    .collect_vec();
                self.type_lookup.insert(
                    union_type,
                    UnionType(constructors.iter().map(|(name, _)| name.clone()).collect()),
                );
                TypeDef {
                    name: format!("T{i}"),
                    constructors,
                }
            })
            .collect_vec()
    }

    fn next_memory_address(&self) -> Memory {
        Memory(format!("m{}", self.memory_ids.len()))
    }
    fn compile_location(&mut self, location: Location) -> Memory {
        if !self.memory_ids.contains_key(&location) {
            self.memory_ids
                .insert(location.clone(), self.next_memory_address());
        }
        self.memory_ids[&location].clone().into()
    }
    fn compile_arg(&mut self, arg: &IntermediateArg) -> Memory {
        let p = arg.0.as_ptr();
        if !self.arg_ids.contains_key(&p) {
            self.arg_ids.insert(p, self.next_arg_id());
        }
        let memory: Memory = self.arg_ids[&p].clone().into();
        self.lazy_vals
            .insert(arg.clone().into(), memory.clone().into());
        memory
    }
    fn compile_args(&mut self, args: &Vec<IntermediateArg>) -> Vec<Memory> {
        args.iter().map(|arg| self.compile_arg(arg)).collect()
    }
    fn next_arg_id(&self) -> Memory {
        Memory(format!("a{}", self.arg_ids.len()))
    }
    fn new_memory_location(&mut self) -> Memory {
        let mut boxes: Vec<Location> = Vec::new();
        while match boxes.last() {
            None => true,
            Some(x) => self.memory_ids.contains_key(&x),
        } {
            boxes.push(Location::new());
        }
        let last = boxes.last().unwrap();
        let memory = self.next_memory_address();
        self.memory_ids.insert(last.clone(), memory.clone());
        memory
    }
    fn next_fn_name(&self) -> Name {
        format!("F{}", self.fn_defs.len())
    }
    fn compile_lazy_value(&mut self, value: IntermediateValue) -> (Vec<Statement>, Value) {
        match self.lazy_vals.get(&value) {
            Some(value) => (Vec::new(), value.clone()),
            None => {
                let type_ = self.compile_type(&self.value_type(&value));
                let (mut statements, non_lazy_val) = self.compile_value(value.clone(), false);
                let memory = self.new_memory_location();
                statements.push(
                    Declaration {
                        type_: MachineType::Lazy(Box::new(type_.clone())),
                        memory: memory.clone(),
                    }
                    .into(),
                );
                statements.push(
                    Assignment {
                        check_null: false,
                        target: memory.clone(),
                        value: Expression::Wrap(non_lazy_val, type_),
                    }
                    .into(),
                );
                self.lazy_vals.insert(value, memory.clone().into());
                (statements, memory.into())
            }
        }
    }
    fn compile_value(&mut self, value: IntermediateValue, lazy: bool) -> (Vec<Statement>, Value) {
        match &value {
            IntermediateValue::IntermediateArg(IntermediateArg(reference)) => {
                if lazy {
                    (Vec::new(), self.arg_ids[&reference.as_ptr()].clone().into())
                } else {
                    match self.non_lazy_vals.get(&value) {
                        Some(value) => (Vec::new(), value.clone()),
                        None => {
                            let type_ = self.compile_type(&self.value_type(&value));
                            let (mut statements, lazy_val) =
                                self.compile_value(value.clone(), true);
                            let lazy_mem = match &lazy_val {
                                Value::BuiltIn(_) => panic!("Built-in values cannot be lazy."),
                                Value::Memory(memory) => memory.clone(),
                            };
                            statements.push(Await(vec![lazy_mem]).into());
                            let memory = self.new_memory_location();
                            statements.push(
                                Declaration {
                                    type_,
                                    memory: memory.clone(),
                                }
                                .into(),
                            );
                            statements.push(
                                Assignment {
                                    check_null: false,
                                    target: memory.clone(),
                                    value: Expression::Unwrap(lazy_val),
                                }
                                .into(),
                            );
                            self.non_lazy_vals.insert(value, memory.clone().into());
                            (statements, memory.into())
                        }
                    }
                }
            }
            IntermediateValue::IntermediateMemory(location) => {
                if lazy {
                    self.compile_lazy_value(value)
                } else {
                    match self.lazy_vals.get(&value) {
                        None => (Vec::new(), self.compile_location(location.clone()).into()),
                        Some(lazy_val) => match self.non_lazy_vals.get(&value) {
                            Some(val) => (Vec::new(), val.clone()),
                            None => {
                                let Value::Memory(lazy_mem) = lazy_val.clone() else {
                                    panic!("Memory converted to non-memory.")
                                };
                                let mem = self.new_memory_location();
                                self.non_lazy_vals
                                    .insert(location.clone().into(), mem.clone().into());
                                (
                                    vec![
                                        Await(vec![lazy_mem.clone()]).into(),
                                        Declaration {
                                            type_: self.compile_type(&self.value_type(&value)),
                                            memory: mem.clone(),
                                        }
                                        .into(),
                                        Assignment {
                                            target: mem.clone(),
                                            value: Expression::Unwrap(lazy_mem.clone().into()),
                                            check_null: false,
                                        }
                                        .into(),
                                    ],
                                    mem.into(),
                                )
                            }
                        },
                    }
                }
            }
            IntermediateValue::IntermediateBuiltIn(built_in) => {
                if lazy {
                    self.compile_lazy_value(value)
                } else {
                    (
                        Vec::new(),
                        Value::from(match built_in {
                            IntermediateBuiltIn::Boolean(boolean) => BuiltIn::from(boolean.clone()),
                            IntermediateBuiltIn::Integer(integer) => BuiltIn::from(integer.clone()),
                            IntermediateBuiltIn::BuiltInFn(name, _) => {
                                BuiltIn::BuiltInFn(OPERATOR_NAMES[name].clone()).into()
                            }
                        }),
                    )
                }
            }
        }
    }
    fn compile_values(
        &mut self,
        values: Vec<IntermediateValue>,
        lazy: bool,
    ) -> (Vec<Statement>, Vec<Value>) {
        let (statements, values) = values
            .into_iter()
            .map(|value| self.compile_value(value, lazy))
            .collect::<(Vec<Vec<Statement>>, Vec<Value>)>();
        let statements = statements.concat();
        let (awaits, other_statements): (Vec<_>, Vec<_>) =
            statements
                .into_iter()
                .partition_map(|statement| match statement {
                    Statement::Await(Await(vs)) => Either::Left(vs),
                    other => Either::Right(other),
                });
        let mut statements = Vec::new();
        if awaits.len() > 0 {
            statements.push(Await(awaits.concat()).into());
        }
        statements.extend(other_statements);
        (statements, values)
    }
    fn compile_expression(
        &mut self,
        expression: IntermediateExpression,
    ) -> (Vec<Statement>, Expression) {
        match expression {
            IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                values,
            )) => {
                let referenced_value_indices = values
                    .iter()
                    .enumerate()
                    .filter_map(|(i, value)| match self.value_type(value) {
                        IntermediateType::Reference(type_) => Some((i, type_)),
                        _ => None,
                    })
                    .collect_vec();
                let (mut statements, mut values) = self.compile_values(values, false);
                for (i, type_) in referenced_value_indices {
                    let memory = self.new_memory_location();
                    let type_ = self.compile_type(&type_.borrow().clone());
                    statements.push(
                        Declaration {
                            memory: memory.clone(),
                            type_: MachineType::Reference(Box::new(type_.clone())),
                        }
                        .into(),
                    );
                    statements.push(
                        Assignment {
                            target: memory.clone(),
                            check_null: false,
                            value: Expression::Reference(values[i].clone(), type_),
                        }
                        .into(),
                    );
                    values[i] = memory.into();
                }
                (statements, TupleExpression(values).into())
            }
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx,
            }) => {
                let IntermediateType::IntermediateTupleType(IntermediateTupleType(mut types)) =
                    self.value_type(&value)
                else {
                    panic!("Accessing non-tuple type.")
                };
                let type_ = types.remove(idx);
                let (mut statements, value) = self.compile_value(value, false);
                let value = ElementAccess { value, idx }.into();
                let value = if matches!(&type_, IntermediateType::Reference(_)) {
                    let memory = self.new_memory_location();
                    statements.push(
                        Declaration {
                            memory: memory.clone(),
                            type_: self.compile_type(&type_),
                        }
                        .into(),
                    );
                    statements.push(
                        Assignment {
                            target: memory.clone(),
                            value,
                            check_null: false,
                        }
                        .into(),
                    );
                    Expression::Dereference(memory.into())
                } else {
                    value
                };
                (statements, value)
            }
            IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args }) => {
                let MachineType::FnType(fn_type) = self.compile_type(&self.value_type(&fn_)) else {
                    panic!("Function has non-function type.")
                };
                let (fn_statements, fn_value) = self.compile_value(fn_, false);
                let (args_statements, args_values) = self.compile_values(args, true);
                (
                    vec![fn_statements, args_statements].concat(),
                    FnCall {
                        fn_: fn_value,
                        fn_type,
                        args: args_values,
                    }
                    .into(),
                )
            }
            IntermediateExpression::IntermediateCtorCall(IntermediateCtorCall {
                idx,
                data,
                type_,
            }) => {
                let (statements, value) = match data {
                    None => (Vec::new(), None),
                    Some(value) => {
                        let (statements, value) = self.compile_value(value, false);
                        (statements, Some(value))
                    }
                };
                (
                    statements,
                    ConstructorCall {
                        idx,
                        data: value.map(|value| {
                            let MachineType::UnionType(UnionType(variants)) =
                                self.compile_type(&type_.into())
                            else {
                                panic!("Did not compile union type into union type.")
                            };
                            (variants[idx].clone(), value)
                        }),
                    }
                    .into(),
                )
            }
            IntermediateExpression::IntermediateValue(value) => {
                let (statements, value) = self.compile_value(value, false);
                (statements, value.into())
            }
            IntermediateExpression::IntermediateFnDef(fn_def) => {
                let (statements, closure_inst) = self.compile_fn_def(fn_def);
                (statements, closure_inst.into())
            }
        }
    }
    fn update_declarations(
        &mut self,
        statement: Statement,
        declarations: &HashMap<Memory, AllocationState>,
    ) -> Vec<Statement> {
        match statement {
            Statement::Await(await_) => vec![await_.into()],
            Statement::Assignment(Assignment {
                target,
                value,
                check_null,
            }) if matches!(
                declarations.get(&target),
                Some(&AllocationState::Undeclared(_))
            ) && !matches!(&value, Expression::FnCall(_) | Expression::Wrap(_, _)) =>
            {
                let Some(&AllocationState::Undeclared(Some(ref type_))) = declarations.get(&target)
                else {
                    panic!("Untyped undeclared appeared.");
                };
                let temporary_target = self.new_memory_location();
                vec![
                    Declaration {
                        memory: temporary_target.clone(),
                        type_: type_.clone(),
                    }
                    .into(),
                    Assignment {
                        value,
                        target: temporary_target.clone(),
                        check_null,
                    }
                    .into(),
                    Assignment {
                        target,
                        value: Expression::Wrap(temporary_target.into(), type_.clone()),
                        check_null: true,
                    }
                    .into(),
                ]
            }
            Statement::Assignment(assignment) => vec![assignment.into()],
            Statement::Declaration(Declaration { type_: _, memory })
                if declarations.contains_key(&memory) =>
            {
                Vec::new()
            }
            Statement::Declaration(Declaration { type_, memory }) => {
                vec![Declaration { type_, memory }.into()]
            }
            Statement::IfStatement(IfStatement {
                condition,
                branches,
            }) => vec![IfStatement {
                condition,
                branches: (
                    self.update_all_declarations(branches.0, declarations),
                    self.update_all_declarations(branches.1, declarations),
                ),
            }
            .into()],
            Statement::MatchStatement(MatchStatement {
                expression,
                branches,
            }) => vec![MatchStatement {
                expression,
                branches: branches
                    .into_iter()
                    .map(|MatchBranch { target, statements }| MatchBranch {
                        target,
                        statements: self.update_all_declarations(statements, declarations),
                    })
                    .collect_vec(),
            }
            .into()],
        }
    }
    fn update_all_declarations(
        &mut self,
        statements: Vec<Statement>,
        declarations: &HashMap<Memory, AllocationState>,
    ) -> Vec<Statement> {
        statements
            .into_iter()
            .flat_map(|statement| self.update_declarations(statement, declarations))
            .collect()
    }
    fn mark_missing_declarations(
        &mut self,
        shared_declarations: &HashMap<Memory, AllocationState>,
    ) {
        for (memory, state) in shared_declarations {
            if matches!(state, AllocationState::Undeclared(_)) {
                let location = self
                    .memory_ids
                    .iter()
                    .find(|(_, mem)| mem == &memory)
                    .map(|(loc, _)| loc)
                    .unwrap();
                self.non_lazy_vals.remove(&location.clone().into());
                self.lazy_vals
                    .insert(location.clone().into(), memory.clone().into());
            }
        }
    }
    fn compile_if_statement(&mut self, if_statement: IntermediateIfStatement) -> Vec<Statement> {
        let IntermediateIfStatement {
            condition,
            branches: (true_branch, false_branch),
        } = if_statement;
        let (mut statements, condition) = self.compile_value(condition, false);
        let true_branch = self.compile_statements(true_branch);
        let false_branch = self.compile_statements(false_branch);
        let true_declarations = Statement::declarations(&true_branch);
        let false_declarations = Statement::declarations(&false_branch);
        let shared_declarations =
            Statement::merge_declarations_parallel(true_declarations, false_declarations);
        let true_branch = self.update_all_declarations(true_branch, &shared_declarations);
        let false_branch = self.update_all_declarations(false_branch, &shared_declarations);
        self.mark_missing_declarations(&shared_declarations);
        statements.extend(Statement::from_declarations(shared_declarations));
        statements.push(
            IfStatement {
                condition,
                branches: (true_branch, false_branch),
            }
            .into(),
        );
        statements
    }
    fn compile_match_statement(
        &mut self,
        match_statement: IntermediateMatchStatement,
    ) -> Vec<Statement> {
        let IntermediateMatchStatement { subject, branches } = match_statement;
        let type_ = self.value_type(&subject);
        let MachineType::UnionType(union_type) = self.compile_type(&type_) else {
            panic!("Match expression subject has non-union type.")
        };
        let (mut statements, subject) = self.compile_value(subject, false);
        let branches = branches
            .into_iter()
            .map(
                |IntermediateMatchBranch { target, statements }| MatchBranch {
                    target: target.map(|arg| self.compile_arg(&arg)),
                    statements: self.compile_statements(statements),
                },
            )
            .collect_vec();
        let mut shared_declarations = HashMap::new();
        let mut it = branches
            .iter()
            .map(|branch| Statement::declarations(&branch.statements));
        match it.next() {
            None => (),
            Some(first) => {
                shared_declarations = first;
                for declarations in it {
                    shared_declarations =
                        Statement::merge_declarations_parallel(shared_declarations, declarations);
                }
            }
        }
        let branches = branches
            .into_iter()
            .map(|MatchBranch { target, statements }| MatchBranch {
                target,
                statements: self.update_all_declarations(statements, &shared_declarations),
            })
            .collect_vec();
        self.mark_missing_declarations(&shared_declarations);
        statements.extend(Statement::from_declarations(shared_declarations));
        statements.push(
            MatchStatement {
                expression: (subject, union_type),
                branches,
            }
            .into(),
        );
        statements
    }
    fn compile_statement(&mut self, statement: IntermediateStatement) -> Vec<Statement> {
        match statement {
            IntermediateStatement::Assignment(memory) => self.compile_assignment(memory),
            IntermediateStatement::IntermediateIfStatement(if_statement) => {
                self.compile_if_statement(if_statement)
            }
            IntermediateStatement::IntermediateMatchStatement(match_statement) => {
                self.compile_match_statement(match_statement)
            }
        }
    }
    fn compile_assignment(&mut self, assignment: IntermediateMemory) -> Vec<Statement> {
        let IntermediateMemory {
            expression,
            location,
        } = assignment;
        let type_ = self.compile_type(&self.expression_type(&expression.borrow().clone()));
        let (mut statements, value) = self.compile_expression(expression.borrow().clone());
        let memory = self.compile_location(location.clone());
        if matches!(&value, Expression::FnCall(_)) {
            self.lazy_vals
                .insert(location.into(), memory.clone().into());
            statements.push(
                Assignment {
                    target: memory,
                    value,
                    check_null: true,
                }
                .into(),
            );
        } else {
            self.non_lazy_vals
                .insert(location.into(), memory.clone().into());
            statements.push(
                Declaration {
                    memory: memory.clone().into(),
                    type_,
                }
                .into(),
            );
            statements.push(
                Assignment {
                    target: memory,
                    value,
                    check_null: false,
                }
                .into(),
            );
        }
        statements
    }
    fn compile_statements(&mut self, statements: Vec<IntermediateStatement>) -> Vec<Statement> {
        statements
            .into_iter()
            .map(|statement| self.compile_statement(statement))
            .concat()
    }
    fn replace_open_vars(
        &mut self,
        fn_def: &mut IntermediateFnDef,
    ) -> Vec<(IntermediateValue, Location)> {
        let open_vars = fn_def.find_open_vars();
        let new_locations = open_vars.iter().map(|_| Location::new()).collect_vec();
        let substitution = open_vars
            .iter()
            .zip(new_locations.iter())
            .map(|(var, loc)| (var.clone(), loc.clone().into()))
            .collect::<HashMap<_, _>>();
        fn_def.substitute(&substitution);
        open_vars
            .iter()
            .zip(new_locations.iter())
            .map(|(var, loc)| (var.clone(), loc.clone()))
            .collect()
    }
    fn closure_prefix(&mut self, env_types: &Vec<(Location, MachineType)>) -> Vec<Statement> {
        env_types
            .iter()
            .enumerate()
            .flat_map(|(i, (location, type_))| {
                let memory = self.compile_location(location.clone());
                self.lazy_vals
                    .insert(location.clone().into(), memory.clone().into());
                vec![
                    Declaration {
                        memory: memory.clone(),
                        type_: type_.clone(),
                    }
                    .into(),
                    Assignment {
                        target: memory,
                        value: ElementAccess {
                            idx: i,
                            value: Memory(Id::from("env")).into(),
                        }
                        .into(),
                        check_null: false,
                    }
                    .into(),
                ]
            })
            .collect_vec()
    }
    fn compile_fn_def(
        &mut self,
        mut fn_def: IntermediateFnDef,
    ) -> (Vec<Statement>, ClosureInstantiation) {
        let env_mapping = self.replace_open_vars(&mut fn_def);
        let env_types = env_mapping
            .iter()
            .map(|(value, location)| {
                (
                    location.clone(),
                    MachineType::Lazy(Box::new(self.compile_type(&self.value_type(&value)))),
                )
            })
            .collect_vec();

        let IntermediateFnDef {
            args,
            statements,
            return_value,
        } = fn_def;
        let args = args
            .into_iter()
            .map(|arg| {
                (
                    self.compile_arg(&arg),
                    MachineType::Lazy(Box::new(self.compile_type(&self.value_type(&arg.into())))),
                )
            })
            .collect_vec();
        let mut prefix = self.closure_prefix(&env_types);
        let mut statements = self.compile_statements(statements);
        prefix.extend(statements);
        statements = prefix;
        let ret_type =
            MachineType::Lazy(Box::new(self.compile_type(&self.value_type(&return_value))));
        let (extra_statements, ret_val) = self.compile_value(return_value, true);
        statements.extend(extra_statements);
        let declarations = Statement::declarations(&statements);
        let allocations = declarations
            .into_iter()
            .filter_map(|(memory, state)| match state {
                AllocationState::Undeclared(Some(type_)) => Some(Declaration { memory, type_ }),
                AllocationState::Undeclared(None) => None,
                AllocationState::Declared(_) => None,
            })
            .collect();
        let name = self.next_fn_name();
        let env_type: MachineType =
            TupleType(env_types.into_iter().map(|(_, type_)| type_).collect_vec()).into();
        self.fn_defs.push(FnDef {
            name: name.clone(),
            arguments: args,
            statements,
            ret: (ret_val, ret_type),
            env: if env_mapping.len() > 0 {
                Some(env_type.clone())
            } else {
                None
            },
            allocations,
        });

        if env_mapping.len() > 0 {
            let tuple_mem = self.new_memory_location();
            let (statements, values): (Vec<_>, Vec<_>) = env_mapping
                .into_iter()
                .map(|(value, _)| self.compile_value(value, true))
                .collect();
            let mut statements = statements.concat();
            statements.extend([
                Declaration {
                    memory: tuple_mem.clone(),
                    type_: env_type,
                }
                .into(),
                Assignment {
                    target: tuple_mem.clone(),
                    value: TupleExpression(values).into(),
                    check_null: false,
                }
                .into(),
            ]);
            (
                statements,
                ClosureInstantiation {
                    name,
                    env: Some(tuple_mem.into()),
                },
            )
        } else {
            (Vec::new(), ClosureInstantiation { name, env: None })
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        Await, BuiltIn, ClosureInstantiation, FnDef, Id, IfStatement, MatchBranch, MatchStatement,
        Memory, Name, Statement, TupleType, Value,
    };

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
            let value: IntermediateMemory = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
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
            let inner1: IntermediateMemory = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(Vec::new()).into()).into();
            let inner3: IntermediateMemory = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
                vec![
                    IntermediateBuiltIn::Boolean(Boolean { value: true }).into(),
                ]
            )).into();
            let outer: IntermediateMemory = IntermediateExpression::IntermediateTupleExpression(IntermediateTupleExpression(
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
            let args = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::BOOL).into(),
            ];
            let memory: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: args.clone(),
                statements: Vec::new(),
                return_value: args[0].clone().into()
            }).into();
            (memory.location.clone().into(), vec![memory.into()])
        };
        "projection fn def"
    )]
    #[test_case(
        {
            let arg: TypedVariable = Type::Tuple(vec![TYPE_INT, TYPE_BOOL]).into();
            TypedFunctionDefinition{
                parameters: vec![arg.clone()],
                return_type: Box::new(TYPE_BOOL),
                body: TypedBlock{
                    assignments: Vec::new(),
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
            let result: IntermediateMemory = IntermediateExpression::from(IntermediateElementAccess{
                value: arg.clone().into(),
                idx: 1
            }).into();
            let memory: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: vec![arg.clone()],
                statements: vec![
                    result.clone().into()
                ],
                return_value: result.location.into()
            }).into();
            (memory.location.clone().into(), vec![memory.into()])
        };
        "element access"
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
            let args: Vec<IntermediateArg> = vec![
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                )).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            let call1: IntermediateMemory = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: args[0].clone().into(),
                args: vec![args[1].clone().into()]
            }).into();
            let call2: IntermediateMemory = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: args[0].clone().into(),
                args: vec![call1.location.clone().into()]
            }).into();
            let fn_def: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: args.clone(),
                statements: vec![
                    IntermediateStatement::Assignment(call1),
                    IntermediateStatement::Assignment(call2.clone()),
                ],
                return_value: call2.location.into()
            }).into();
            (fn_def.location.clone().into(), vec![fn_def.into()])
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
                memory.location.clone().into(),
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
                memory.location.clone().into(),
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
                    IntermediateType::Reference(reference.clone().into())
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
                        nil.location.clone().into()
                    ]
                )
            ).into();
            let head: IntermediateMemory = IntermediateExpression::from(
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
            TypedFunctionDefinition{
                parameters: vec![arg.clone()],
                body: TypedBlock{
                    assignments: Vec::new(),
                    expression: Box::new(TypedIf{
                        condition: Box::new(
                            TypedAccess{
                                variable: arg.into(),
                                parameters: Vec::new()
                            }.into(),
                        ),
                        true_block: TypedBlock {
                            assignments: Vec::new(),
                            expression: Box::new(
                                Integer{
                                    value: 1
                                }.into()
                            )
                        },
                        false_block: TypedBlock {
                            assignments: Vec::new(),
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
            let return_address: IntermediateMemory = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT)).into();
            let memory: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: vec![arg.clone()],
                statements: vec![
                    IntermediateIfStatement{
                        condition: arg.into(),
                        branches: (
                            vec![
                                IntermediateMemory{
                                    location: return_address.location.clone(),
                                    expression: Rc::new(RefCell::new(IntermediateBuiltIn::from(Integer{value: 1}).into()))
                                }.into(),
                            ],
                            vec![
                                IntermediateMemory{
                                    location: return_address.location.clone(),
                                    expression: Rc::new(RefCell::new(IntermediateBuiltIn::from(Integer{value: 0}).into()))
                                }.into()
                            ]
                        )
                    }.into()
                ],
                return_value: return_address.location.clone().into()
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
            TypedFunctionDefinition{
                parameters: vec![arg.clone()],
                body: TypedBlock{
                    assignments: vec![
                        TypedAssignment{
                            variable: y.clone(),
                            expression: TypedExpression::from(TypedFunctionCall{
                                function: Box::new(
                                    TypedAccess {
                                        variable: TypedVariable {
                                            variable: DEFAULT_CONTEXT.with(|context| context.get(&Id::from("++")).unwrap().variable.clone()),
                                            type_: Type::Function(vec![TYPE_INT], Box::new(TYPE_INT)).into(),
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
                        }
                    ],
                    expression: Box::new(TypedIf{
                        condition: Box::new(
                            TypedFunctionCall{
                                function: Box::new(
                                    TypedAccess {
                                        variable: TypedVariable {
                                            variable: DEFAULT_CONTEXT.with(|context| context.get(&Id::from(">")).unwrap().variable.clone()),
                                            type_: Type::Function(vec![TYPE_INT,TYPE_INT], Box::new(TYPE_BOOL)).into(),
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
                            assignments: Vec::new(),
                            expression: Box::new(
                                TypedFunctionCall{
                                    function: Box::new(
                                        TypedAccess {
                                            variable: TypedVariable {
                                                variable: DEFAULT_CONTEXT.with(|context| context.get(&Id::from("++")).unwrap().variable.clone()),
                                                type_: Type::Function(vec![TYPE_INT], Box::new(TYPE_INT)).into(),
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
                            assignments: Vec::new(),
                            expression: Box::new(
                                TypedFunctionCall{
                                    function: Box::new(
                                        TypedAccess {
                                            variable: TypedVariable {
                                                variable: DEFAULT_CONTEXT.with(|context| context.get(&Id::from("++")).unwrap().variable.clone()),
                                                type_: Type::Function(vec![TYPE_INT], Box::new(TYPE_INT)).into(),
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
            let return_address: IntermediateMemory = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT)).into();
            let y: IntermediateMemory = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: IntermediateBuiltIn::BuiltInFn(
                        Name::from("++"),
                        IntermediateFnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ).into(),
                args: vec![
                    arg.clone().into()
                ]
            }).into();
            let c: IntermediateMemory = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: IntermediateBuiltIn::BuiltInFn(
                        Name::from(">"),
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
            let z: IntermediateMemory = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: IntermediateBuiltIn::BuiltInFn(
                        Name::from("++"),
                        IntermediateFnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ).into(),
                args: vec![
                    y.location.clone().into()
                ]
            }).into();
            let memory: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: vec![arg.clone()],
                statements: vec![
                    y.clone().into(),
                    c.clone().into(),
                    IntermediateIfStatement{
                        condition: c.location.into(),
                        branches: (
                            vec![
                                z.clone().into(),
                                IntermediateMemory{
                                    location: return_address.location.clone(),
                                    expression: Rc::new(RefCell::new(IntermediateValue::from(z.location).into()))
                                }.into(),
                            ],
                            vec![
                                IntermediateMemory{
                                    location: return_address.location.clone(),
                                    expression: Rc::new(RefCell::new(IntermediateValue::from(y.location).into()))
                                }.into()
                            ]
                        )
                    }.into()
                ],
                return_value: return_address.location.clone().into()
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
            let arg = TypedVariable::from(Type::Union(Id::from("Bull"),vec![None,None]));
            TypedFunctionDefinition{
                parameters: vec![arg.clone()],
                body: TypedBlock{
                    assignments: Vec::new(),
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
                                    assignments: Vec::new(),
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
                                    assignments: Vec::new(),
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
            let return_address: IntermediateMemory = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT)).into();
            let memory: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: vec![arg.clone()],
                statements: vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateMemory{
                                        location: return_address.location.clone(),
                                        expression: Rc::new(RefCell::new(IntermediateBuiltIn::from(Integer{value: 1}).into()))
                                    }.into(),
                                ]
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateMemory{
                                        location: return_address.location.clone(),
                                        expression: Rc::new(RefCell::new(IntermediateBuiltIn::from(Integer{value: 0}).into()))
                                    }.into(),
                                ]
                            },
                        ]
                    }.into()
                ],
                return_value: return_address.location.clone().into()
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
            let arg = TypedVariable::from(Type::Union(Id::from("Either"),vec![Some(TYPE_INT),Some(TYPE_INT)]));
            let var = TypedVariable::from(TYPE_INT);
            TypedFunctionDefinition{
                parameters: vec![arg.clone()],
                body: TypedBlock{
                    assignments: Vec::new(),
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
                                    assignments: Vec::new(),
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
            let return_address: IntermediateMemory = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT)).into();
            let var: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let memory: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: vec![arg.clone()],
                statements: vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(var.clone()),
                                statements: vec![
                                    IntermediateMemory{
                                        location: return_address.location.clone(),
                                        expression: Rc::new(RefCell::new(var.clone().into()))
                                    }.into(),
                                ]
                            },
                            IntermediateMatchBranch{
                                target: Some(var.clone()),
                                statements: vec![
                                    IntermediateMemory{
                                        location: return_address.location.clone(),
                                        expression: Rc::new(RefCell::new(var.clone().into()))
                                    }.into(),
                                ]
                            },
                        ]
                    }.into()
                ],
                return_value: return_address.location.clone().into()
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
            let arg = TypedVariable::from(Type::Union(Id::from("Option"),vec![Some(TYPE_INT),None]));
            let var = TypedVariable::from(TYPE_INT);
            TypedFunctionDefinition{
                parameters: vec![arg.clone()],
                body: TypedBlock{
                    assignments: Vec::new(),
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
                                    assignments: Vec::new(),
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
                                    assignments: Vec::new(),
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
            let return_address: IntermediateMemory = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT)).into();
            let var: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let memory: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: vec![arg.clone()],
                statements: vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(var.clone()),
                                statements: vec![
                                    IntermediateMemory{
                                        location: return_address.location.clone(),
                                        expression: Rc::new(RefCell::new(var.clone().into()))
                                    }.into(),
                                ]
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateMemory{
                                        location: return_address.location.clone(),
                                        expression: Rc::new(RefCell::new(IntermediateBuiltIn::from(Integer{value: 0}).into()))
                                    }.into(),
                                ]
                            },
                        ]
                    }.into()
                ],
                return_value: return_address.location.clone().into()
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
        let expected_fn: IntermediateExpression = IntermediateFnDef {
            args: Vec::new(),
            statements,
            return_value: value,
        }
        .into();
        let mut lowerer = Lowerer::new();
        let value = lowerer.lower_expression(expression);
        let efficient_value = lowerer.remove_wasted_allocations_from_value(value);
        let efficient_statements =
            lowerer.remove_wasted_allocations_from_statements(lowerer.statements.clone());
        let efficient_fn = IntermediateFnDef {
            args: Vec::new(),
            statements: efficient_statements,
            return_value: efficient_value,
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
            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: args.clone(),
                statements: Vec::new(),
                return_value: args[0].clone().into(),
            })
        };
        let p1 = {
            let args = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: args.clone(),
                statements: Vec::new(),
                return_value: args[1].clone().into(),
            })
        };
        let q0 = {
            let args = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: args.clone(),
                statements: Vec::new(),
                return_value: args[0].clone().into(),
            })
        };
        let q1 = {
            let args = vec![
                IntermediateType::from(AtomicTypeEnum::INT).into(),
                IntermediateType::from(AtomicTypeEnum::INT).into(),
            ];
            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: args.clone(),
                statements: Vec::new(),
                return_value: args[1].clone().into(),
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
                Some(IntermediateType::Reference(reference.into())),
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
                    IntermediateType::Reference(reference.into())
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
                        value.location.into()
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
                        value.location.into()
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
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let fn_: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef{
                args: vec![arg.clone()],
                statements: Vec::new(),
                return_value: arg.clone().into()
            }).into();
            let value: IntermediateMemory = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: fn_.location.clone().into(),
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
                        fn_.location.into()
                    ),
                    (
                        y.variable,
                        value.location.into()
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
            let fn_: IntermediateMemory = IntermediateArg::from(
                IntermediateType::from(IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())))
            ).into();
            let recursive_call: IntermediateMemory = IntermediateExpression::IntermediateFnCall(
                IntermediateFnCall{
                    fn_: fn_.location.clone().into(),
                    args: Vec::new()
                }
            ).into();
            *fn_.expression.clone().borrow_mut() = IntermediateExpression::IntermediateFnDef(IntermediateFnDef{
                args: Vec::new(),
                statements: vec![
                    recursive_call.clone().into()
                ],
                return_value: recursive_call.location.into()
            }).into();
            let value: IntermediateMemory = IntermediateExpression::IntermediateFnCall(IntermediateFnCall{
                fn_: fn_.location.clone().into(),
                args: Vec::new()
            }).into();
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
                        fn_.location.into()
                    ),
                    (
                        y.variable,
                        value.location.into()
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
            let a_fn: IntermediateMemory = IntermediateArg::from(
                IntermediateType::from(IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::BOOL.into())))
            ).into();
            let b_fn: IntermediateMemory = IntermediateArg::from(
                IntermediateType::from(IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::BOOL.into())))
            ).into();
            let a_call: IntermediateMemory = IntermediateExpression::IntermediateFnCall(
                IntermediateFnCall{
                    fn_: a_fn.location.clone().into(),
                    args: Vec::new()
                }
            ).into();
            let b_call: IntermediateMemory = IntermediateExpression::IntermediateFnCall(
                IntermediateFnCall{
                    fn_: b_fn.location.clone().into(),
                    args: Vec::new()
                }
            ).into();
            *a_fn.expression.clone().borrow_mut() = IntermediateExpression::IntermediateFnDef(IntermediateFnDef{
                args: Vec::new(),
                statements: vec![
                    b_call.clone().into()
                ],
                return_value: b_call.location.into()
            }).into();
            *b_fn.expression.clone().borrow_mut() = IntermediateExpression::IntermediateFnDef(IntermediateFnDef{
                args: Vec::new(),
                statements: vec![
                    a_call.clone().into()
                ],
                return_value: a_call.location.into()
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
                        a_fn.location.into()
                    ),
                    (
                        b.variable,
                        b_fn.location.into()
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
        let int_arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
        let bool_arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
        let id_int_fn: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
            args: vec![int_arg.clone()],
            statements: Vec::new(),
            return_value: int_arg.into()
        }).into();
        let id_bool_fn: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
            args: vec![bool_arg.clone()],
            statements: Vec::new(),
            return_value: bool_arg.into()
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
                    id_int_fn.location.into()
                ),
                (
                    id_bool.variable,
                    id_bool_fn.location.clone().into()
                ),
                (
                    id_bool2.variable,
                    id_bool_fn.location.into()
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
            .map(|(k, v)| (k, v.expression.borrow().clone()))
            .collect::<HashMap<_, _>>();
        for (k, v) in expected_scope {
            let value = flat_scope
                .get(&(k, Vec::new()))
                .as_ref()
                .map(|&v| lowerer.remove_wasted_allocations_from_expression(v.clone()));
            assert!(ExpressionEqualityChecker::equal(&value.unwrap(), &v.into()))
        }
    }

    #[test_case(
        {
            let main: TypedVariable = ParametricType {
                type_: Type::Function(Vec::new(), Box::new(TYPE_INT)),
                parameters: Vec::new()
            }.into();
            TypedProgram {
                type_definitions: TypeDefinitions::new(),
                assignments: vec![
                    TypedAssignment{
                        variable: main.clone(),
                        expression: TypedExpression::from(TypedFunctionDefinition{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock {
                                assignments: Vec::new(),
                                expression: Box::new(Integer{value:0}.into())
                            }
                        }).into()
                    }
                ],
                main
            }
        },
        {
            let main: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: Vec::new(),
                statements: Vec::new(),
                return_value: IntermediateBuiltIn::from(Integer{value: 0}).into()
            }).into();
            IntermediateProgram{
                statements: vec![
                    main.clone().into()
                ],
                main: main.location.into()
            }
        };
        "return 0"
    )]
    #[test_case(
        {
            let main: TypedVariable = ParametricType {
                type_: Type::Function(Vec::new(), Box::new(TYPE_INT)),
                parameters: Vec::new()
            }.into();
            let parameter = Rc::new(RefCell::new(None));
            let type_definitions:TypeDefinitions = [(
                Id::from("Option"),
                ParametricType {
                    parameters: vec![parameter.clone()],
                    type_: Type::Union(
                        Id::from("Option"),
                        vec![
                            Some(Type::Variable(parameter)),
                            None
                        ]
                    )
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
                assignments: vec![
                    TypedAssignment {
                        expression: ParametricExpression{
                            parameters: Vec::new(),
                            expression: TypedMatch{
                                subject: Box::new(
                                    TypedConstructorCall{
                                        idx: 1,
                                        output_type: Type::Instantiation(type_definitions.get(&Id::from("Option")).unwrap().clone(), vec![TYPE_INT]),
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
                                            assignments: Vec::new(),
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
                                            assignments: Vec::new(),
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
                    },
                    TypedAssignment{
                        variable: main.clone(),
                        expression: TypedExpression::from(TypedFunctionDefinition{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock {
                                assignments: Vec::new(),
                                expression: Box::new(TypedAccess{
                                    variable: x,
                                    parameters: Vec::new(),
                                }.into())
                            }
                        }).into()
                    }
                ],
                main
            }
        },
        {
            let ctor: IntermediateMemory = IntermediateExpression::from(IntermediateCtorCall{
                idx: 1,
                data: Some(IntermediateBuiltIn::from(Integer{value: 1}).into()),
                type_: IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),None])
            }).into();
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let location = Location::new();
            let main: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: Vec::new(),
                statements: Vec::new(),
                return_value: location.clone().into()
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
                                    IntermediateMemory{
                                        location: location.clone(),
                                        expression: Rc::new(RefCell::new(arg.clone().into()))
                                    }.into(),
                                ]
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateMemory{
                                        location: location.clone(),
                                        expression: Rc::new(RefCell::new(IntermediateBuiltIn::from(Integer{value: 0}).into()))
                                    }.into(),
                                ]
                            },
                        ]
                    }.into(),
                    main.clone().into()
                ],
                main: main.location.into()
            }
        };
        "union type usage"
    )]
    #[test_case(
        {
            let main: TypedVariable = ParametricType {
                type_: Type::Function(Vec::new(), Box::new(TYPE_INT)),
                parameters: Vec::new()
            }.into();
            let parameter = Rc::new(RefCell::new(None));
            let type_variable = Type::Variable(parameter.clone());
            let arg: TypedVariable = ParametricType{
                parameters: Vec::new(),
                type_: type_variable.clone()
            }.into();
            let id: TypedVariable = ParametricType{
                parameters: vec![parameter.clone()],
                type_: Type::Function(vec![type_variable.clone()],Box::new(type_variable.clone()))
            }.into();
            TypedProgram {
                type_definitions: TypeDefinitions::new(),
                assignments: vec![
                    TypedAssignment {
                        expression: ParametricExpression{
                            parameters: vec![(Id::from("T"), parameter.clone())],
                            expression: TypedFunctionDefinition{
                                parameters: vec![
                                    arg.clone()
                                ],
                                return_type: Box::new(type_variable.clone()),
                                body: TypedBlock {
                                    assignments: Vec::new(),
                                    expression: Box::new(TypedAccess{
                                        variable: arg,
                                        parameters: Vec::new()
                                    }.into())
                                }
                            }.into()
                        },
                        variable: id.clone()
                    },
                    TypedAssignment{
                        variable: main.clone(),
                        expression: TypedExpression::from(TypedFunctionDefinition{
                            parameters: Vec::new(),
                            return_type: Box::new(TYPE_INT),
                            body: TypedBlock {
                                assignments: Vec::new(),
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
                    }
                ],
                main
            }
        },
        {
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let id_int: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: vec![arg.clone()],
                statements: Vec::new(),
                return_value: arg.into()
            }).into();
            let fn_call: IntermediateMemory = IntermediateExpression::from(IntermediateFnCall{
                args: vec![IntermediateBuiltIn::from(Integer{value: 0}).into()],
                fn_: id_int.location.clone().into()
            }).into();
            let main: IntermediateMemory = IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                args: Vec::new(),
                statements: vec![
                    id_int.into(),
                    fn_call.clone().into()
                ],
                return_value: fn_call.location.into()
            }).into();
            IntermediateProgram{
                statements: vec![
                    main.clone().into()
                ],
                main: main.location.into()
            }
        };
        "parametric variable"
    )]
    fn test_lower_program(program: TypedProgram, expected: IntermediateProgram) {
        let mut lowerer = Lowerer::new();
        let lower_program = lowerer.lower_program(program);
        let lower_fn: IntermediateExpression = IntermediateFnDef {
            args: Vec::new(),
            statements: lower_program.statements,
            return_value: lower_program.main,
        }
        .into();
        let expected_fn: IntermediateExpression = IntermediateFnDef {
            args: Vec::new(),
            statements: expected.statements,
            return_value: expected.main,
        }
        .into();
        assert!(ExpressionEqualityChecker::equal(&lower_fn, &expected_fn));
    }

    #[test_case(
        (
            IntermediateBuiltIn::from(Integer{value: 11}).into(),
            MemoryMap::new()
        ),
        AtomicTypeEnum::INT.into();
        "integer"
    )]
    #[test_case(
        (
            IntermediateBuiltIn::from(Boolean{value: true}).into(),
            MemoryMap::new()
        ),
        AtomicTypeEnum::BOOL.into();
        "boolean"
    )]
    #[test_case(
        (
            IntermediateBuiltIn::BuiltInFn(
                Name::from("+"),
                IntermediateFnType(
                    vec![
                        AtomicTypeEnum::INT.into(),
                        AtomicTypeEnum::INT.into(),
                    ],
                    Box::new(AtomicTypeEnum::INT.into())
                ).into()
            ).into(),
            MemoryMap::new()
        ),
        IntermediateFnType(
            vec![
                AtomicTypeEnum::INT.into(),
                AtomicTypeEnum::INT.into(),
            ],
            Box::new(AtomicTypeEnum::INT.into())
        ).into();
        "builtin-function"
    )]
    #[test_case(
        {
            let location = Location::new();
            (
                IntermediateValue::IntermediateMemory(location.clone()),
                MemoryMap::from([(
                    location,
                    vec![Rc::new(RefCell::new(IntermediateBuiltIn::from(Integer{value: 8}).into()))]
                )])
            )
        },
        AtomicTypeEnum::INT.into();
        "single value memory location"
    )]
    #[test_case(
        {
            let location = Location::new();
            (
                IntermediateValue::IntermediateMemory(location.clone()),
                MemoryMap::from([(
                    location,
                    vec![
                        Rc::new(RefCell::new(IntermediateBuiltIn::from(Integer{value: 8}).into())),
                        Rc::new(RefCell::new(IntermediateBuiltIn::from(Integer{value: -8}).into())),
                    ]
                )])
            )
        },
        AtomicTypeEnum::INT.into();
        "multiple value memory location"
    )]
    #[test_case(
        (
            IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT)).into(),
            MemoryMap::new()
        ),
        AtomicTypeEnum::INT.into();
        "argument"
    )]
    fn test_value_type(value_memory_map: (IntermediateValue, MemoryMap), type_: IntermediateType) {
        let (value, memory_map) = value_memory_map;
        let mut lowerer = Lowerer::new();
        lowerer.memory = memory_map;
        assert_eq!(lowerer.value_type(&value), type_);
    }

    #[test_case(
        (
            IntermediateCtorCall{
                idx: 0,
                data: None,
                type_: IntermediateUnionType(vec![None,None])
            }.into(),
            MemoryMap::new()
        ),
        IntermediateUnionType(vec![None,None]).into();
        "ctor call no data"
    )]
    #[test_case(
        (
            {
                let reference = Rc::new(RefCell::new(IntermediateTupleType(Vec::new()).into()));
                let type_ = IntermediateUnionType(vec![Some(IntermediateType::Reference(reference.clone())), None]);
                *reference.borrow_mut() = type_.clone().into();
                IntermediateCtorCall{
                    idx: 1,
                    data: None,
                    type_: type_
                }.into()
            },
            MemoryMap::new()
        ),
        {
            let reference = Rc::new(RefCell::new(IntermediateTupleType(Vec::new()).into()));
            let type_ = IntermediateUnionType(vec![Some(IntermediateType::Reference(reference.clone())), None]);
            *reference.borrow_mut() = type_.clone().into();
            type_.into()
        };
        "recursive ctor"
    )]
    #[test_case(
        (
            IntermediateFnDef{
                args: Vec::new(),
                statements: Vec::new(),
                return_value: IntermediateBuiltIn::from(Integer{value: 5}).into()
            }.into(),
            MemoryMap::new()
        ),
        IntermediateFnType(Vec::new(), Box::new(AtomicTypeEnum::INT.into())).into();
        "fn def no args"
    )]
    #[test_case(
        (
            {
                let args = vec![
                    IntermediateType::from(AtomicTypeEnum::INT).into(),
                    IntermediateType::from(AtomicTypeEnum::BOOL).into(),
                ];
                IntermediateFnDef{
                    args: args.clone(),
                    statements: Vec::new(),
                    return_value: args[1].clone().into()
                }.into()
            },
            MemoryMap::new()
        ),
        IntermediateFnType(vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::BOOL.into()], Box::new(AtomicTypeEnum::BOOL.into())).into();
        "fn def with args"
    )]
    #[test_case(
        (
            IntermediateFnCall{
                fn_: IntermediateBuiltIn::BuiltInFn(
                    Name::from("<"),
                    IntermediateFnType(
                        vec![
                            AtomicTypeEnum::INT.into(),
                            AtomicTypeEnum::INT.into()
                        ],
                        Box::new(AtomicTypeEnum::BOOL.into())
                    ).into()
                ).into(),
                args: vec![
                    IntermediateBuiltIn::from(Integer{value: 3}).into(),
                    IntermediateBuiltIn::from(Integer{value: 4}).into(),
                ]
            }.into(),
            MemoryMap::new()
        ),
        AtomicTypeEnum::BOOL.into();
        "fn call"
    )]
    #[test_case(
        (
            IntermediateTupleExpression(Vec::new()).into(),
            MemoryMap::new()
        ),
        IntermediateTupleType(Vec::new()).into();
        "empty tuple"
    )]
    #[test_case(
        (
            IntermediateTupleExpression(
                vec![
                    IntermediateBuiltIn::from(Integer{value: 4}).into(),
                    IntermediateBuiltIn::from(Boolean{value: false}).into(),
                ]
            ).into(),
            MemoryMap::new()
        ),
        IntermediateTupleType(vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::BOOL.into()]).into();
        "non-empty tuple"
    )]
    #[test_case(
        {
            let location = Location::new();
            (
                IntermediateElementAccess{
                    value: location.clone().into(),
                    idx: 1
                }.into(),
                MemoryMap::from([(
                    location,
                    vec![Rc::new(RefCell::new(
                        IntermediateTupleExpression(
                            vec![
                                IntermediateBuiltIn::from(Integer{value: 4}).into(),
                                IntermediateBuiltIn::from(Boolean{value: false}).into(),
                            ]
                        ).into(),
                    ))]
                )])
            )
        },
        AtomicTypeEnum::BOOL.into();
        "tuple access"
    )]
    fn test_expression_type(
        expression_memory_map: (IntermediateExpression, MemoryMap),
        type_: IntermediateType,
    ) {
        let (expression, memory_map) = expression_memory_map;
        let mut lowerer = Lowerer::new();
        lowerer.memory = memory_map;
        assert_eq!(lowerer.expression_type(&expression), type_);
    }

    #[test_case(
        Vec::new(),
        Vec::new();
        "empty type defs"
    )]
    #[test_case(
        vec![
            Rc::new(RefCell::new(IntermediateUnionType(
                vec![None,None,]
            ).into())),
            Rc::new(RefCell::new(IntermediateUnionType(
                vec![Some(AtomicTypeEnum::INT.into()),None,]
            ).into()))
        ],
        vec![
            TypeDef {
                name: Name::from("T0"),
                constructors: vec![
                    (Name::from("T0C0"), None),
                    (Name::from("T0C1"), None),
                ]
            },
            TypeDef {
                name: Name::from("T1"),
                constructors: vec![
                    (Name::from("T1C0"), Some(AtomicTypeEnum::INT.into())),
                    (Name::from("T1C1"), None),
                ]
            },
        ];
        "non-recursive union types"
    )]
    #[test_case(
        vec![
            {
                let reference = Rc::new(RefCell::new(IntermediateTupleType(Vec::new()).into()));
                let recursive_type = IntermediateUnionType(
                    vec![
                        Some(IntermediateType::Reference(reference.clone())),
                        None
                    ]
                ).into();
                *reference.borrow_mut() = recursive_type;
                reference
            },
            {
                let reference = Rc::new(RefCell::new(IntermediateTupleType(Vec::new()).into()));
                let recursive_type = IntermediateUnionType(
                    vec![
                        Some(IntermediateTupleType(vec![IntermediateType::Reference(reference.clone()), AtomicTypeEnum::INT.into()]).into()),
                        None
                    ]
                ).into();
                *reference.borrow_mut() = recursive_type;
                reference
            },
        ],
        vec![
            TypeDef {
                name: Name::from("T0"),
                constructors: vec![
                    (Name::from("T0C0"), Some(MachineType::Reference(Box::new(MachineType::NamedType(Name::from("T0")))))),
                    (Name::from("T0C1"), None),
                ]
            },
            TypeDef {
                name: Name::from("T1"),
                constructors: vec![
                    (Name::from("T1C0"), Some(TupleType(vec![
                        MachineType::Reference(Box::new(MachineType::NamedType(Name::from("T1")))),
                        AtomicTypeEnum::INT.into()
                    ]).into())),
                    (Name::from("T1C1"), None),
                ]
            },
        ];
        "recursive union types"
    )]
    #[test_case(
        vec![
            Rc::new(RefCell::new(IntermediateType::Reference(
                Rc::new(RefCell::new(
                    IntermediateTupleType(
                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()]
                    ).into()
                ))
            ))),
            Rc::new(RefCell::new(IntermediateUnionType(
                vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::BOOL.into()),]
            ).into()))
        ],
        vec![
            TypeDef {
                name: Name::from("T0"),
                constructors: vec![
                    (Name::from("T0C0"), Some(AtomicTypeEnum::INT.into())),
                    (Name::from("T0C1"), Some(AtomicTypeEnum::BOOL.into())),
                ]
            },
        ];
        "mixed types"
    )]
    fn test_compile_type_defs(
        type_defs: Vec<Rc<RefCell<IntermediateType>>>,
        expected_type_defs: Vec<TypeDef>,
    ) {
        let mut lowerer = Lowerer::new();
        assert_eq!(lowerer.compile_type_defs(type_defs), expected_type_defs)
    }

    #[test_case(
        IntermediateBuiltIn::from(Integer{value: 4}).into(),
        BuiltIn::from(Integer{value: 4}).into();
        "integer"
    )]
    #[test_case(
        IntermediateBuiltIn::from(Boolean{value: true}).into(),
        BuiltIn::from(Boolean{value: true}).into();
        "boolean"
    )]
    #[test_case(
        IntermediateBuiltIn::BuiltInFn(
            Name::from("=="),
            IntermediateFnType(
                vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::BOOL.into())
            ).into()
        ).into(),
        BuiltIn::BuiltInFn(
            Name::from("Comparison_EQ__BuiltIn"),
        ).into();
        "built-in fn"
    )]
    #[test_case(
        IntermediateValue::IntermediateMemory(
            Location::new()
        ),
        Memory(
            Id::from("m0")
        ).into();
        "memory"
    )]
    fn test_compile_values(value: IntermediateValue, expected_value: Value) {
        let mut lowerer = Lowerer::new();
        let (_, compiled_value) = lowerer.compile_value(value, false);
        assert_eq!(compiled_value, expected_value);
    }
    #[test]
    fn test_compile_multiple_memory_locations() {
        let locations = vec![Location::new(), Location::new(), Location::new()];
        let mut lowerer = Lowerer::new();
        let value_0 = lowerer.compile_value(locations[0].clone().into(), false);
        let value_1 = lowerer.compile_value(locations[1].clone().into(), false);
        let value_2 = lowerer.compile_value(locations[2].clone().into(), false);
        assert_ne!(value_0, value_1);
        assert_ne!(value_2, value_1);
        assert_ne!(value_2, value_0);

        assert_eq!(
            value_0,
            lowerer.compile_value(locations[0].clone().into(), false)
        );
        assert_eq!(
            value_1,
            lowerer.compile_value(locations[1].clone().into(), false)
        );
        assert_eq!(
            value_2,
            lowerer.compile_value(locations[2].clone().into(), false)
        );
    }
    #[test]
    fn test_compile_arguments() {
        let types = vec![
            Rc::new(RefCell::new(AtomicTypeEnum::INT.into())),
            Rc::new(RefCell::new(AtomicTypeEnum::BOOL.into())),
            Rc::new(RefCell::new(AtomicTypeEnum::INT.into())),
        ];
        let mut lowerer = Lowerer::new();
        lowerer
            .arg_ids
            .insert(types[0].as_ptr(), Memory(Id::from("a0")));
        lowerer
            .arg_ids
            .insert(types[1].as_ptr(), Memory(Id::from("a1")));
        lowerer
            .arg_ids
            .insert(types[2].as_ptr(), Memory(Id::from("a2")));

        let args = types
            .into_iter()
            .map(|type_| IntermediateArg(type_))
            .collect_vec();
        let value_0 = lowerer.compile_value(args[0].clone().into(), true);
        let value_1 = lowerer.compile_value(args[1].clone().into(), true);
        let value_2 = lowerer.compile_value(args[2].clone().into(), true);
        assert_ne!(value_0, value_1);
        assert_ne!(value_2, value_1);
        assert_ne!(value_2, value_0);

        assert_eq!(value_0, lowerer.compile_value(args[0].clone().into(), true));
        assert_eq!(value_1, lowerer.compile_value(args[1].clone().into(), true));
        assert_eq!(value_2, lowerer.compile_value(args[2].clone().into(), true));
    }

    #[test_case(
        IntermediateTupleExpression(Vec::new()).into(),
        (
            Vec::new(),
            TupleExpression(Vec::new()).into()
        );
        "empty expression"
    )]
    #[test_case(
        IntermediateTupleExpression(
            vec![
                IntermediateBuiltIn::from(Integer{value: 5}).into(),
                IntermediateBuiltIn::from(Boolean{value: true}).into(),
            ]
        ).into(),
        (
            Vec::new(),
            TupleExpression(vec![
                BuiltIn::from(Integer{value: 5}).into(),
                BuiltIn::from(Boolean{value: true}).into(),
            ]).into()
        );
        "tuple expression"
    )]
    #[test_case(
        IntermediateTupleExpression(
            vec![
                IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT)).into()
            ]
        ).into(),
        (
            vec![
                Await(vec![Memory(Id::from("a0"))]).into(),
                Declaration{
                    memory: Memory(Id::from("m0")),
                    type_: AtomicTypeEnum::INT.into()
                }.into(),
                Assignment{
                    check_null: false,
                    target: Memory(Id::from("m0")),
                    value: Expression::Unwrap(Memory(Id::from("a0")).into())
                }.into()
            ],
            TupleExpression(vec![
                Memory(Id::from("m0")).into()
            ]).into()
        );
        "tuple expression with argument"
    )]
    #[test_case(
        IntermediateTupleExpression(
            {
                let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
                vec![arg.clone().into(),arg.into()]
            }
        ).into(),
        (
            vec![
                Await(vec![Memory(Id::from("a1"))]).into(),
                Declaration{
                    type_: AtomicTypeEnum::INT.into(),
                    memory: Memory(Id::from("m0"))
                }.into(),
                Assignment{
                    check_null: false,
                    target: Memory(Id::from("m0")),
                    value: Expression::Unwrap(Memory(Id::from("a1")).into())
                }.into(),
            ],
            TupleExpression(vec![
                Memory(Id::from("m0")).into(),
                Memory(Id::from("m0")).into(),
            ]).into()
        );
        "tuple expression duplicate arguments"
    )]
    #[test_case(
        IntermediateElementAccess{
            value: IntermediateArg::from(IntermediateType::from(
                IntermediateTupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    AtomicTypeEnum::BOOL.into(),
                ]))
            ).into(),
            idx: 1
        }.into(),
        (
            vec![
                Await(vec![Memory(Id::from("a0"))]).into(),
                Declaration {
                    type_: TupleType(vec![
                        AtomicTypeEnum::INT.into(),
                        AtomicTypeEnum::BOOL.into(),
                    ]).into(),
                    memory: Memory(Id::from("m0"))
                }.into(),
                Assignment{
                    check_null: false,
                    target: Memory(Id::from("m0")),
                    value: Expression::Unwrap(Memory(Id::from("a0")).into())
                }.into()
            ],
            ElementAccess{
                value: Memory(Id::from("m0")).into(),
                idx: 1
            }.into()
        );
        "argument element access"
    )]
    #[test_case(
        IntermediateFnCall{
            fn_: IntermediateBuiltIn::BuiltInFn(
                Name::from("++"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                ).into()
            ).into(),
            args: vec![IntermediateBuiltIn::from(Integer{value: 7}).into()]
        }.into(),
        (
            vec![
                Declaration{
                    type_: MachineType::Lazy(
                        Box::new(AtomicTypeEnum::INT.into())
                    ),
                    memory: Memory(Id::from("m0"))
                }.into(),
                Assignment{
                    check_null: false,
                    target: Memory(Id::from("m0")),
                    value: Expression::Wrap(
                        BuiltIn::from(Integer{value: 7}).into(),
                        AtomicTypeEnum::INT.into()
                    )
                }.into()
            ],
            FnCall{
                args: vec![Memory(Id::from("m0")).into()],
                fn_: BuiltIn::BuiltInFn(
                    Name::from("Increment__BuiltIn"),
                ).into(),
                fn_type: FnType(
                    vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                )
            }.into()
        );
        "built-in fn call"
    )]
    #[test_case(
        IntermediateFnCall{
            fn_: IntermediateBuiltIn::BuiltInFn(
                Name::from("*"),
                IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into())
                ).into()
            ).into(),
            args: vec![
                IntermediateBuiltIn::from(Integer{value: 9}).into(),
                IntermediateBuiltIn::from(Integer{value: 9}).into(),
            ]
        }.into(),
        (
            vec![
                Declaration {
                    type_: MachineType::Lazy(
                        Box::new(AtomicTypeEnum::INT.into())
                    ),
                    memory: Memory(Id::from("m0"))
                }.into(),
                Assignment{
                    check_null: false,
                    target: Memory(Id::from("m0")),
                    value: Expression::Wrap(
                        BuiltIn::from(Integer{value: 9}).into(),
                        AtomicTypeEnum::INT.into()
                    )
                }.into()
            ],
            FnCall{
                args: vec![
                    Memory(Id::from("m0")).into(),
                    Memory(Id::from("m0")).into(),
                ],
                fn_: BuiltIn::BuiltInFn(
                    Name::from("Multiply__BuiltIn"),
                ).into(),
                fn_type: FnType(
                    vec![
                        MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                        MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                    ],
                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                )
            }.into()
        );
        "fn call reused arg"
    )]
    #[test_case(
        IntermediateFnCall{
            fn_: IntermediateArg::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::BOOL.into())
                ))
            ).into(),
            args: vec![
                IntermediateArg::from(
                    IntermediateType::from(
                        AtomicTypeEnum::INT
                    )
                ).into(),
            ]
        }.into(),
        (
            vec![
                Await(vec![Memory(Id::from("a1"))]).into(),
                Declaration {
                    type_: FnType(
                        vec![
                            MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                        ],
                        Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::BOOL.into())))
                    ).into(),
                    memory: Memory(Id::from("m0"))
                }.into(),
                Assignment{
                    check_null: false,
                    target: Memory(Id::from("m0")),
                    value: Expression::Unwrap(
                        Memory(Id::from("a1")).into()
                    )
                }.into()
            ],
            FnCall{
                args: vec![
                    Memory(Id::from("a0")).into(),
                ],
                fn_: Memory(Id::from("m0")).into(),
                fn_type: FnType(
                    vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::BOOL.into())))
                )
            }.into()
        );
        "fn call higher-order call from args"
    )]
    #[test_case(
        IntermediateCtorCall{
            idx: 0,
            data: None,
            type_: IntermediateUnionType(vec![None,None])
        }.into(),
        (
            Vec::new(),
            ConstructorCall{
                idx: 0,
                data: None
            }.into()
        );
        "no data constructor call"
    )]
    fn test_compile_expressions(
        expression: IntermediateExpression,
        expected: (Vec<Statement>, Expression),
    ) {
        let mut lowerer = Lowerer::new();
        for value in expression.values() {
            match value {
                IntermediateValue::IntermediateArg(IntermediateArg(reference)) => {
                    lowerer
                        .arg_ids
                        .insert(reference.as_ptr(), lowerer.next_arg_id());
                }
                _ => (),
            }
        }
        let result = lowerer.compile_expression(expression);
        assert_eq!(result, expected);
    }
    #[test]
    fn test_compile_tuple_expression_with_reference() {
        let reference = Rc::new(RefCell::new(IntermediateUnionType(Vec::new()).into()));
        let nat_type: IntermediateType = IntermediateUnionType(vec![
            Some(
                IntermediateTupleType(vec![IntermediateType::Reference(reference.clone())]).into(),
            ),
            None,
        ])
        .into();
        *reference.borrow_mut() = nat_type.clone();

        let argument = IntermediateArg::from(IntermediateType::Reference(reference.clone()));

        let location = Location::new();
        let intermediate_expression =
            IntermediateTupleExpression(vec![location.clone().into()]).into();

        let mut lowerer = Lowerer::new();
        lowerer.compile_type_defs(vec![reference]);
        lowerer
            .memory
            .insert(location, vec![Rc::new(RefCell::new(argument.into()))]);

        let (statements, expression) = lowerer.compile_expression(intermediate_expression);
        assert_eq!(
            statements,
            vec![
                Declaration {
                    memory: Memory(Id::from("m1")),
                    type_: MachineType::Reference(Box::new(MachineType::UnionType(UnionType(
                        vec![Id::from("T0C0"), Id::from("T0C1"),]
                    ))))
                }
                .into(),
                Assignment {
                    target: Memory(Id::from("m1")),
                    check_null: false,
                    value: Expression::Reference(
                        Memory(Id::from("m0")).into(),
                        MachineType::UnionType(UnionType(
                            vec![Id::from("T0C0"), Id::from("T0C1"),]
                        ))
                    )
                }
                .into()
            ]
        );
        assert_eq!(
            expression,
            TupleExpression(vec![Memory(Id::from("m1")).into()]).into()
        );
    }
    #[test]
    fn test_compile_tuple_access_expression_with_dereference() {
        let reference = Rc::new(RefCell::new(IntermediateUnionType(Vec::new()).into()));
        let nat_type: IntermediateType = IntermediateUnionType(vec![
            Some(IntermediateType::Reference(reference.clone())),
            None,
        ])
        .into();
        *reference.borrow_mut() = nat_type.clone();

        let argument = IntermediateArg::from(IntermediateType::from(IntermediateTupleType(vec![
            IntermediateType::Reference(reference.clone()),
        ])));

        let location = Location::new();
        let intermediate_expression = IntermediateElementAccess {
            value: location.clone().into(),
            idx: 0,
        }
        .into();

        let mut lowerer = Lowerer::new();
        lowerer.compile_type_defs(vec![reference]);
        lowerer
            .memory
            .insert(location, vec![Rc::new(RefCell::new(argument.into()))]);

        let (statements, expression) = lowerer.compile_expression(intermediate_expression);
        assert_eq!(
            statements,
            vec![
                Declaration {
                    memory: Memory(Id::from("m1")),
                    type_: MachineType::Reference(Box::new(MachineType::NamedType(Name::from(
                        "T0"
                    ))))
                }
                .into(),
                Assignment {
                    target: Memory(Id::from("m1")),
                    check_null: false,
                    value: ElementAccess {
                        value: Memory(Id::from("m0")).into(),
                        idx: 0
                    }
                    .into()
                }
                .into()
            ]
        );
        assert_eq!(
            expression,
            Expression::Dereference(Memory(Id::from("m1")).into())
        );
    }
    #[test_case(
        {
            let type_ = IntermediateUnionType(vec![Some(AtomicTypeEnum::BOOL.into()),Some(AtomicTypeEnum::INT.into())]);
            (
                IntermediateCtorCall{
                    idx: 1,
                    data: Some(IntermediateBuiltIn::from(Integer{value: 9}).into()),
                    type_: type_.clone()
                }.into(),
                Rc::new(RefCell::new(type_.into()))
            )
        },
        (
            Vec::new(),
            ConstructorCall{
                idx: 1,
                data: Some((Name::from("T0C1"), BuiltIn::from(Integer{value: 9}).into()))
            }.into()
        );
        "data constructor call"
    )]
    #[test_case(
        {
            let reference = Rc::new(RefCell::new(IntermediateUnionType(Vec::new()).into()));
            let union_type = IntermediateUnionType(vec![Some(IntermediateType::Reference(reference.clone())),None]);
            *reference.borrow_mut() = union_type.clone().into();
            (
                IntermediateCtorCall{
                    idx: 0,
                    data: Some(Location::new().into()),
                    type_: union_type
                }.into(),
                reference
            )
        },
        (
            Vec::new(),
            ConstructorCall{
                idx: 0,
                data: Some((Name::from("T0C0"), Memory(Id::from("m0")).into()))
            }.into()
        );
        "recursive constructor call"
    )]
    fn test_compile_constructors(
        constructor_type: (IntermediateCtorCall, Rc<RefCell<IntermediateType>>),
        expected: (Vec<Statement>, Expression),
    ) {
        let (constructor, type_) = constructor_type;
        let mut lowerer = Lowerer::new();
        lowerer.compile_type_defs(vec![type_]);
        let result = lowerer.compile_expression(constructor.into());
        assert_eq!(result, expected);
    }

    #[test_case(
        (
            Vec::new(),
            vec![
                IntermediateStatement::Assignment(
                    IntermediateMemory{
                        expression: Rc::new(RefCell::new(
                            IntermediateTupleExpression(vec![
                                IntermediateBuiltIn::from(Integer{value: 5}).into(),
                                IntermediateBuiltIn::from(Boolean{value: false}).into(),
                            ]).into()
                        )),
                        location: Location::new()
                    }
                )
            ],
        ),
        vec![
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: TupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    AtomicTypeEnum::BOOL.into(),
                ]).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m0")),
                value: TupleExpression(vec![
                    BuiltIn::from(Integer{value: 5}).into(),
                    BuiltIn::from(Boolean{value: false}).into(),
                ]).into(),
                check_null: false
            }.into()
        ];
        "tuple expression assignment"
    )]
    #[test_case(
        {
            let argument = IntermediateArg::from(
                IntermediateType::from(
                    IntermediateTupleType(vec![
                        AtomicTypeEnum::INT.into(),
                        AtomicTypeEnum::BOOL.into(),
                    ])
                )
            );
            (
                vec![argument.clone()],
                vec![
                    IntermediateStatement::Assignment(
                        IntermediateMemory{
                            expression: Rc::new(RefCell::new(
                                IntermediateElementAccess{
                                    idx: 1,
                                    value: argument.into()
                                }.into()
                            )),
                            location: Location::new()
                        }
                    )
                ]
            )
        },
        vec![
            Await(vec![Memory(Id::from("a0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: TupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    AtomicTypeEnum::BOOL.into(),
                ]).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m0")),
                value: Expression::Unwrap(Memory(Id::from("a0")).into()),
                check_null: false
            }.into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: AtomicTypeEnum::BOOL.into(),
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: ElementAccess{
                    idx: 1,
                    value: Memory(Id::from("m0")).into(),
                }.into(),
                check_null: false
            }.into()
        ];
        "tuple access assignment"
    )]
    #[test_case(
        (
            Vec::new(),
            vec![
                IntermediateStatement::Assignment(
                    IntermediateMemory{
                        expression: Rc::new(RefCell::new(IntermediateFnCall{
                            fn_: IntermediateBuiltIn::BuiltInFn(
                                Name::from("--"),
                                IntermediateFnType(
                                    vec![AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into())
                                ).into()
                            ).into(),
                            args: vec![
                                IntermediateBuiltIn::from(Integer{value: 11}).into()
                            ]
                        }.into())),
                        location: Location::new()
                    }
                )
            ],
        ),
        vec![
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: MachineType::Lazy(
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m0")),
                value: Expression::Wrap(
                    BuiltIn::from(Integer{value: 11}).into(),
                    AtomicTypeEnum::INT.into()
                ).into(),
                check_null: false
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: FnCall{
                    fn_: BuiltIn::BuiltInFn(
                        Name::from("Decrement__BuiltIn"),
                    ).into(),
                    fn_type: FnType(
                        vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                        Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                    ),
                    args: vec![Memory(Id::from("m0")).into()]
                }.into(),
                check_null: true
            }.into(),
        ];
        "fn call"
    )]
    #[test_case(
        {
            let type_: IntermediateType = IntermediateFnType(
                vec![
                    IntermediateTupleType(vec![
                        AtomicTypeEnum::INT.into()
                    ]).into()
                ],
                Box::new(AtomicTypeEnum::INT.into())
            ).into();
            let arg_0 = IntermediateArg::from(type_.clone());
            let arg_1 = IntermediateArg::from(type_.clone());
            let tuple = Location::new();
            (
                vec![arg_0.clone(), arg_1.clone()],
                vec![
                    IntermediateStatement::Assignment(
                        IntermediateMemory{
                            expression: Rc::new(RefCell::new(
                                IntermediateTupleExpression(vec![
                                    IntermediateBuiltIn::from(Integer{value: 5}).into(),
                                ]).into()
                            )),
                            location: tuple.clone()
                        }
                    ),
                    IntermediateStatement::Assignment(
                        IntermediateMemory{
                            expression: Rc::new(RefCell::new(IntermediateFnCall{
                                fn_: arg_0.into(),
                                args: vec![tuple.clone().into()]
                            }.into())),
                            location: Location::new()
                        }
                    ),
                    IntermediateStatement::Assignment(
                        IntermediateMemory{
                            expression: Rc::new(RefCell::new(IntermediateFnCall{
                                fn_: arg_1.into(),
                                args: vec![tuple.clone().into()]
                            }.into())),
                            location: Location::new()
                        }
                    )
                ],
            )
        },
        vec![
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: TupleType(vec![
                    AtomicTypeEnum::INT.into(),
                ]).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m0")),
                value: TupleExpression(vec![
                    BuiltIn::from(Integer{value: 5}).into(),
                ]).into(),
                check_null: false
            }.into(),
            Await(vec![Memory(Id::from("a0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: FnType(
                    vec![MachineType::Lazy(Box::new(
                        TupleType(vec![AtomicTypeEnum::INT.into()]).into()
                    ))],
                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                ).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: Expression::Unwrap(
                    Memory(Id::from("a0")).into()
                ),
                check_null: false
            }.into(),
            Declaration {
                memory: Memory(Id::from("m2")),
                type_: MachineType::Lazy(Box::new(TupleType(vec![
                    AtomicTypeEnum::INT.into(),
                ]).into()))
            }.into(),
            Assignment {
                target: Memory(Id::from("m2")),
                value: Expression::Wrap(
                    Memory(Id::from("m0")).into(),
                    TupleType(vec![
                        AtomicTypeEnum::INT.into(),
                    ]).into()
                ),
                check_null: false
            }.into(),
            Assignment {
                target: Memory(Id::from("m3")),
                value: FnCall{
                    fn_: Memory(Id::from("m1")).into(),
                    args: vec![
                        Memory(Id::from("m2")).into(),
                    ],
                    fn_type: FnType(
                        vec![MachineType::Lazy(Box::new(TupleType(vec![AtomicTypeEnum::INT.into()]).into()))],
                        Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                    )
                }.into(),
                check_null: true
            }.into(),
            Await(vec![Memory(Id::from("a1"))]).into(),
            Declaration {
                memory: Memory(Id::from("m4")),
                type_: FnType(
                    vec![MachineType::Lazy(Box::new(
                        TupleType(vec![AtomicTypeEnum::INT.into()]).into()
                    ))],
                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                ).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m4")),
                value: Expression::Unwrap(
                    Memory(Id::from("a1")).into()
                ),
                check_null: false
            }.into(),
            Assignment {
                target: Memory(Id::from("m5")),
                value: FnCall{
                    fn_: Memory(Id::from("m4")).into(),
                    args: vec![
                        Memory(Id::from("m2")).into(),
                    ],
                    fn_type: FnType(
                        vec![MachineType::Lazy(Box::new(TupleType(vec![AtomicTypeEnum::INT.into()]).into()))],
                        Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                    )
                }.into(),
                check_null: true
            }.into(),
        ];
        "tuple expression then fn call"
    )]
    #[test_case(
        {
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            let location = Location::new();
            (
                vec![arg.clone()],
                vec![
                    IntermediateIfStatement{
                        condition: arg.into(),
                        branches: (
                            vec![
                                IntermediateStatement::Assignment(
                                    IntermediateMemory {
                                        location: location.clone(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into()
                                        ))
                                    }
                                )
                            ],
                            vec![
                                IntermediateStatement::Assignment(
                                    IntermediateMemory {
                                        location: location.clone(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                        ))
                                    }
                                )
                            ]
                        )
                    }.into()
                ]
            )
        },
        vec![
            Await(vec![Memory(Id::from("a0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m0")),
                value: Expression::Unwrap(
                    Memory(Id::from("a0")).into()
                ),
                check_null: false
            }.into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            IfStatement {
                condition: Memory(Id::from("m0")).into(),
                branches: (
                    vec![
                        Assignment {
                            target: Memory(Id::from("m1")),
                            value: Expression::Value(
                                BuiltIn::from(Integer{value: 1}).into()
                            ),
                            check_null: false
                        }.into(),
                    ],
                    vec![
                        Assignment {
                            target: Memory(Id::from("m1")),
                            value: Expression::Value(
                                BuiltIn::from(Integer{value: 0}).into()
                            ),
                            check_null: false
                        }.into(),
                    ],
                )
            }.into()
        ];
        "if statement awaited argument"
    )]
    #[test_case(
        {
            let location = Location::new();
            (
                Vec::new(),
                vec![
                    IntermediateIfStatement{
                        condition: IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into(),
                        branches: (
                            vec![
                                IntermediateStatement::Assignment(
                                    IntermediateMemory {
                                        location: location.clone(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into()
                                        ))
                                    }
                                )
                            ],
                            vec![
                                IntermediateStatement::Assignment(
                                    IntermediateMemory {
                                        location: location.clone(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: false})).into()
                                        ))
                                    }
                                )
                            ]
                        )
                    }.into()
                ]
            )
        },
        vec![
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            IfStatement {
                condition: BuiltIn::from(Boolean{value: true}).into(),
                branches: (
                    vec![
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: Expression::Value(
                                BuiltIn::from(Boolean{value: true}).into()
                            ),
                            check_null: false
                        }.into(),
                    ],
                    vec![
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: Expression::Value(
                                BuiltIn::from(Boolean{value: false}).into()
                            ),
                            check_null: false
                        }.into(),
                    ],
                )
            }.into()
        ];
        "if statement value only"
    )]
    #[test_case(
        {
            let location = Location::new();
            (
                Vec::new(),
                vec![
                    IntermediateIfStatement{
                        condition: IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into(),
                        branches: (
                            vec![
                                IntermediateStatement::Assignment(
                                    IntermediateMemory {
                                        location: location.clone(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateFnCall{
                                                fn_: IntermediateBuiltIn::BuiltInFn(
                                                    Name::from("++"),
                                                    IntermediateFnType(
                                                        vec![AtomicTypeEnum::INT.into()],
                                                        Box::new(AtomicTypeEnum::INT.into())
                                                    ).into()
                                                ).into(),
                                                args: vec![IntermediateBuiltIn::from(Integer{value: 0}).into()]
                                            }.into()
                                        ))
                                    }
                                )
                            ],
                            vec![
                                IntermediateStatement::Assignment(
                                    IntermediateMemory {
                                        location: location.clone(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                        ))
                                    }
                                )
                            ]
                        )
                    }.into()
                ]
            )
        },
        vec![
            IfStatement {
                condition: BuiltIn::from(Boolean{value: true}).into(),
                branches: (
                    vec![
                        Declaration {
                            type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                            memory: Memory(Id::from("m0"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: Expression::Wrap(
                                BuiltIn::from(Integer{value: 0}).into(),
                                AtomicTypeEnum::INT.into()
                            ),
                            check_null: false
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m1")),
                            value: FnCall{
                                fn_: BuiltIn::BuiltInFn(
                                    Name::from("Increment__BuiltIn"),
                                ).into(),
                                fn_type: FnType(
                                    vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                                ),
                                args: vec![Memory(Id::from("m0")).into()]
                            }.into(),
                            check_null: true
                        }.into(),
                    ],
                    vec![
                        Declaration {
                            type_: AtomicTypeEnum::INT.into(),
                            memory: Memory(Id::from("m2"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m2")),
                            value: Value::from(
                                BuiltIn::from(Integer{value: 0})
                            ).into(),
                            check_null: false
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m1")),
                            value: Expression::Wrap(
                                Memory(Id::from("m2")).into(),
                                AtomicTypeEnum::INT.into()
                            ),
                            check_null: true
                        }.into(),
                    ],
                )
            }.into()
        ];
        "if statement value and call"
    )]
    #[test_case(
        {
            let location = Location::new();
            (
                Vec::new(),
                vec![
                    IntermediateIfStatement{
                        condition: IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into(),
                        branches: (
                            vec![
                                IntermediateStatement::Assignment(
                                    IntermediateMemory {
                                        location: location.clone(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                        ))
                                    }
                                )
                            ],
                            vec![
                                IntermediateStatement::Assignment(
                                    IntermediateMemory {
                                        location: location.clone(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateFnCall{
                                                fn_: IntermediateBuiltIn::BuiltInFn(
                                                    Name::from("++"),
                                                    IntermediateFnType(
                                                        vec![AtomicTypeEnum::INT.into()],
                                                        Box::new(AtomicTypeEnum::INT.into())
                                                    ).into()
                                                ).into(),
                                                args: vec![IntermediateBuiltIn::from(Integer{value: 0}).into()]
                                            }.into()
                                        ))
                                    }
                                )
                            ],
                        )
                    }.into(),
                    IntermediateStatement::Assignment(
                        IntermediateMemory {
                            location: Location::new(),
                            expression: Rc::new(RefCell::new(
                                IntermediateTupleExpression(
                                    vec![location.clone().into()]
                                ).into()
                            ))
                        }
                    )
                ]
            )
        },
        vec![
            IfStatement {
                condition: BuiltIn::from(Boolean{value: true}).into(),
                branches: (
                    vec![
                        Declaration {
                            type_: AtomicTypeEnum::INT.into(),
                            memory: Memory(Id::from("m2"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m2")),
                            value: Value::from(
                                BuiltIn::from(Integer{value: 0})
                            ).into(),
                            check_null: false
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: Expression::Wrap(
                                Memory(Id::from("m2")).into(),
                                AtomicTypeEnum::INT.into()
                            ),
                            check_null: true
                        }.into(),
                    ],
                    vec![
                        Declaration {
                            type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                            memory: Memory(Id::from("m1"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m1")),
                            value: Expression::Wrap(
                                BuiltIn::from(Integer{value: 0}).into(),
                                AtomicTypeEnum::INT.into()
                            ),
                            check_null: false
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: FnCall{
                                fn_: BuiltIn::BuiltInFn(
                                    Name::from("Increment__BuiltIn"),
                                ).into(),
                                fn_type: FnType(
                                    vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                                ),
                                args: vec![Memory(Id::from("m1")).into()]
                            }.into(),
                            check_null: true
                        }.into(),
                    ],
                )
            }.into(),
            Await(vec![Memory(Id::from("m0"))]).into(),
            Declaration {
                type_: AtomicTypeEnum::INT.into(),
                memory: Memory(Id::from("m3"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m3")),
                value: Expression::Unwrap(
                    Memory(Id::from("m0")).into()
                ),
                check_null: false
            }.into(),
            Declaration {
                type_: TupleType(vec![AtomicTypeEnum::INT.into()]).into(),
                memory: Memory(Id::from("m4"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m4")),
                value: TupleExpression(
                    vec![Memory(Id::from("m3")).into()]
                ).into(),
                check_null: false
            }.into(),
        ];
        "if statement value and call use"
    )]
    #[test_case(
        (
            Vec::new(),
            {
                let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
                vec![
                    IntermediateStatement::Assignment(IntermediateMemory{
                        location: Location::new(),
                        expression: Rc::new(RefCell::new(IntermediateFnDef {
                            args: vec![arg.clone()],
                            statements: Vec::new(),
                            return_value: arg.clone().into()
                        }.into()))
                    })
                ]
            }
        ),
        vec![
            Declaration {
                type_: FnType(
                    vec![
                        MachineType::Lazy(Box::new(AtomicTypeEnum::BOOL.into())),
                    ],
                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::BOOL.into())))
                ).into(),
                memory: Memory(Id::from("m0")),
            }.into(),
            Assignment {
                target: Memory(Id::from("m0")),
                value: ClosureInstantiation{
                    name: Name::from("F0"),
                    env: None
                }.into(),
                check_null: false
            }.into()
        ];
        "identity function"
    )]
    fn test_compile_statements(
        args_statements: (Vec<IntermediateArg>, Vec<IntermediateStatement>),
        expected_statements: Vec<Statement>,
    ) {
        let (args, statements) = args_statements;
        let mut lowerer = Lowerer::new();
        lowerer.compile_args(&args);
        for statement in &statements {
            if let IntermediateStatement::Assignment(IntermediateMemory {
                expression,
                location,
            }) = statement
            {
                if !lowerer.memory.contains_key(&location) {
                    lowerer.memory.insert(location.clone(), Vec::new());
                }
                lowerer
                    .memory
                    .get_mut(&location)
                    .unwrap()
                    .push(expression.clone());
            }
            if let IntermediateStatement::IntermediateIfStatement(if_statement) = statement {
                for statement in &if_statement.branches.0 {
                    if let IntermediateStatement::Assignment(IntermediateMemory {
                        expression,
                        location,
                    }) = statement
                    {
                        if !lowerer.memory.contains_key(&location) {
                            lowerer.memory.insert(location.clone(), Vec::new());
                        }
                        lowerer
                            .memory
                            .get_mut(&location)
                            .unwrap()
                            .push(expression.clone());
                    }
                }
            }
        }
        let compiled_statements = lowerer.compile_statements(statements);
        assert_eq!(compiled_statements, expected_statements);
    }
    #[test_case(
        {
            let bull_type: IntermediateType = IntermediateUnionType(vec![None,None]).into();
            let arg: IntermediateArg = IntermediateType::from(bull_type.clone()).into();
            let location = Location::new();
            (
                vec![arg.clone()],
                vec![Rc::new(RefCell::new(bull_type))],
                vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateStatement::Assignment(
                                        IntermediateMemory {
                                            location: location.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into()
                                            ))
                                        }
                                    )
                                ],
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateStatement::Assignment(
                                        IntermediateMemory {
                                            location: location.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                            ))
                                        }
                                    )
                                ]
                            }
                        ]
                    }.into()
                ]
            )
        },
        vec![
            Await(vec![Memory(Id::from("a0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: UnionType(vec![Name::from("T0C0"),Name::from("T0C1")]).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m0")),
                value: Expression::Unwrap(
                    Memory(Id::from("a0")).into()
                ),
                check_null: false
            }.into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            MatchStatement {
                expression: (
                    Memory(Id::from("m0")).into(),
                    UnionType(vec![Name::from("T0C0"),Name::from("T0C1")])
                ),
                branches: vec![
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("m1")),
                                value: Expression::Value(
                                    BuiltIn::from(Integer{value: 1}).into()
                                ),
                                check_null: false
                            }.into(),
                        ],
                    },
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("m1")),
                                value: Expression::Value(
                                    BuiltIn::from(Integer{value: 0}).into()
                                ),
                                check_null: false
                            }.into(),
                        ],
                    }
                ]
            }.into()
        ];
        "match statement no targets"
    )]
    #[test_case(
        {
            let either_type: IntermediateType = IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::BOOL.into())]).into();
            let arg: IntermediateArg = IntermediateType::from(either_type.clone()).into();
            let target0: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let target1: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            let location = Location::new();
            (
                vec![arg.clone()],
                vec![Rc::new(RefCell::new(either_type))],
                vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(target0.clone()),
                                statements: vec![
                                    IntermediateStatement::Assignment(
                                        IntermediateMemory {
                                            location: location.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    fn_: IntermediateBuiltIn::BuiltInFn(
                                                        Name::from(">"),
                                                        IntermediateFnType(
                                                            vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                                                            Box::new(AtomicTypeEnum::BOOL.into())
                                                        ).into()
                                                    ).into(),
                                                    args: vec![
                                                        target0.into(),
                                                        IntermediateBuiltIn::from(Integer{value: 0}).into()
                                                    ]
                                                }.into()
                                            ))
                                        }
                                    )
                                ],
                            },
                            IntermediateMatchBranch{
                                target: Some(target1.clone()),
                                statements: vec![
                                    IntermediateStatement::Assignment(
                                        IntermediateMemory {
                                            location: location.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(target1).into()
                                            ))
                                        }
                                    )
                                ]
                            }
                        ]
                    }.into()
                ]
            )
        },
        vec![
            Await(vec![Memory(Id::from("a0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: UnionType(vec![Name::from("T0C0"),Name::from("T0C1")]).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m0")),
                value: Expression::Unwrap(
                    Memory(Id::from("a0")).into()
                ),
                check_null: false
            }.into(),
            MatchStatement {
                expression: (
                    Memory(Id::from("m0")).into(),
                    UnionType(vec![Name::from("T0C0"),Name::from("T0C1")])
                ),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("a1"))),
                        statements: vec![
                            Declaration {
                                memory: Memory(Id::from("m1")),
                                type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m1")),
                                value: Expression::Wrap(
                                    BuiltIn::from(Integer{value: 0}).into(),
                                    AtomicTypeEnum::INT.into()
                                ),
                                check_null: false
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m2")),
                                value: FnCall{
                                    fn_: BuiltIn::BuiltInFn(
                                        Name::from("Comparison_GT__BuiltIn"),
                                    ).into(),
                                    fn_type: FnType(
                                        vec![
                                            MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                                            MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                                        ],
                                        Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::BOOL.into())))
                                    ),
                                    args: vec![
                                        Memory(Id::from("a1")).into(),
                                        Memory(Id::from("m1")).into(),
                                    ]
                                }.into(),
                                check_null: true
                            }.into(),
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("a2"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("a2"))]).into(),
                            Declaration {
                                memory: Memory(Id::from("m3")),
                                type_: AtomicTypeEnum::BOOL.into()
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m3")),
                                value: Expression::Unwrap(
                                    Memory(Id::from("a2")).into()
                                ),
                                check_null: false
                            }.into(),
                            Declaration {
                                memory: Memory(Id::from("m4")),
                                type_: AtomicTypeEnum::BOOL.into()
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m4")),
                                value: Expression::Value(
                                    Memory(Id::from("m3")).into()
                                ),
                                check_null: false
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m2")),
                                value: Expression::Wrap(
                                    Memory(Id::from("m4")).into(),
                                    AtomicTypeEnum::BOOL.into()
                                ),
                                check_null: true
                            }.into(),
                        ],
                    }
                ]
            }.into()
        ];
        "match statement with targets"
    )]
    #[test_case(
        {
            let either_type: IntermediateType = IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::BOOL.into())]).into();
            let arg: IntermediateArg = IntermediateType::from(either_type.clone()).into();
            let target0: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let target1: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            let location = Location::new();
            (
                vec![arg.clone()],
                vec![Rc::new(RefCell::new(either_type))],
                vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(target0.clone()),
                                statements: vec![
                                    IntermediateStatement::Assignment(
                                        IntermediateMemory {
                                            location: location.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    fn_: IntermediateBuiltIn::BuiltInFn(
                                                        Name::from(">"),
                                                        IntermediateFnType(
                                                            vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                                                            Box::new(AtomicTypeEnum::BOOL.into())
                                                        ).into()
                                                    ).into(),
                                                    args: vec![
                                                        target0.into(),
                                                        IntermediateBuiltIn::from(Integer{value: 0}).into()
                                                    ]
                                                }.into()
                                            ))
                                        }
                                    )
                                ],
                            },
                            IntermediateMatchBranch{
                                target: Some(target1.clone()),
                                statements: vec![
                                    IntermediateStatement::Assignment(
                                        IntermediateMemory {
                                            location: location.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(target1).into()
                                            ))
                                        }
                                    )
                                ]
                            }
                        ]
                    }.into(),
                    IntermediateStatement::Assignment(
                        IntermediateMemory {
                            location: Location::new(),
                            expression: Rc::new(RefCell::new(
                                IntermediateTupleExpression(
                                    vec![location.clone().into(), IntermediateBuiltIn::from(Integer{value: 0}).into()]
                                ).into()
                            ))
                        }
                    ),
                    IntermediateStatement::Assignment(
                        IntermediateMemory {
                            location: Location::new(),
                            expression: Rc::new(RefCell::new(
                                IntermediateTupleExpression(
                                    vec![location.clone().into(), IntermediateBuiltIn::from(Integer{value: 1}).into()]
                                ).into()
                            ))
                        }
                    )
                ]
            )
        },
        vec![
            Await(vec![Memory(Id::from("a0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m0")),
                type_: UnionType(vec![Name::from("T0C0"),Name::from("T0C1")]).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m0")),
                value: Expression::Unwrap(
                    Memory(Id::from("a0")).into()
                ),
                check_null: false
            }.into(),
            MatchStatement {
                expression: (
                    Memory(Id::from("m0")).into(),
                    UnionType(vec![Name::from("T0C0"),Name::from("T0C1")])
                ),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("a1"))),
                        statements: vec![
                            Declaration {
                                memory: Memory(Id::from("m1")),
                                type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m1")),
                                value: Expression::Wrap(
                                    BuiltIn::from(Integer{value: 0}).into(),
                                    AtomicTypeEnum::INT.into()
                                ),
                                check_null: false
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m2")),
                                value: FnCall{
                                    fn_: BuiltIn::BuiltInFn(
                                        Name::from("Comparison_GT__BuiltIn"),
                                    ).into(),
                                    fn_type: FnType(
                                        vec![
                                            MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                                            MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                                        ],
                                        Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::BOOL.into())))
                                    ),
                                    args: vec![
                                        Memory(Id::from("a1")).into(),
                                        Memory(Id::from("m1")).into(),
                                    ]
                                }.into(),
                                check_null: true
                            }.into(),
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("a2"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("a2"))]).into(),
                            Declaration {
                                memory: Memory(Id::from("m3")),
                                type_: AtomicTypeEnum::BOOL.into()
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m3")),
                                value: Expression::Unwrap(
                                    Memory(Id::from("a2")).into()
                                ),
                                check_null: false
                            }.into(),
                            Declaration {
                                memory: Memory(Id::from("m4")),
                                type_: AtomicTypeEnum::BOOL.into()
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m4")),
                                value: Expression::Value(
                                    Memory(Id::from("m3")).into()
                                ),
                                check_null: false
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m2")),
                                value: Expression::Wrap(
                                    Memory(Id::from("m4")).into(),
                                    AtomicTypeEnum::BOOL.into()
                                ),
                                check_null: true
                            }.into(),
                        ],
                    }
                ]
            }.into(),
            Await(vec![Memory(Id::from("m2"))]).into(),
            Declaration {
                type_: AtomicTypeEnum::BOOL.into(),
                memory: Memory(Id::from("m5"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m5")),
                value: Expression::Unwrap(
                    Memory(Id::from("m2")).into()
                ),
                check_null: false
            }.into(),
            Declaration {
                type_: TupleType(vec![AtomicTypeEnum::BOOL.into(),AtomicTypeEnum::INT.into()]).into(),
                memory: Memory(Id::from("m6"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m6")),
                value: TupleExpression(
                    vec![Memory(Id::from("m5")).into(),BuiltIn::from(Integer{value: 0}).into()]
                ).into(),
                check_null: false
            }.into(),
            Declaration {
                type_: TupleType(vec![AtomicTypeEnum::BOOL.into(),AtomicTypeEnum::INT.into()]).into(),
                memory: Memory(Id::from("m7"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m7")),
                value: TupleExpression(
                    vec![Memory(Id::from("m5")).into(),BuiltIn::from(Integer{value: 1}).into()]
                ).into(),
                check_null: false
            }.into(),
        ];
        "match statement with targets and use"
    )]
    fn test_compile_match_statements(
        args_types_statements: (
            Vec<IntermediateArg>,
            Vec<Rc<RefCell<IntermediateType>>>,
            Vec<IntermediateStatement>,
        ),
        expected_statements: Vec<Statement>,
    ) {
        let (args, types, statements) = args_types_statements;
        let mut lowerer = Lowerer::new();
        lowerer.compile_args(&args);
        lowerer.compile_type_defs(types);
        for statement in &statements {
            if let IntermediateStatement::Assignment(IntermediateMemory {
                expression,
                location,
            }) = statement
            {
                if !lowerer.memory.contains_key(&location) {
                    lowerer.memory.insert(location.clone(), Vec::new());
                }
                lowerer
                    .memory
                    .get_mut(&location)
                    .unwrap()
                    .push(expression.clone());
            }
            if let IntermediateStatement::IntermediateMatchStatement(match_statement) = statement {
                for statement in &match_statement.branches[0].statements {
                    if let IntermediateStatement::Assignment(IntermediateMemory {
                        expression,
                        location,
                    }) = statement
                    {
                        if !lowerer.memory.contains_key(&location) {
                            lowerer.memory.insert(location.clone(), Vec::new());
                        }
                        lowerer
                            .memory
                            .get_mut(&location)
                            .unwrap()
                            .push(expression.clone());
                    }
                }
            }
        }
        let compiled_statements = lowerer.compile_statements(statements);
        assert_eq!(compiled_statements, expected_statements);
    }

    #[test_case(
        {
            let arg0: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let arg1: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let y = Location::new();
            let y_expression = Rc::new(RefCell::new(IntermediateFnCall{
                fn_: IntermediateBuiltIn::BuiltInFn(
                    Name::from("+"),
                    IntermediateFnType(
                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into())
                    ).into()
                ).into(),
                args: vec![
                    arg0.clone().into(),
                    arg1.clone().into(),
                ]
            }.into()));
            (
                vec![
                    (y.clone(), y_expression.clone())
                ],
                IntermediateFnDef {
                    args: vec![arg0.clone(), arg1.clone()],
                    statements: vec![
                        IntermediateStatement::Assignment(IntermediateMemory{
                            location: y.clone(),
                            expression: y_expression,
                        })
                    ],
                    return_value: y.into()
                }
            )
        },
        (
            Vec::new(),
            ClosureInstantiation{
                name: Name::from("F0"),
                env: None
            },
            FnDef{
                name: Name::from("F0"),
                arguments: vec![
                    (Memory(Id::from("a0")), MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))),
                    (Memory(Id::from("a1")), MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))),
                ],
                env: None,
                statements: vec![
                    Assignment{
                        target: Memory(Id::from("m0")),
                        value: FnCall{
                            fn_: BuiltIn::BuiltInFn(
                                Name::from("Plus__BuiltIn"),
                            ).into(),
                            fn_type: FnType(
                                vec![
                                    MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                                    MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                                ],
                                Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                            ),
                            args: vec![
                                Memory(Id::from("a0")).into(),
                                Memory(Id::from("a1")).into(),
                            ]
                        }.into(),
                        check_null: true
                    }.into()
                ],
                ret: (
                    Memory(Id::from("m0")).into(),
                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                ),
                allocations: vec![
                    Declaration {
                        memory: Memory(Id::from("m0")),
                        type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                    }.into(),
                ]
            }
        );
        "env-free closure"
    )]
    #[test_case(
        {
            let x = Location::new();
            let y = Location::new();
            let z = Location::new();
            let z_expression = Rc::new(RefCell::new(IntermediateFnCall{
                fn_: IntermediateBuiltIn::BuiltInFn(
                    Name::from("+"),
                    IntermediateFnType(
                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into())
                    ).into()
                ).into(),
                args: vec![
                    x.clone().into(),
                    y.clone().into()
                ]
            }.into()));
            (
                vec![
                    (x.clone(), Rc::new(RefCell::new(IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 3})).into()))),
                    (y.clone(), Rc::new(RefCell::new(IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 4})).into()))),
                    (z.clone(), z_expression.clone()),
                ],
                IntermediateFnDef {
                    args: Vec::new(),
                    statements: vec![
                        IntermediateStatement::Assignment(IntermediateMemory{
                            location: z.clone(),
                            expression: z_expression,
                        })
                    ],
                    return_value: z.into()
                }
            )
        },
        (
            vec![
                Declaration {
                    memory: Memory(Id::from("m5")),
                    type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                }.into(),
                Assignment {
                    target: Memory(Id::from("m5")),
                    check_null: false,
                    value: Expression::Wrap(
                        Memory(Id::from("m4")).into(),
                        AtomicTypeEnum::INT.into()
                    )
                }.into(),
                Declaration {
                    memory: Memory(Id::from("m7")),
                    type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                }.into(),
                Assignment {
                    target: Memory(Id::from("m7")),
                    check_null: false,
                    value: Expression::Wrap(
                        Memory(Id::from("m6")).into(),
                        AtomicTypeEnum::INT.into()
                    )
                }.into(),
                Declaration {
                    memory: Memory(Id::from("m3")),
                    type_: TupleType(vec![
                        MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                        MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                    ]).into()
                }.into(),
                Assignment {
                    target: Memory(Id::from("m3")),
                    check_null: false,
                    value: TupleExpression(vec![
                        Memory(Id::from("m5")).into(),
                        Memory(Id::from("m7")).into(),
                    ]).into()
                }.into(),
            ],
            ClosureInstantiation{
                name: Name::from("F0"),
                env: Some(Memory(Id::from("m3")).into())
            },
            FnDef{
                name: Name::from("F0"),
                arguments: Vec::new(),
                env: Some(TupleType(vec![
                    MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                    MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                ]).into()),
                statements: vec![
                    Declaration {
                        memory: Memory(Id::from("m0")),
                        type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                    }.into(),
                    Assignment {
                        target: Memory(Id::from("m0")),
                        check_null: false,
                        value: ElementAccess{
                            value: Memory(Id::from("env")).into(),
                            idx: 0
                        }.into()
                    }.into(),
                    Declaration {
                        memory: Memory(Id::from("m1")),
                        type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                    }.into(),
                    Assignment {
                        target: Memory(Id::from("m1")),
                        check_null: false,
                        value: ElementAccess{
                            value: Memory(Id::from("env")).into(),
                            idx: 1
                        }.into()
                    }.into(),
                    Assignment{
                        target: Memory(Id::from("m2")),
                        value: FnCall{
                            fn_: BuiltIn::BuiltInFn(
                                Name::from("Plus__BuiltIn"),
                            ).into(),
                            fn_type: FnType(
                                vec![
                                    MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                                    MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                                ],
                                Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                            ),
                            args: vec![
                                Memory(Id::from("m0")).into(),
                                Memory(Id::from("m1")).into(),
                            ]
                        }.into(),
                        check_null: true
                    }.into()
                ],
                ret: (
                    Memory(Id::from("m2")).into(),
                    MachineType::Lazy(Box::new(AtomicType(AtomicTypeEnum::INT).into()))
                ),
                allocations: vec![
                    Declaration {
                        memory: Memory(Id::from("m2")),
                        type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                    }.into(),
                ]
            }
        );
        "env closure"
    )]
    fn test_compile_fn_defs(
        locations_fn_defs: (
            Vec<(Location, Rc<RefCell<IntermediateExpression>>)>,
            IntermediateFnDef,
        ),
        expected: (Vec<Statement>, ClosureInstantiation, FnDef),
    ) {
        let (locations, fn_def) = locations_fn_defs;
        let (expected_statements, expected_value, expected_fn_def) = expected;

        let mut lowerer = Lowerer::new();
        for (location, expression) in locations {
            if !lowerer.memory.contains_key(&location) {
                lowerer.memory.insert(location.clone(), Vec::new());
            }
            lowerer
                .memory
                .get_mut(&location)
                .unwrap()
                .push(expression.clone());
        }

        let compiled = lowerer.compile_fn_def(fn_def);
        assert_eq!(compiled, (expected_statements, expected_value));
        let compiled_fn_def = &lowerer.fn_defs[0];
        assert_eq!(compiled_fn_def, &expected_fn_def);
    }
}
