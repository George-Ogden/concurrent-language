use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    AllocationState, Assignment, AtomicTypeEnum, Await, BuiltIn, ClosureInstantiation,
    ConstructorCall, Declaration, ElementAccess, Expression, FnCall, FnDef, FnType, Id,
    IfStatement, MachineType, MatchBranch, MatchStatement, Memory, Name, Program, Statement,
    TupleExpression, TupleType, TypeDef, UnionType, Value,
};
use itertools::{Either, Itertools};
use lowering::*;
use once_cell::sync::Lazy;

const OPERATOR_NAMES: Lazy<HashMap<Id, Id>> = Lazy::new(|| {
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
            ("!", "Negation__BuiltIn"),
        ]
        .into_iter()
        .map(|(op, name)| (Id::from(op), Id::from(name))),
    )
});

type MemoryMap = HashMap<Location, Vec<IntermediateExpression>>;
type ReferenceNames = HashMap<*mut IntermediateType, MachineType>;
type MemoryIds = HashMap<Location, Memory>;
type ValueScope = HashMap<IntermediateValue, Value>;
type TypeLookup = HashMap<IntermediateUnionType, UnionType>;
type FnDefs = Vec<FnDef>;

pub struct Compiler {
    memory: MemoryMap,
    reference_names: ReferenceNames,
    memory_ids: MemoryIds,
    lazy_vals: ValueScope,
    non_lazy_vals: ValueScope,
    type_lookup: TypeLookup,
    fn_defs: FnDefs,
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            memory: MemoryMap::new(),
            reference_names: ReferenceNames::new(),
            memory_ids: MemoryIds::new(),
            lazy_vals: ValueScope::new(),
            non_lazy_vals: ValueScope::new(),
            type_lookup: TypeLookup::new(),
            fn_defs: FnDefs::new(),
        }
    }

    fn update_memory(&mut self, memory: &IntermediateAssignment) {
        let values = self
            .memory
            .entry(memory.location.clone())
            .or_insert(Vec::new());
        values.push(memory.expression.clone());
    }
    fn register_memory(&mut self, statements: &Vec<IntermediateStatement>) {
        for statement in statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    location,
                }) => {
                    match &expression {
                        IntermediateExpression::IntermediateLambda(IntermediateLambda {
                            args: _,
                            statements,
                            ret: _,
                        }) => {
                            self.register_memory(statements);
                        }
                        _ => {}
                    }
                    if !self.memory.contains_key(&location) {
                        self.memory.insert(location.clone(), Vec::new());
                    }
                    self.memory
                        .get_mut(&location)
                        .unwrap()
                        .push(expression.clone());
                }
                IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                    condition: _,
                    branches,
                }) => {
                    self.register_memory(&branches.0);
                    self.register_memory(&branches.1);
                }
                IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                    subject: _,
                    branches,
                }) => {
                    for branch in branches {
                        self.register_memory(&branch.statements)
                    }
                }
            }
        }
    }

    fn compile_type(&self, type_: &IntermediateType) -> MachineType {
        match type_ {
            IntermediateType::AtomicType(atomic_type) => {
                let atomic_type_enum = atomic_type.0;
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
        let machine_types = types
            .iter()
            .enumerate()
            .map(|(i, (_, IntermediateUnionType(types)))| {
                let names = types
                    .iter()
                    .enumerate()
                    .map(|(j, _)| format!("T{i}C{j}"))
                    .collect_vec();
                let intermediate_type = UnionType(names);
                self.type_lookup.insert(
                    IntermediateUnionType(types.clone()),
                    intermediate_type.clone(),
                );
                (format!("T{i}"), intermediate_type)
            })
            .collect_vec();
        types
            .into_iter()
            .zip(machine_types.into_iter())
            .map(
                |((_, IntermediateUnionType(types)), (type_name, machine_type))| {
                    let UnionType(ctor_names) = machine_type;
                    let constructors = types
                        .into_iter()
                        .zip(ctor_names.into_iter())
                        .map(|(type_, name)| {
                            (name, type_.as_ref().map(|type_| self.compile_type(type_)))
                        })
                        .collect_vec();
                    TypeDef {
                        name: type_name,
                        constructors,
                    }
                },
            )
            .collect_vec()
    }

    fn next_memory_address(&self) -> Memory {
        Memory(format!("m{}", self.memory_ids.len()))
    }
    fn compile_location(&mut self, location: &Location) -> Memory {
        if !self.memory_ids.contains_key(location) {
            self.memory_ids
                .insert(location.clone(), self.next_memory_address());
        }
        self.memory_ids[location].clone().into()
    }
    fn compile_arg(&mut self, arg: &IntermediateArg) -> Memory {
        self.compile_location(&arg.location)
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
                let type_ = self.compile_type(&value.type_());
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
            IntermediateValue::IntermediateArg(arg) => {
                if lazy {
                    (Vec::new(), self.compile_arg(arg).into())
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
                        None => (Vec::new(), self.compile_location(location).into()),
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
                            IntermediateBuiltIn::BuiltInFn(BuiltInFn(name, _)) => {
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
                let (statements, values) = self.compile_values(values, false);
                (statements, TupleExpression(values).into())
            }
            IntermediateExpression::IntermediateElementAccess(IntermediateElementAccess {
                value,
                idx,
            }) => {
                let (statements, value) = self.compile_value(value, false);
                (statements, ElementAccess { value, idx }.into())
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
            IntermediateExpression::IntermediateLambda(lambda) => {
                let (statements, closure_inst) = self.compile_lambda(lambda);
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
        let vals = (self.non_lazy_vals.clone(), self.lazy_vals.clone());
        let true_branch = self.compile_statements(true_branch);
        (self.non_lazy_vals, self.lazy_vals) = vals.clone();
        let false_branch = self.compile_statements(false_branch);
        (self.non_lazy_vals, self.lazy_vals) = vals.clone();
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
        let vals = (self.non_lazy_vals.clone(), self.lazy_vals.clone());
        let branches = branches
            .into_iter()
            .map(|IntermediateMatchBranch { target, statements }| {
                let branch = MatchBranch {
                    target: target.map(|arg| self.compile_arg(&arg)),
                    statements: self.compile_statements(statements),
                };
                (self.non_lazy_vals, self.lazy_vals) = vals.clone();
                branch
            })
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
            IntermediateStatement::IntermediateAssignment(memory) => {
                self.compile_assignment(memory)
            }
            IntermediateStatement::IntermediateIfStatement(if_statement) => {
                self.compile_if_statement(if_statement)
            }
            IntermediateStatement::IntermediateMatchStatement(match_statement) => {
                self.compile_match_statement(match_statement)
            }
        }
    }
    fn compile_assignment(&mut self, assignment: IntermediateAssignment) -> Vec<Statement> {
        let IntermediateAssignment {
            expression,
            location,
        } = assignment;
        let type_ = self.compile_type(&self.expression_type(&expression));
        let (mut statements, value) = self.compile_expression(expression);
        let memory = self.compile_location(&location);
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
        fn_def: &mut IntermediateLambda,
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
            .map(|(val, loc)| {
                self.update_memory(&IntermediateAssignment {
                    location: loc.clone(),
                    expression: val.clone().into(),
                });
                (val.clone(), loc.clone())
            })
            .collect()
    }
    fn closure_prefix(&mut self, env_types: &Vec<(Location, MachineType)>) -> Vec<Statement> {
        env_types
            .iter()
            .enumerate()
            .flat_map(|(i, (location, type_))| {
                let memory = self.compile_location(location);
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
    fn compile_lambda(
        &mut self,
        mut lambda: IntermediateLambda,
    ) -> (Vec<Statement>, ClosureInstantiation) {
        let env_mapping = self.replace_open_vars(&mut lambda);
        let env_types = env_mapping
            .iter()
            .map(|(value, location)| {
                (
                    location.clone(),
                    MachineType::Lazy(Box::new(self.compile_type(&self.value_type(&value)))),
                )
            })
            .collect_vec();

        let vals = (self.non_lazy_vals.clone(), self.lazy_vals.clone());
        self.lazy_vals = HashMap::new();
        self.non_lazy_vals = HashMap::new();
        let IntermediateLambda {
            args,
            statements,
            ret: return_value,
        } = lambda;
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
        let ret_type = MachineType::Lazy(Box::new(self.compile_type(&return_type)));
        let (extra_statements, ret_val) = self.compile_value(return_value, true);
        statements.extend(extra_statements);
        (self.non_lazy_vals, self.lazy_vals) = vals;
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
    fn compile_program(&mut self, program: IntermediateProgram) -> Program {
        let IntermediateProgram {
            mut statements,
            main,
            types,
        } = program;
        self.register_memory(&statements);
        let type_defs = self.compile_type_defs(types);
        let call = Location::new();
        let assignment = IntermediateAssignment {
            expression: IntermediateFnCall {
                fn_: main.clone().into(),
                args: Vec::new(),
            }
            .into(),
            location: call.clone(),
        };
        self.update_memory(&assignment);
        let IntermediateType::IntermediateFnType(IntermediateFnType(_, return_type)) =
            self.value_type(&main)
        else {
            panic!("Main has non-fn type")
        };
        statements.push(IntermediateStatement::IntermediateAssignment(assignment));
        let (statements, _) = self.compile_lambda(IntermediateLambda {
            args: Vec::new(),
            ret: (call.clone().into(), *return_type),
            statements: statements,
        });
        assert_eq!(statements.len(), 0);
        self.fn_defs.last_mut().unwrap().name = Name::from("Main");
        Program {
            fn_defs: self.fn_defs.clone(),
            type_defs,
        }
    }
    pub fn compile(program: IntermediateProgram) -> Program {
        let mut compiler = Compiler::new();
        compiler.compile_program(program)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use crate::AtomicType;
    use lowering::{Boolean, Integer};
    use test_case::test_case;

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
            BuiltInFn(
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
                    vec![IntermediateBuiltIn::from(Integer{value: 8}).into()]
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
                        IntermediateBuiltIn::from(Integer{value: 8}).into(),
                        IntermediateBuiltIn::from(Integer{value: -8}).into(),
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
        let mut compiler = Compiler::new();
        compiler.memory = memory_map;
        assert_eq!(compiler.value_type(&value), type_);
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
            IntermediateLambda{
                args: Vec::new(),
                statements: Vec::new(),
                ret: (IntermediateBuiltIn::from(Integer{value: 5}).into(), AtomicTypeEnum::INT.into())
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
                IntermediateLambda{
                    args: args.clone(),
                    statements: Vec::new(),
                    ret: (args[1].clone().into(), args[1].type_.clone())
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
                fn_: BuiltInFn(
                    Name::from(""),
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
                    vec![
                        IntermediateTupleExpression(
                            vec![
                                IntermediateBuiltIn::from(Integer{value: 4}).into(),
                                IntermediateBuiltIn::from(Boolean{value: false}).into(),
                            ]
                        ).into(),
                    ]
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
        let mut compiler = Compiler::new();
        compiler.memory = memory_map;
        assert_eq!(compiler.expression_type(&expression), type_);
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
        let mut compiler = Compiler::new();
        assert_eq!(compiler.compile_type_defs(type_defs), expected_type_defs)
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
        BuiltInFn(
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
        let mut compiler = Compiler::new();
        let (_, compiled_value) = compiler.compile_value(value, false);
        assert_eq!(compiled_value, expected_value);
    }
    #[test]
    fn test_compile_multiple_memory_locations() {
        let locations = vec![Location::new(), Location::new(), Location::new()];
        let mut compiler = Compiler::new();
        let value_0 = compiler.compile_value(locations[0].clone().into(), false);
        let value_1 = compiler.compile_value(locations[1].clone().into(), false);
        let value_2 = compiler.compile_value(locations[2].clone().into(), false);
        assert_ne!(value_0, value_1);
        assert_ne!(value_2, value_1);
        assert_ne!(value_2, value_0);

        assert_eq!(
            value_0,
            compiler.compile_value(locations[0].clone().into(), false)
        );
        assert_eq!(
            value_1,
            compiler.compile_value(locations[1].clone().into(), false)
        );
        assert_eq!(
            value_2,
            compiler.compile_value(locations[2].clone().into(), false)
        );
    }
    #[test]
    fn test_compile_arguments() {
        let types: Vec<IntermediateType> = vec![
            AtomicTypeEnum::INT.into(),
            AtomicTypeEnum::BOOL.into(),
            AtomicTypeEnum::INT.into(),
        ];
        let mut compiler = Compiler::new();
        for type_ in &types {
            compiler.compile_arg(&type_.clone().into());
        }

        let args = types
            .into_iter()
            .map(|type_| IntermediateArg::from(type_))
            .collect_vec();
        let value_0 = compiler.compile_value(args[0].clone().into(), true);
        let value_1 = compiler.compile_value(args[1].clone().into(), true);
        let value_2 = compiler.compile_value(args[2].clone().into(), true);
        assert_ne!(value_0, value_1);
        assert_ne!(value_2, value_1);
        assert_ne!(value_2, value_0);

        assert_eq!(
            value_0,
            compiler.compile_value(args[0].clone().into(), true)
        );
        assert_eq!(
            value_1,
            compiler.compile_value(args[1].clone().into(), true)
        );
        assert_eq!(
            value_2,
            compiler.compile_value(args[2].clone().into(), true)
        );
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
                Await(vec![Memory(Id::from("m0"))]).into(),
                Declaration{
                    memory: Memory(Id::from("m1")),
                    type_: AtomicTypeEnum::INT.into()
                }.into(),
                Assignment{
                    check_null: false,
                    target: Memory(Id::from("m1")),
                    value: Expression::Unwrap(Memory(Id::from("m0")).into())
                }.into()
            ],
            TupleExpression(vec![
                Memory(Id::from("m1")).into()
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
                Await(vec![Memory(Id::from("m0"))]).into(),
                Declaration{
                    type_: AtomicTypeEnum::INT.into(),
                    memory: Memory(Id::from("m1"))
                }.into(),
                Assignment{
                    check_null: false,
                    target: Memory(Id::from("m1")),
                    value: Expression::Unwrap(Memory(Id::from("m0")).into())
                }.into(),
            ],
            TupleExpression(vec![
                Memory(Id::from("m1")).into(),
                Memory(Id::from("m1")).into(),
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
                Await(vec![Memory(Id::from("m0"))]).into(),
                Declaration {
                    type_: TupleType(vec![
                        AtomicTypeEnum::INT.into(),
                        AtomicTypeEnum::BOOL.into(),
                    ]).into(),
                    memory: Memory(Id::from("m1"))
                }.into(),
                Assignment{
                    check_null: false,
                    target: Memory(Id::from("m1")),
                    value: Expression::Unwrap(Memory(Id::from("m0")).into())
                }.into()
            ],
            ElementAccess{
                value: Memory(Id::from("m1")).into(),
                idx: 1
            }.into()
        );
        "argument element access"
    )]
    #[test_case(
        IntermediateFnCall{
            fn_: BuiltInFn(
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
            fn_: BuiltInFn(
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
                Await(vec![Memory(Id::from("m0"))]).into(),
                Declaration {
                    type_: FnType(
                        vec![
                            MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                        ],
                        Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::BOOL.into())))
                    ).into(),
                    memory: Memory(Id::from("m1"))
                }.into(),
                Assignment{
                    check_null: false,
                    target: Memory(Id::from("m1")),
                    value: Expression::Unwrap(
                        Memory(Id::from("m0")).into()
                    )
                }.into()
            ],
            FnCall{
                args: vec![
                    Memory(Id::from("m2")).into(),
                ],
                fn_: Memory(Id::from("m1")).into(),
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
        let mut compiler = Compiler::new();
        let result = compiler.compile_expression(expression);
        assert_eq!(result, expected);
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
        let mut compiler = Compiler::new();
        compiler.compile_type_defs(vec![type_]);
        let result = compiler.compile_expression(constructor.into());
        assert_eq!(result, expected);
    }

    #[test_case(
        vec![
            IntermediateAssignment{
                expression:
                    IntermediateTupleExpression(vec![
                        IntermediateBuiltIn::from(Integer{value: 5}).into(),
                        IntermediateBuiltIn::from(Boolean{value: false}).into(),
                    ]).into()
                ,
                location: Location::new()
            }.into()
        ],
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
        vec![
            IntermediateAssignment{
                expression:
                    IntermediateElementAccess{
                        idx: 1,
                        value: IntermediateArg::from(
                            IntermediateType::from(
                                IntermediateTupleType(vec![
                                    AtomicTypeEnum::INT.into(),
                                    AtomicTypeEnum::BOOL.into(),
                                ])
                            )
                        ).into()
                    }.into()
                ,
                location: Location::new()
            }.into()
        ],
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: TupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    AtomicTypeEnum::BOOL.into(),
                ]).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: Expression::Unwrap(Memory(Id::from("m0")).into()),
                check_null: false
            }.into(),
            Declaration {
                memory: Memory(Id::from("m2")),
                type_: AtomicTypeEnum::BOOL.into(),
            }.into(),
            Assignment {
                target: Memory(Id::from("m2")),
                value: ElementAccess{
                    idx: 1,
                    value: Memory(Id::from("m1")).into(),
                }.into(),
                check_null: false
            }.into()
        ];
        "tuple access assignment"
    )]
    #[test_case(
        vec![
            IntermediateAssignment{
                expression: IntermediateFnCall{
                    fn_: BuiltInFn(
                        Name::from("--"),
                        IntermediateFnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into())
                        ).into()
                    ).into(),
                    args: vec![
                        IntermediateBuiltIn::from(Integer{value: 11}).into()
                    ]
                }.into(),
                location: Location::new()
            }.into()
        ],
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
            vec![
                IntermediateAssignment{
                    expression:
                        IntermediateTupleExpression(vec![
                            IntermediateBuiltIn::from(Integer{value: 5}).into(),
                        ]).into()
                    ,
                    location: tuple.clone()
                }.into(),
                IntermediateAssignment{
                    expression: IntermediateFnCall{
                        fn_: arg_0.into(),
                        args: vec![tuple.clone().into()]
                    }.into(),
                    location: Location::new()
                }.into(),
                IntermediateAssignment{
                    expression: IntermediateFnCall{
                        fn_: arg_1.into(),
                        args: vec![tuple.clone().into()]
                    }.into(),
                    location: Location::new()
                }.into()
            ]
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
            Await(vec![Memory(Id::from("m1"))]).into(),
            Declaration {
                memory: Memory(Id::from("m2")),
                type_: FnType(
                    vec![MachineType::Lazy(Box::new(
                        TupleType(vec![AtomicTypeEnum::INT.into()]).into()
                    ))],
                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                ).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m2")),
                value: Expression::Unwrap(
                    Memory(Id::from("m1")).into()
                ),
                check_null: false
            }.into(),
            Declaration {
                memory: Memory(Id::from("m3")),
                type_: MachineType::Lazy(Box::new(TupleType(vec![
                    AtomicTypeEnum::INT.into(),
                ]).into()))
            }.into(),
            Assignment {
                target: Memory(Id::from("m3")),
                value: Expression::Wrap(
                    Memory(Id::from("m0")).into(),
                    TupleType(vec![
                        AtomicTypeEnum::INT.into(),
                    ]).into()
                ),
                check_null: false
            }.into(),
            Assignment {
                target: Memory(Id::from("m4")),
                value: FnCall{
                    fn_: Memory(Id::from("m2")).into(),
                    args: vec![
                        Memory(Id::from("m3")).into(),
                    ],
                    fn_type: FnType(
                        vec![MachineType::Lazy(Box::new(TupleType(vec![AtomicTypeEnum::INT.into()]).into()))],
                        Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                    )
                }.into(),
                check_null: true
            }.into(),
            Await(vec![Memory(Id::from("m5"))]).into(),
            Declaration {
                memory: Memory(Id::from("m6")),
                type_: FnType(
                    vec![MachineType::Lazy(Box::new(
                        TupleType(vec![AtomicTypeEnum::INT.into()]).into()
                    ))],
                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                ).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m6")),
                value: Expression::Unwrap(
                    Memory(Id::from("m5")).into()
                ),
                check_null: false
            }.into(),
            Assignment {
                target: Memory(Id::from("m7")),
                value: FnCall{
                    fn_: Memory(Id::from("m6")).into(),
                    args: vec![
                        Memory(Id::from("m3")).into(),
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
            vec![
                IntermediateIfStatement{
                    condition: arg.into(),
                    branches: (
                        vec![
                            IntermediateAssignment {
                                location: location.clone(),
                                expression:
                                    IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into()

                            }.into()
                        ],
                        vec![
                            IntermediateAssignment {
                                location: location.clone(),
                                expression:
                                    IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()

                            }.into()
                        ]
                    )
                }.into()
            ]
        },
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: AtomicTypeEnum::BOOL.into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: Expression::Unwrap(
                    Memory(Id::from("m0")).into()
                ),
                check_null: false
            }.into(),
            Declaration {
                memory: Memory(Id::from("m2")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            IfStatement {
                condition: Memory(Id::from("m1")).into(),
                branches: (
                    vec![
                        Assignment {
                            target: Memory(Id::from("m2")),
                            value: Expression::Value(
                                BuiltIn::from(Integer{value: 1}).into()
                            ),
                            check_null: false
                        }.into(),
                    ],
                    vec![
                        Assignment {
                            target: Memory(Id::from("m2")),
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
            vec![
                IntermediateIfStatement{
                    condition: IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into(),
                    branches: (
                        vec![
                            IntermediateAssignment {
                                location: location.clone(),
                                expression:
                                    IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into()

                            }.into()
                        ],
                        vec![
                            IntermediateAssignment {
                                location: location.clone(),
                                expression:
                                    IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: false})).into()

                            }.into()
                        ]
                    )
                }.into()
            ]
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
            vec![
                IntermediateIfStatement{
                    condition: IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into(),
                    branches: (
                        vec![
                            IntermediateAssignment {
                                location: location.clone(),
                                expression:
                                    IntermediateFnCall{
                                        fn_: BuiltInFn(
                                            Name::from("++"),
                                            IntermediateFnType(
                                                vec![AtomicTypeEnum::INT.into()],
                                                Box::new(AtomicTypeEnum::INT.into())
                                            ).into()
                                        ).into(),
                                        args: vec![IntermediateBuiltIn::from(Integer{value: 0}).into()]
                                    }.into()

                            }.into()
                        ],
                        vec![
                            IntermediateAssignment {
                                location: location.clone(),
                                expression:
                                    IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()

                            }.into()
                        ]
                    )
                }.into()
            ]
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
            vec![
                IntermediateIfStatement{
                    condition: IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into(),
                    branches: (
                        vec![
                            IntermediateAssignment {
                                location: location.clone(),
                                expression:
                                    IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()

                            }.into()
                        ],
                        vec![
                            IntermediateAssignment {
                                location: location.clone(),
                                expression:
                                    IntermediateFnCall{
                                        fn_: BuiltInFn(
                                            Name::from("++"),
                                            IntermediateFnType(
                                                vec![AtomicTypeEnum::INT.into()],
                                                Box::new(AtomicTypeEnum::INT.into())
                                            ).into()
                                        ).into(),
                                        args: vec![IntermediateBuiltIn::from(Integer{value: 0}).into()]
                                    }.into()

                            }.into()
                        ],
                    )
                }.into(),
                IntermediateAssignment {
                    location: Location::new(),
                    expression:
                        IntermediateTupleExpression(
                            vec![location.clone().into()]
                        ).into()

                }.into()
            ]
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
        {
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            let location = Location::new();
            vec![
                IntermediateAssignment{
                    location: location,
                    expression: IntermediateLambda {
                        args: vec![arg.clone()],
                        statements: Vec::new(),
                        ret: (arg.clone().into(),AtomicTypeEnum::BOOL.into())
                    }.into()
                }.into(),
            ]
        },
        vec![
            Declaration {
                type_: FnType(
                    vec![
                        MachineType::Lazy(Box::new(AtomicTypeEnum::BOOL.into())),
                    ],
                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::BOOL.into())))
                ).into(),
                memory: Memory(Id::from("m1")),
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
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
        statements: Vec<IntermediateStatement>,
        expected_statements: Vec<Statement>,
    ) {
        let mut compiler = Compiler::new();
        compiler.register_memory(&statements);
        let compiled_statements = compiler.compile_statements(statements);
        assert_eq!(compiled_statements, expected_statements);
    }
    #[test_case(
        {
            let bull_type: IntermediateType = IntermediateUnionType(vec![None,None]).into();
            let arg: IntermediateArg = IntermediateType::from(bull_type.clone()).into();
            let location = Location::new();
            (
                vec![Rc::new(RefCell::new(bull_type))],
                vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment {
                                        location: location.clone(),
                                        expression:
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into()

                                    }.into()
                                ],
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: vec![
                                    IntermediateAssignment {
                                        location: location.clone(),
                                        expression:
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()

                                    }.into()
                                ]
                            }
                        ]
                    }.into()
                ]
            )
        },
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: UnionType(vec![Name::from("T0C0"),Name::from("T0C1")]).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: Expression::Unwrap(
                    Memory(Id::from("m0")).into()
                ),
                check_null: false
            }.into(),
            Declaration {
                memory: Memory(Id::from("m2")),
                type_: AtomicTypeEnum::INT.into()
            }.into(),
            MatchStatement {
                expression: (
                    Memory(Id::from("m1")).into(),
                    UnionType(vec![Name::from("T0C0"),Name::from("T0C1")])
                ),
                branches: vec![
                    MatchBranch {
                        target: None,
                        statements: vec![
                            Assignment {
                                target: Memory(Id::from("m2")),
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
                                target: Memory(Id::from("m2")),
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
                vec![Rc::new(RefCell::new(either_type))],
                vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(target0.clone()),
                                statements: vec![
                                    IntermediateAssignment {
                                        location: location.clone(),
                                        expression:
                                            IntermediateFnCall{
                                                fn_: BuiltInFn(
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

                                    }.into()
                                ],
                            },
                            IntermediateMatchBranch{
                                target: Some(target1.clone()),
                                statements: vec![
                                    IntermediateAssignment {
                                        location: location.clone(),
                                        expression:
                                            IntermediateValue::from(target1).into()

                                    }.into()
                                ]
                            }
                        ]
                    }.into()
                ]
            )
        },
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: UnionType(vec![Name::from("T0C0"),Name::from("T0C1")]).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: Expression::Unwrap(
                    Memory(Id::from("m0")).into()
                ),
                check_null: false
            }.into(),
            MatchStatement {
                expression: (
                    Memory(Id::from("m1")).into(),
                    UnionType(vec![Name::from("T0C0"),Name::from("T0C1")])
                ),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("m2"))),
                        statements: vec![
                            Declaration {
                                memory: Memory(Id::from("m3")),
                                type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m3")),
                                value: Expression::Wrap(
                                    BuiltIn::from(Integer{value: 0}).into(),
                                    AtomicTypeEnum::INT.into()
                                ),
                                check_null: false
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m4")),
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
                                        Memory(Id::from("m2")).into(),
                                        Memory(Id::from("m3")).into(),
                                    ]
                                }.into(),
                                check_null: true
                            }.into(),
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("m5"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("m5"))]).into(),
                            Declaration {
                                memory: Memory(Id::from("m6")),
                                type_: AtomicTypeEnum::BOOL.into()
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m6")),
                                value: Expression::Unwrap(
                                    Memory(Id::from("m5")).into()
                                ),
                                check_null: false
                            }.into(),
                            Declaration {
                                memory: Memory(Id::from("m7")),
                                type_: AtomicTypeEnum::BOOL.into()
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m7")),
                                value: Expression::Value(
                                    Memory(Id::from("m6")).into()
                                ),
                                check_null: false
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m4")),
                                value: Expression::Wrap(
                                    Memory(Id::from("m7")).into(),
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
                vec![Rc::new(RefCell::new(either_type))],
                vec![
                    IntermediateMatchStatement{
                        subject: arg.into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(target0.clone()),
                                statements: vec![
                                    IntermediateAssignment {
                                        location: location.clone(),
                                        expression:
                                            IntermediateFnCall{
                                                fn_: BuiltInFn(
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

                                    }.into()
                                ],
                            },
                            IntermediateMatchBranch{
                                target: Some(target1.clone()),
                                statements: vec![
                                    IntermediateAssignment {
                                        location: location.clone(),
                                        expression:
                                            IntermediateValue::from(target1).into()

                                    }.into()
                                ]
                            }
                        ]
                    }.into(),
                    IntermediateAssignment {
                        location: Location::new(),
                        expression:
                            IntermediateTupleExpression(
                                vec![location.clone().into(), IntermediateBuiltIn::from(Integer{value: 0}).into()]
                            ).into()

                    }.into(),
                    IntermediateAssignment {
                        location: Location::new(),
                        expression:
                            IntermediateTupleExpression(
                                vec![location.clone().into(), IntermediateBuiltIn::from(Integer{value: 1}).into()]
                            ).into()

                    }.into()
                ]
            )
        },
        vec![
            Await(vec![Memory(Id::from("m0"))]).into(),
            Declaration {
                memory: Memory(Id::from("m1")),
                type_: UnionType(vec![Name::from("T0C0"),Name::from("T0C1")]).into()
            }.into(),
            Assignment {
                target: Memory(Id::from("m1")),
                value: Expression::Unwrap(
                    Memory(Id::from("m0")).into()
                ),
                check_null: false
            }.into(),
            MatchStatement {
                expression: (
                    Memory(Id::from("m1")).into(),
                    UnionType(vec![Name::from("T0C0"),Name::from("T0C1")])
                ),
                branches: vec![
                    MatchBranch {
                        target: Some(Memory(Id::from("m2"))),
                        statements: vec![
                            Declaration {
                                memory: Memory(Id::from("m3")),
                                type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m3")),
                                value: Expression::Wrap(
                                    BuiltIn::from(Integer{value: 0}).into(),
                                    AtomicTypeEnum::INT.into()
                                ),
                                check_null: false
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m4")),
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
                                        Memory(Id::from("m2")).into(),
                                        Memory(Id::from("m3")).into(),
                                    ]
                                }.into(),
                                check_null: true
                            }.into(),
                        ],
                    },
                    MatchBranch {
                        target: Some(Memory(Id::from("m5"))),
                        statements: vec![
                            Await(vec![Memory(Id::from("m5"))]).into(),
                            Declaration {
                                memory: Memory(Id::from("m6")),
                                type_: AtomicTypeEnum::BOOL.into()
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m6")),
                                value: Expression::Unwrap(
                                    Memory(Id::from("m5")).into()
                                ),
                                check_null: false
                            }.into(),
                            Declaration {
                                memory: Memory(Id::from("m7")),
                                type_: AtomicTypeEnum::BOOL.into()
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m7")),
                                value: Expression::Value(
                                    Memory(Id::from("m6")).into()
                                ),
                                check_null: false
                            }.into(),
                            Assignment {
                                target: Memory(Id::from("m4")),
                                value: Expression::Wrap(
                                    Memory(Id::from("m7")).into(),
                                    AtomicTypeEnum::BOOL.into()
                                ),
                                check_null: true
                            }.into(),
                        ],
                    }
                ]
            }.into(),
            Await(vec![Memory(Id::from("m4"))]).into(),
            Declaration {
                type_: AtomicTypeEnum::BOOL.into(),
                memory: Memory(Id::from("m8"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m8")),
                value: Expression::Unwrap(
                    Memory(Id::from("m4")).into()
                ),
                check_null: false
            }.into(),
            Declaration {
                type_: TupleType(vec![AtomicTypeEnum::BOOL.into(),AtomicTypeEnum::INT.into()]).into(),
                memory: Memory(Id::from("m9"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m9")),
                value: TupleExpression(
                    vec![Memory(Id::from("m8")).into(),BuiltIn::from(Integer{value: 0}).into()]
                ).into(),
                check_null: false
            }.into(),
            Declaration {
                type_: TupleType(vec![AtomicTypeEnum::BOOL.into(),AtomicTypeEnum::INT.into()]).into(),
                memory: Memory(Id::from("m10"))
            }.into(),
            Assignment {
                target: Memory(Id::from("m10")),
                value: TupleExpression(
                    vec![Memory(Id::from("m8")).into(),BuiltIn::from(Integer{value: 1}).into()]
                ).into(),
                check_null: false
            }.into(),
        ];
        "match statement with targets and use"
    )]
    fn test_compile_match_statements(
        args_types_statements: (
            Vec<Rc<RefCell<IntermediateType>>>,
            Vec<IntermediateStatement>,
        ),
        expected_statements: Vec<Statement>,
    ) {
        let (types, statements) = args_types_statements;
        let mut compiler = Compiler::new();
        compiler.compile_type_defs(types);
        compiler.register_memory(&statements);
        let compiled_statements = compiler.compile_statements(statements);
        assert_eq!(compiled_statements, expected_statements);
    }

    #[test_case(
        {
            let arg0: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let arg1: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            let y = Location::new();
            let y_expression: IntermediateExpression = IntermediateFnCall{
                fn_: BuiltInFn(
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
            }.into();
            (
                vec![
                    (y.clone(), y_expression.clone())
                ],
                IntermediateLambda {
                    args: vec![arg0.clone(), arg1.clone()],
                    statements: vec![
                        IntermediateAssignment{
                            location: y.clone(),
                            expression: y_expression,
                        }.into()
                    ],
                    ret: (y.into(), AtomicTypeEnum::INT.into())
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
                    (Memory(Id::from("m0")), MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))),
                    (Memory(Id::from("m1")), MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))),
                ],
                env: None,
                statements: vec![
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
        "env-free closure"
    )]
    #[test_case(
        {
            let x = Location::new();
            let y = Location::new();
            let z = Location::new();
            let z_expression: IntermediateExpression = IntermediateFnCall{
                fn_: BuiltInFn(
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
            }.into();
            (
                vec![
                    (x.clone(), IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 3})).into()),
                    (y.clone(), IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 4})).into()),
                    (z.clone(), z_expression.clone()),
                ],
                IntermediateLambda {
                    args: ...,
                    statements: vec![
                        IntermediateAssignment{
                            location: z.clone(),
                            expression: z_expression,
                        }.into()
                    ],
                    ret: (z.into(), AtomicTypeEnum::INT.into())
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
        locations_fn_defs: (Vec<(Location, IntermediateExpression)>, IntermediateLambda),
        expected: (Vec<Statement>, ClosureInstantiation, FnDef),
    ) {
        let (locations, fn_def) = locations_fn_defs;
        let (expected_statements, expected_value, expected_fn_def) = expected;

        let mut compiler = Compiler::new();
        compiler.register_memory(&fn_def.statements);
        let extra_statements = locations
            .into_iter()
            .map(|(location, expression)| {
                IntermediateStatement::IntermediateAssignment(
                    IntermediateAssignment {
                        location,
                        expression,
                    }
                    .into(),
                )
            })
            .collect();
        compiler.register_memory(&extra_statements);

        let compiled = compiler.compile_lambda(fn_def);
        assert_eq!(compiled, (expected_statements, expected_value));
        let compiled_fn_def = &compiler.fn_defs[0];
        assert_eq!(compiled_fn_def, &expected_fn_def);
    }

    #[test]
    fn test_memory_sharing_across_fn_defs() {
        let f0 = IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            location: Location::new(),
            expression: IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: (
                    IntermediateValue::from(IntermediateBuiltIn::from(Boolean { value: true }))
                        .into(),
                    AtomicTypeEnum::BOOL.into(),
                ),
            }
            .into(),
        });
        let f1 = IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
            location: Location::new(),
            expression: IntermediateLambda {
                args: Vec::new(),
                statements: Vec::new(),
                ret: (
                    IntermediateValue::from(IntermediateBuiltIn::from(Boolean { value: true }))
                        .into(),
                    AtomicTypeEnum::BOOL.into(),
                ),
            }
            .into(),
        });
        let statements = vec![f0, f1];

        let mut compiler = Compiler::new();
        compiler.register_memory(&statements);
        compiler.compile_statements(statements);
        let [ref f0, ref f1] = compiler.fn_defs[..] else {
            panic!("Wrong number of fn-defs generated.")
        };
        assert_ne!(f0.ret, f1.ret)
    }

    #[test_case(
        {
            let identity = Location::new();
            let main = Location::new();
            let y = Location::new();
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            IntermediateProgram {
                statements: vec![
                    IntermediateAssignment{
                        location: identity.clone(),
                        expression:
                            IntermediateLambda{
                                args: vec![arg.clone()],
                                statements: Vec::new(),
                                ret: (arg.clone().into(), AtomicTypeEnum::INT.into())
                            }.into()

                    }.into(),
                    IntermediateAssignment{
                        location: main.clone(),
                        expression:
                            IntermediateLambda{
                                args: Vec::new(),
                                statements: vec![
                                    IntermediateAssignment{
                                        location: y.clone(),
                                        expression:
                                            IntermediateFnCall{
                                                fn_: identity.clone().into(),
                                                args: vec![IntermediateBuiltIn::from(Integer{value: 0}).into()]
                                            }.into()

                                    }.into()
                                ],
                                ret: (y.clone().into(), AtomicTypeEnum::INT.into())
                            }.into()

                    }.into(),
                ],
                main: main.clone().into(),
                types: Vec::new()
            }
        },
        Program {
            type_defs: Vec::new(),
            fn_defs: vec![
                FnDef {
                    name: Name::from("F0"),
                    arguments: vec![(Memory(Id::from("m0")), MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))],
                    statements: Vec::new(),
                    ret: (Memory(Id::from("m0")).into(), MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))),
                    env: None,
                    allocations: Vec::new()
                },
                FnDef {
                    name: Name::from("F1"),
                    arguments: Vec::new(),
                    statements: vec![
                        Declaration {
                            type_: MachineType::Lazy(Box::new(FnType(
                                vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                                Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))).into())
                            ).into(),
                            memory: Memory(Id::from("m2")),
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m2")),
                            value: ElementAccess {
                                value: Memory(Id::from("env")).into(),
                                idx: 0
                            }.into(),
                            check_null: false
                        }.into(),
                        Await(vec![Memory(Id::from("m2"))]).into(),
                        Declaration {
                            type_: FnType(
                                vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                                Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                            ).into(),
                            memory: Memory(Id::from("m3"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m3")),
                            value: Expression::Unwrap(Memory(Id::from("m2")).into()),
                            check_null: false
                        }.into(),
                        Declaration {
                            type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                            memory: Memory(Id::from("m4"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m4")),
                            value: Expression::Wrap(
                                BuiltIn::from(Integer { value: 0 }).into(),
                                AtomicTypeEnum::INT.into()
                            ),
                            check_null: false
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m5")),
                            value: FnCall {
                                fn_: Memory(Id::from("m3")).into(),
                                fn_type: FnType(
                                    vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                                ),
                                args: vec![Memory(Id::from("m4")).into()]
                            }.into(),
                            check_null: true
                        }.into()
                    ],
                    ret: (Memory(Id::from("m5")).into(), MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))),
                    env: Some(TupleType(vec![
                        MachineType::Lazy(
                            Box::new(FnType(
                                vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                                Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                            ).into())
                        )
                    ]).into()),
                    allocations: vec![
                        Declaration {
                            type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                            memory: Memory(Id::from("m5"))
                        }
                    ]
                },
                FnDef {
                    name: Name::from("Main"),
                    arguments: Vec::new(),
                    statements: vec![
                        Declaration {
                            type_: FnType(
                                vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                                Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                            ).into(),
                            memory: Memory(Id::from("m1"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m1")),
                            value: ClosureInstantiation { name: Name::from("F0"), env: None }.into(),
                            check_null: false
                        }.into(),
                        Declaration {
                            type_: MachineType::Lazy(
                                Box::new(FnType(
                                    vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                                ).into())
                            ),
                            memory: Memory(Id::from("m7"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m7")),
                            value: Expression::Wrap(
                                Memory(Id::from("m1")).into(),
                                FnType(
                                    vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                                ).into()
                            ),
                            check_null: false
                        }.into(),
                        Declaration {
                            type_: TupleType(
                                vec![MachineType::Lazy(
                                    Box::new(FnType(
                                        vec![MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))],
                                        Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                                    ).into())
                                )]
                            ).into(),
                            memory: Memory(Id::from("m6"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m6")),
                            value: TupleExpression(vec![Memory(Id::from("m7")).into()]).into(),
                            check_null: false
                        }.into(),
                        Declaration {
                            type_: FnType(Vec::new(), Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))).into(),
                            memory: Memory(Id::from("m8"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m8")),
                            value: ClosureInstantiation {
                                name: Name::from("F1"),
                                env: Some(Memory(Id::from("m6")).into())
                            }.into(),
                            check_null: false
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m9")),
                            value: FnCall {
                                fn_: Memory(Id::from("m8")).into(),
                                fn_type: FnType(Vec::new(), Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))).into(),
                                args: Vec::new()
                            }.into(),
                            check_null: true
                        }.into()
                    ],
                    ret: (Memory(Id::from("m9")).into(), MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))),
                    env: None,
                    allocations: vec![
                        Declaration {
                            type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                            memory: Memory(Id::from("m9"))
                        }
                    ]
                }
            ]
        };
        "identity call program"
    )]
    #[test_case(
        {
            let main = Location::new();
            let c = Location::new();
            let r = Location::new();
            IntermediateProgram {
                statements: vec![
                    IntermediateAssignment{
                        location: main.clone(),
                        expression:
                            IntermediateLambda{
                                args: Vec::new(),
                                statements: vec![
                                    IntermediateAssignment{
                                        location: c.clone(),
                                        expression:
                                            IntermediateCtorCall {
                                                idx: 0,
                                                data: None,
                                                type_: IntermediateUnionType(vec![None,None])
                                            }.into()

                                    }.into(),
                                    IntermediateMatchStatement {
                                        subject: c.clone().into(),
                                        branches: vec![
                                            IntermediateMatchBranch{
                                                target: None,
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        location: r.clone(),
                                                        expression:
                                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()

                                                    }.into()
                                                ]
                                            },
                                            IntermediateMatchBranch{
                                                target: None,
                                                statements: vec![
                                                    IntermediateAssignment{
                                                        location: r.clone(),
                                                        expression:
                                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into()

                                                    }.into()
                                                ]
                                            }
                                        ]
                                    }.into()
                                ],
                                ret: (r.clone().into(), AtomicTypeEnum::INT.into())
                            }.into()

                    }.into(),
                ],
                main: main.clone().into(),
                types: vec![
                    Rc::new(RefCell::new(
                        IntermediateUnionType(vec![None,None]).into()
                    ))
                ]
            }
        },
        Program{
            type_defs: vec![
                TypeDef {
                    name: Name::from("T0"),
                    constructors: vec![
                        (Name::from("T0C0"), None),
                        (Name::from("T0C1"), None)
                    ]
                }
            ],
            fn_defs: vec![
                FnDef {
                    name: Name::from("F0"),
                    arguments: Vec::new(), statements: vec![
                        Declaration {
                            type_: UnionType(vec![Name::from("T0C0"), Name::from("T0C1")]).into(),
                            memory: Memory(Id::from("m0"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m0")),
                            value: ConstructorCall { idx: 0, data: None }.into(),
                            check_null: false
                        }.into(),
                        Declaration {
                            type_: AtomicTypeEnum::INT.into(),
                            memory: Memory(Id::from("m1"))
                        }.into(),
                        MatchStatement {
                            expression: (Memory(Id::from("m0")).into(), UnionType(vec![Name::from("T0C0"), Name::from("T0C1")])),
                            branches: vec![
                                MatchBranch {
                                    target: None,
                                    statements: vec![
                                        Assignment {
                                            target: Memory(Id::from("m1")),
                                            value: Value::from(BuiltIn::from(Integer { value: 0 })).into(),
                                            check_null: false
                                        }.into()
                                    ]
                                },
                                MatchBranch {
                                    target: None,
                                    statements: vec![
                                        Assignment {
                                            target: Memory(Id::from("m1")),
                                            value: Value::from(BuiltIn::from(Integer { value: 1 })).into(),
                                            check_null: false
                                        }.into()
                                    ]
                                }
                            ]
                        }.into(),
                        Declaration {
                            type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                            memory: Memory(Id::from("m2"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m2")),
                            value: Expression::Wrap(
                                Memory(Id::from("m1")).into(),
                                AtomicTypeEnum::INT.into()
                            ),
                            check_null: false
                        }.into()
                    ],
                    ret: (Memory(Id::from("m2")).into(),MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))),
                    env: None,
                    allocations: Vec::new()
                },
                FnDef {
                    name: Name::from("Main"),
                    arguments: Vec::new(),
                    statements: vec![
                        Declaration {
                            type_: FnType(
                                Vec::new(),
                                Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                            ).into(),
                            memory: Memory(Id::from("m3"))
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m3")),
                            value: ClosureInstantiation { name: Name::from("F0"), env: None }.into(),
                            check_null: false
                        }.into(),
                        Assignment {
                            target: Memory(Id::from("m4")),
                            value: FnCall {
                                fn_: Memory(Id::from("m3")).into(),
                                fn_type: FnType(
                                    Vec::new(),
                                    Box::new(MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())))
                                ),
                                args: Vec::new()
                            }.into(),
                            check_null: true
                        }.into()
                    ],
                    ret: (Memory(Id::from("m4")).into(), MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into()))),
                    env: None,
                    allocations: vec![
                        Declaration {
                            type_: MachineType::Lazy(Box::new(AtomicTypeEnum::INT.into())),
                            memory: Memory(Id::from("m4"))
                        }
                    ]
                }
            ]
        };
        "program with type defs"
    )]
    fn test_compile_program(program: IntermediateProgram, expected_program: Program) {
        let mut compiler = Compiler::new();
        let compiled_program = compiler.compile_program(program);
        assert_eq!(compiled_program, expected_program);
    }
}
