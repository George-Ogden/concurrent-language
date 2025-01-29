use std::{
    cell::RefCell,
    cmp::minmax,
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
};

use itertools::{zip_eq, Itertools};
use lowering::{
    IntermediateArg, IntermediateAssignment, IntermediateExpression, IntermediateFnCall,
    IntermediateFnDef, IntermediateIfStatement, IntermediateMatchBranch,
    IntermediateMatchStatement, IntermediateProgram, IntermediateStatement,
    IntermediateTupleExpression, IntermediateValue, Location,
};

struct Optimizer {
    single_constraints: HashMap<Location, HashSet<Location>>,
    double_constraints: HashMap<(Location, Location), HashSet<Location>>,
    fn_args: HashMap<Location, Vec<Location>>,
    variables: HashSet<Location>,
    fn_updates: HashMap<Location, Location>,
}

impl Optimizer {
    fn new() -> Self {
        Optimizer {
            single_constraints: HashMap::new(),
            double_constraints: HashMap::new(),
            fn_args: HashMap::new(),
            variables: HashSet::new(),
            fn_updates: HashMap::new(),
        }
    }
    fn used_value(&mut self, value: &IntermediateValue) -> Option<Location> {
        match value {
            lowering::IntermediateValue::IntermediateMemory(location) => Some(location.clone()),
            lowering::IntermediateValue::IntermediateArg(arg) => Some(arg.location.clone()),
            lowering::IntermediateValue::IntermediateBuiltIn(_) => None,
        }
    }
    fn find_used_values(&mut self, expression: &IntermediateExpression) -> Vec<Location> {
        let values = expression.values();
        values
            .into_iter()
            .filter_map(|value| self.used_value(&value))
            .collect()
    }
    fn add_single_constraint(&mut self, location: Location, dependents: Vec<Location>) {
        if !self.single_constraints.contains_key(&location) {
            self.single_constraints
                .insert(location.clone(), HashSet::new());
        }
        self.single_constraints
            .get_mut(&location)
            .unwrap()
            .extend(dependents);
    }
    fn add_double_constraint(
        &mut self,
        arg: Location,
        location: Location,
        dependents: Vec<Location>,
    ) {
        let key = minmax(arg, location).into();
        if !self.double_constraints.contains_key(&key) {
            self.double_constraints.insert(key.clone(), HashSet::new());
        }
        self.double_constraints
            .get_mut(&key)
            .unwrap()
            .extend(dependents);
    }
    fn generate_constraints(&mut self, statements: &Vec<IntermediateStatement>) {
        for statement in statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    location,
                }) => match &expression.borrow().clone() {
                    IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                        args,
                        statements: _,
                        ret: _,
                    }) => {
                        let args = args.into_iter().map(|arg| arg.location.clone()).collect();
                        self.fn_args.insert(location.clone(), args);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        for statement in statements {
            match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    location,
                }) => match &expression.borrow().clone() {
                    IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                        args: _,
                        statements,
                        ret,
                    }) => {
                        self.generate_constraints(statements);
                        let dependents = self.used_value(&ret.0).iter().cloned().collect_vec();
                        self.add_single_constraint(location.clone(), dependents);
                    }
                    IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                        fn_,
                        args,
                    }) => match fn_ {
                        lowering::IntermediateValue::IntermediateBuiltIn(_) => {
                            let dependents = args
                                .iter()
                                .filter_map(|value| self.used_value(value))
                                .collect();
                            self.add_single_constraint(location.clone(), dependents);
                        }
                        lowering::IntermediateValue::IntermediateMemory(fn_) => {
                            self.add_single_constraint(location.clone(), vec![fn_.clone()]);
                            match self.fn_args.get(&fn_) {
                                Some(fn_args) => {
                                    for (loc, arg) in zip_eq(fn_args.clone(), args) {
                                        let dependents =
                                            self.used_value(arg).iter().cloned().collect_vec();
                                        self.add_double_constraint(
                                            loc,
                                            location.clone(),
                                            dependents,
                                        )
                                    }
                                }
                                None => {
                                    let dependents = args
                                        .iter()
                                        .filter_map(|arg| self.used_value(arg))
                                        .collect();
                                    self.add_single_constraint(location.clone(), dependents);
                                }
                            }
                        }
                        _ => {
                            let mut values = args.clone();
                            values.push(fn_.clone());
                            self.generate_constraints(&vec![IntermediateAssignment {
                                expression: Rc::new(RefCell::new(
                                    IntermediateTupleExpression(values).into(),
                                )),
                                location: location.clone(),
                            }
                            .into()]);
                        }
                    },
                    expression => {
                        let used_values = self.find_used_values(&expression);
                        self.add_single_constraint(location.clone(), used_values)
                    }
                },
                IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                    condition,
                    branches,
                }) => {
                    let targets = (
                        HashSet::<Location>::from_iter(IntermediateStatement::all_targets(
                            &branches.0,
                        )),
                        HashSet::<Location>::from_iter(IntermediateStatement::all_targets(
                            &branches.1,
                        )),
                    );
                    self.generate_constraints(&branches.0);
                    self.generate_constraints(&branches.1);
                    let shared_targets = targets.0.intersection(&targets.1);
                    for target in shared_targets {
                        let dependents = self.used_value(condition).iter().cloned().collect();
                        self.add_single_constraint(target.clone(), dependents);
                    }
                }
                IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                    subject,
                    branches,
                }) => {
                    let mut shared_targets: Option<HashSet<Location>> = None;
                    let subject_dependents: Vec<_> =
                        self.used_value(subject).iter().cloned().collect();
                    for branch in branches {
                        match &branch.target {
                            Some(IntermediateArg { type_: _, location }) => {
                                self.add_single_constraint(
                                    location.clone(),
                                    subject_dependents.clone(),
                                );
                            }
                            None => {}
                        }
                        self.generate_constraints(&branch.statements);
                        let targets = HashSet::from_iter(IntermediateStatement::all_targets(
                            &branch.statements,
                        ));
                        shared_targets = Some(match shared_targets {
                            None => targets,
                            Some(set) => set.intersection(&targets).cloned().collect(),
                        })
                    }
                    for target in shared_targets.unwrap_or_default() {
                        self.add_single_constraint(target, subject_dependents.clone());
                    }
                }
            }
        }
    }
    fn solve_constraints(&self, initial_solution: Location) -> HashSet<Location> {
        let mut solution = HashSet::from_iter([initial_solution.clone()]);
        let mut new_variables = VecDeque::from([initial_solution]);
        let mut double_constraint_index: HashMap<Location, Vec<Location>> = HashMap::from_iter(
            self.double_constraints
                .keys()
                .flat_map(|(l1, l2)| [(l1.clone(), Vec::new()), (l2.clone(), Vec::new())]),
        );
        for (k, v) in self
            .double_constraints
            .keys()
            .flat_map(|(l1, l2)| [(l1.clone(), l2.clone()), (l2.clone(), l1.clone())])
        {
            double_constraint_index.get_mut(&k).unwrap().push(v);
        }
        while let Some(c) = new_variables.pop_front() {
            if let Some(set) = self.single_constraints.get(&c) {
                for variable in set {
                    if !solution.contains(&variable) {
                        solution.insert(variable.clone());
                        new_variables.push_back(variable.clone());
                    }
                }
            }
            if let Some(others) = double_constraint_index.get(&c) {
                for other in others {
                    if solution.contains(other) {
                        let key = minmax(c.clone(), other.clone()).into();
                        for variable in &self.double_constraints[&key] {
                            if !solution.contains(&variable) {
                                solution.insert(variable.clone());
                                new_variables.push_back(variable.clone());
                            }
                        }
                    }
                }
            }
        }
        solution
    }
    fn filter_args<T>(&self, location: &Location, values: Vec<T>) -> Vec<T> {
        match self.fn_args.get(&location) {
            None => values,
            Some(args) => values
                .into_iter()
                .zip_eq(args)
                .filter(|(_, arg)| self.variables.contains(&arg))
                .map(|(v, _)| v)
                .collect_vec(),
        }
    }
    fn remove_redundancy(
        &mut self,
        statements: Vec<IntermediateStatement>,
    ) -> Vec<IntermediateStatement> {
        let statements = statements
            .into_iter()
            .flat_map(|statement| match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    location,
                }) => {
                    if self.variables.contains(&location) {
                        if let IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                            args,
                            ret,
                            statements,
                        }) = expression.borrow().clone()
                        {
                            let used_args = self.filter_args(&location, args.clone());
                            if used_args.len() != args.len() {
                                let fresh_args = args
                                    .iter()
                                    .map(|arg| IntermediateArg::from(arg.type_.clone()))
                                    .collect_vec();
                                let fn_loc = Location::new();
                                let ret_loc = Location::new();
                                self.variables.insert(fn_loc.clone());
                                self.variables.insert(ret_loc.clone());
                                self.fn_updates.insert(location.clone(), fn_loc.clone());
                                let unoptimized_fn = IntermediateFnDef {
                                    args: fresh_args.clone(),
                                    statements: vec![IntermediateAssignment {
                                        location: ret_loc.clone(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateFnCall {
                                                fn_: fn_loc.clone().into(),
                                                args: self.filter_args(
                                                    &location,
                                                    fresh_args
                                                        .into_iter()
                                                        .map(Into::into)
                                                        .collect_vec(),
                                                ),
                                            }
                                            .into(),
                                        )),
                                    }
                                    .into()],
                                    ret: (ret_loc.into(), ret.1.clone()),
                                }
                                .into();
                                return vec![
                                    IntermediateAssignment {
                                        expression: Rc::new(RefCell::new(
                                            IntermediateFnDef {
                                                args: used_args,
                                                ret,
                                                statements,
                                            }
                                            .into(),
                                        )),
                                        location: fn_loc,
                                    }
                                    .into(),
                                    IntermediateAssignment {
                                        expression: Rc::new(RefCell::new(unoptimized_fn)),
                                        location,
                                    }
                                    .into(),
                                ];
                            }
                        }
                    }
                    vec![IntermediateAssignment {
                        expression,
                        location,
                    }
                    .into()]
                }
                statement => vec![statement],
            })
            .collect_vec();
        statements
            .into_iter()
            .filter_map(|statement| match statement {
                IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                    expression,
                    location,
                }) => {
                    if self.variables.contains(&location) {
                        match expression.clone().borrow().clone() {
                            IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                                args,
                                statements,
                                ret,
                            }) => Some(
                                IntermediateAssignment {
                                    expression: Rc::new(RefCell::new(
                                        IntermediateFnDef {
                                            args,
                                            statements: self.remove_redundancy(statements),
                                            ret,
                                        }
                                        .into(),
                                    )),
                                    location,
                                }
                                .into(),
                            ),
                            IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                                fn_: IntermediateValue::IntermediateMemory(fn_),
                                args,
                            }) if self.fn_updates.contains_key(&fn_)
                                && !self.fn_updates.values().contains(&location) =>
                            {
                                Some(
                                    IntermediateAssignment {
                                        expression: Rc::new(RefCell::new(
                                            IntermediateFnCall {
                                                args: self.filter_args(&fn_, args),
                                                fn_: self.fn_updates[&fn_].clone().into(),
                                            }
                                            .into(),
                                        )),
                                        location,
                                    }
                                    .into(),
                                )
                            }
                            _ => Some(
                                IntermediateAssignment {
                                    expression,
                                    location,
                                }
                                .into(),
                            ),
                        }
                    } else {
                        None
                    }
                }
                IntermediateStatement::IntermediateIfStatement(IntermediateIfStatement {
                    condition,
                    branches,
                }) => {
                    if let IntermediateValue::IntermediateMemory(location) = &condition {
                        if !self.variables.contains(location) {
                            return None;
                        }
                    }
                    Some(
                        IntermediateIfStatement {
                            condition,
                            branches: (
                                self.remove_redundancy(branches.0),
                                self.remove_redundancy(branches.1),
                            ),
                        }
                        .into(),
                    )
                }
                IntermediateStatement::IntermediateMatchStatement(IntermediateMatchStatement {
                    subject,
                    branches,
                }) => {
                    if let IntermediateValue::IntermediateMemory(location) = &subject {
                        if !self.variables.contains(location) {
                            return None;
                        }
                    }
                    Some(
                        IntermediateMatchStatement {
                            subject,
                            branches: branches
                                .into_iter()
                                .map(
                                    |IntermediateMatchBranch {
                                         mut target,
                                         statements,
                                     }| {
                                        if let Some(IntermediateArg { type_: _, location }) =
                                            &target
                                        {
                                            if !self.variables.contains(location) {
                                                target = None;
                                            }
                                        }
                                        IntermediateMatchBranch {
                                            target,
                                            statements: self.remove_redundancy(statements),
                                        }
                                    },
                                )
                                .collect(),
                        }
                        .into(),
                    )
                }
            })
            .collect_vec()
    }
    fn remove_dead_code(program: IntermediateProgram) -> IntermediateProgram {
        let mut optimizer = Optimizer::new();
        optimizer.generate_constraints(&program.statements);
        let IntermediateValue::IntermediateMemory(location) = &program.main else {
            return program;
        };
        optimizer.variables = optimizer.solve_constraints(location.clone());
        let statements = optimizer.remove_redundancy(program.statements);
        IntermediateProgram {
            statements,
            main: program.main,
            types: program.types,
        }
    }
}

#[cfg(test)]
mod tests {

    use std::{cell::RefCell, rc::Rc};

    use super::*;

    use lowering::{
        AtomicTypeEnum, Boolean, ExpressionEqualityChecker, Id, Integer, IntermediateArg,
        IntermediateBuiltIn, IntermediateCtorCall, IntermediateElementAccess, IntermediateFnCall,
        IntermediateFnDef, IntermediateFnType, IntermediateIfStatement, IntermediateMatchBranch,
        IntermediateMatchStatement, IntermediateProgram, IntermediateStatement,
        IntermediateTupleExpression, IntermediateTupleType, IntermediateType,
        IntermediateUnionType, IntermediateValue,
    };
    use test_case::test_case;

    #[test_case(
        (
            IntermediateValue::from(
                IntermediateBuiltIn::from(Integer{
                    value: 8
                })
            ).into(),
            Vec::new(),
        );
        "integer"
    )]
    #[test_case(
        (
            IntermediateValue::from(
                IntermediateBuiltIn::from(Boolean{
                    value: false
                })
            ).into(),
            Vec::new(),
        );
        "boolean"
    )]
    #[test_case(
        (
            IntermediateValue::from(
                IntermediateBuiltIn::BuiltInFn(
                    Id::from("+"),
                    IntermediateFnType(
                        vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into())
                    ).into()
                )
            ).into(),
            Vec::new(),
        );
        "built-in fn"
    )]
    #[test_case(
        {
            let location = Location::new();
            (
                IntermediateValue::from(
                    location.clone()
                ).into(),
                vec![location.clone()],
            )
        };
        "memory location"
    )]
    #[test_case(
        {
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            (
                IntermediateValue::from(
                    arg.clone()
                ).into(),
                vec![arg.location],
            )
        };
        "arg"
    )]
    #[test_case(
        {
            let location = Location::new();
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::INT).into();
            (
                IntermediateTupleExpression(vec![
                    arg.clone().into(), location.clone().into(), IntermediateBuiltIn::from(Integer{value: 7}).into()
                ]).into(),
                vec![location.clone(), arg.location],
            )
        };
        "tuple"
    )]
    #[test_case(
        {
            let location = Location::new();
            (
                IntermediateElementAccess{
                    value: location.clone().into(),
                    idx: 8
                }.into(),
                vec![location.clone()],
            )
        };
        "element access"
    )]
    fn test_find_used_values(expression_locations: (IntermediateExpression, Vec<Location>)) {
        let (expression, expected_locations) = expression_locations;
        let mut optimizer = Optimizer::new();

        let expected: HashSet<_> = expected_locations.into_iter().collect();
        let locations = optimizer.find_used_values(&expression);
        assert_eq!(HashSet::from_iter(locations), expected);
    }

    #[test_case(
        (
            vec![
                IntermediateAssignment{
                    expression: Rc::new(RefCell::new(IntermediateValue::from(
                        IntermediateBuiltIn::from(Integer{
                            value: 8
                        })
                    ).into())),
                    location: Location::new()
                }.into()
            ],
            Vec::new(),
            Vec::new(),
        );
        "no constraint assignment"
    )]
    #[test_case(
        {
            let var1 = Location::new();
            let var2 = Location::new();
            let tuple = Location::new();
            let res = Location::new();
            (
                vec![
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateTupleExpression(vec![
                            var1.clone().into(), var2.clone().into()
                        ]).into())),
                        location: tuple.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateElementAccess{
                            value: tuple.clone().into(),
                            idx: 0
                        }.into())),
                        location: res.clone()
                    }.into()
                ],
                vec![
                    (tuple.clone(), vec![var1.clone(), var2.clone()]),
                    (res.clone(), vec![tuple.clone()]),
                ],
                Vec::new(),
            )
        };
        "basic assignments"
    )]
    #[test_case(
        {
            let id = Location::new();
            let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let x = Location::new();
            let y = Location::new();
            (
                vec![
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateFnDef{
                            args: vec![arg.clone()],
                            statements: Vec::new(),
                            ret: (arg.clone().into(), AtomicTypeEnum::INT.into())
                        }.into())),
                        location: id.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateFnCall{
                            fn_: id.clone().into(),
                            args: vec![
                                x.clone().into()
                            ]
                        }.into())),
                        location: y.clone()
                    }.into()
                ],
                vec![
                    (y.clone(), vec![id.clone()]),
                    (id.clone(), vec![arg.location.clone()]),
                ],
                vec![
                    (
                        (y.clone(), arg.location.clone()),
                        vec![x.clone()]
                    )
                ]
            )
        };
        "identity fn"
    )]
    #[test_case(
        {
            let x = Location::new();
            let y = Location::new();
            let z = Location::new();
            (
                vec![
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateFnCall{
                            fn_: IntermediateBuiltIn::BuiltInFn(
                                Id::from("*"),
                                IntermediateFnType(
                                    vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                                    Box::new(AtomicTypeEnum::INT.into())
                                ).into()
                            ).into(),
                            args: vec![
                                x.clone().into(),
                                y.clone().into()
                            ]
                        }.into())),
                        location: z.clone()
                    }.into()
                ],
                vec![
                    (z, vec![x, y]),
                ],
                Vec::new()
            )
        };
        "built-in fn call"
    )]
    #[test_case(
        {
            let f = IntermediateArg::from(
                IntermediateType::from(IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let x = Location::new();
            let y = Location::new();
            (
                vec![
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateFnCall{
                            fn_: f.clone().into(),
                            args: vec![
                                x.clone().into(),
                            ]
                        }.into())),
                        location: y.clone()
                    }.into()
                ],
                vec![
                    (y, vec![f.location, x]),
                ],
                Vec::new()
            )
        };
        "argument fn call"
    )]
    #[test_case(
        {
            let fn_ = Location::new();
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let z = Location::new();
            (
                vec![
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateFnDef{
                            args: vec![x.clone(), y.clone()],
                            statements: vec![
                                IntermediateAssignment{
                                    location: z.clone(),
                                    expression: Rc::new(RefCell::new(IntermediateFnCall{
                                        fn_: IntermediateValue::from(
                                            IntermediateBuiltIn::BuiltInFn(
                                                Id::from("+"),
                                                IntermediateFnType(
                                                    vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::INT.into()],
                                                    Box::new(AtomicTypeEnum::INT.into())
                                                ).into()
                                            )
                                        ),
                                        args: vec![y.location.clone().into(), IntermediateBuiltIn::from(Integer{value: 9}).into()]
                                    }.into()))
                                }.into()
                            ],
                            ret: (x.clone().into(), AtomicTypeEnum::INT.into())
                        }.into())),
                        location: fn_.clone()
                    }.into(),
                ],
                vec![
                    (fn_.clone(), vec![x.location.clone()]),
                    (z.clone(), vec![y.location.clone()]),
                ],
                Vec::new()
            )
        };
        "fn with statements"
    )]
    #[test_case(
        {
            let foo = Location::new();
            let bar = Location::new();
            let foo_call = Location::new();
            let bar_call = Location::new();
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateFnDef{
                            args: vec![x.clone()],
                            statements: vec![
                                IntermediateAssignment{
                                    location: bar_call.clone(),
                                    expression: Rc::new(RefCell::new(IntermediateFnCall{
                                        fn_: bar.clone().into(),
                                        args: vec![x.location.clone().into()]
                                    }.into()))
                                }.into()
                            ],
                            ret: (bar_call.clone().into(), AtomicTypeEnum::INT.into())
                        }.into())),
                        location: foo.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateFnDef{
                            args: vec![y.clone()],
                            statements: vec![
                                IntermediateAssignment{
                                    location: foo_call.clone(),
                                    expression: Rc::new(RefCell::new(IntermediateFnCall{
                                        fn_: foo.clone().into(),
                                        args: vec![y.location.clone().into()]
                                    }.into()))
                                }.into()
                            ],
                            ret: (foo_call.clone().into(), AtomicTypeEnum::INT.into())
                        }.into())),
                        location: bar.clone()
                    }.into(),
                ],
                vec![
                    (foo.clone(), vec![bar_call.clone()]),
                    (bar.clone(), vec![foo_call.clone()]),
                    (foo_call.clone(), vec![foo.clone()]),
                    (bar_call.clone(), vec![bar.clone()]),
                ],
                vec![
                    ((foo_call.clone(), x.location.clone()), vec![y.location.clone()]),
                    ((bar_call.clone(), y.location.clone()), vec![x.location.clone()]),
                ],
            )
        };
        "mutually recursive fns"
    )]
    #[test_case(
        {
            let f = Location::new();
            let g = Location::new();
            let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let x = Location::new();
            let y = Location::new();
            (
                vec![
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateFnDef{
                            args: vec![arg.clone()],
                            statements: Vec::new(),
                            ret: (arg.clone().into(), AtomicTypeEnum::INT.into())
                        }.into())),
                        location: f.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateValue::from(
                            f.clone()
                        ).into())),
                        location: g.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: Rc::new(RefCell::new(IntermediateFnCall{
                            fn_: g.clone().into(),
                            args: vec![
                                x.clone().into()
                            ]
                        }.into())),
                        location: y.clone()
                    }.into()
                ],
                vec![
                    (y.clone(), vec![g.clone(), x.clone()]),
                    (f.clone(), vec![arg.location.clone()]),
                    (g.clone(), vec![f.clone()]),
                ],
                Vec::new()
            )
        };
        "reassigned fn"
    )]
    #[test_case(
        {
            let c = Location::new();
            (
                vec![
                    IntermediateIfStatement{
                        condition: c.clone().into(),
                        branches: (Vec::new(), Vec::new())
                    }.into()
                ],
                Vec::new(),
                Vec::new()
            )
        };
        "empty if statement"
    )]
    #[test_case(
        {
            let c = Location::new();
            let x = Location::new();
            let y = Location::new();
            let z = Location::new();
            (
                vec![
                    IntermediateIfStatement{
                        condition: c.clone().into(),
                        branches: (
                            vec![
                                IntermediateAssignment{
                                    location: x.clone().into(),
                                    expression: Rc::new(RefCell::new(
                                        IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                    ))
                                }.into(),
                                IntermediateAssignment{
                                    location: z.clone().into(),
                                    expression: Rc::new(RefCell::new(
                                        IntermediateValue::from(x.clone()).into()
                                    ))
                                }.into(),
                            ],
                            vec![
                                IntermediateAssignment{
                                    location: z.clone().into(),
                                    expression: Rc::new(RefCell::new(
                                        IntermediateValue::from(y.clone()).into()
                                    ))
                                }.into(),
                            ],
                        )
                    }.into()
                ],
                vec![(
                    z,
                    vec![c, x, y]
                )],
                Vec::new()
            )
        };
        "if statement"
    )]
    #[test_case(
        {
            let s = Location::new();
            let t = IntermediateArg::from(IntermediateType::from(
                AtomicTypeEnum::INT
            ));
            (
                vec![
                    IntermediateMatchStatement{
                        subject: s.clone().into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(t.clone()),
                                statements: Vec::new()
                            },
                            IntermediateMatchBranch{
                                target: None,
                                statements: Vec::new()
                            },
                        ]
                    }.into()
                ],
                vec![
                    (t.location, vec![s])
                ],
                Vec::new()
            )
        };
        "empty match statement"
    )]
    #[test_case(
        {
            let s = Location::new();
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let y = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::BOOL));
            let z = Location::new();
            (
                vec![
                    IntermediateMatchStatement{
                        subject: s.clone().into(),
                        branches: vec![
                            IntermediateMatchBranch{
                                target: Some(x.clone()),
                                statements: vec![
                                    IntermediateAssignment{
                                        location: z.clone().into(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(x.clone()).into()
                                        ))
                                    }.into(),
                                ]
                            },
                            IntermediateMatchBranch{
                                target: Some(y.clone()),
                                statements: vec![
                                    IntermediateAssignment{
                                        location: z.clone().into(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(
                                                IntermediateBuiltIn::from(Integer{value: 0})
                                            ).into()
                                        ))
                                    }.into(),
                                ]
                            },
                        ]
                    }.into()
                ],
                vec![
                    (x.location.clone(), vec![s.clone()]),
                    (y.location.clone(), vec![s.clone()]),
                    (z.clone(), vec![x.location, s]),
                ],
                Vec::new()
            )
        };
        "match statement"
    )]
    fn test_constraint_generation(
        statements_singles_doubles: (
            Vec<IntermediateStatement>,
            Vec<(Location, Vec<Location>)>,
            Vec<((Location, Location), Vec<Location>)>,
        ),
    ) {
        let (statements, expected_single_constraints, expected_double_constraints) =
            statements_singles_doubles;
        let mut optimizer = Optimizer::new();

        optimizer.generate_constraints(&statements);

        let expected_single_constraints = HashMap::from_iter(
            expected_single_constraints
                .into_iter()
                .map(|(k, v)| (k, HashSet::from_iter(v))),
        );
        let expected_double_constraints = HashMap::from_iter(
            expected_double_constraints
                .into_iter()
                .map(|((loc1, loc2), v)| (minmax(loc1, loc2).into(), HashSet::from_iter(v))),
        );

        let single_constraints: HashMap<_, _> = optimizer
            .single_constraints
            .into_iter()
            .filter_map(|(k, v)| if v.len() > 0 { Some((k, v)) } else { None })
            .collect();
        assert_eq!(single_constraints, expected_single_constraints);
        assert_eq!(optimizer.double_constraints, expected_double_constraints);
    }

    #[test_case(
        {
            let location = Location::new();
            (
                location.clone(),
                (Vec::new(), Vec::new()),
                vec![location]
            )
        };
        "no constraints"
    )]
    #[test_case(
        {
            let a = Location::new();
            let b = Location::new();
            let c = Location::new();
            let d = Location::new();
            let e = Location::new();
            let f = Location::new();
            (
                a.clone(),
                (
                    vec![
                        (a.clone(), vec![b.clone()]),
                        (b.clone(), vec![a.clone(), c.clone()]),
                        (c.clone(), vec![e.clone()]),
                        (d.clone(), vec![e.clone()]),
                        (e.clone(), Vec::new()),
                        (f.clone(), vec![d.clone()]),
                    ],
                    Vec::new()
                ),
                vec![a,b,c,e]
            )
        };
        "single constraints only"
    )]
    #[test_case(
        {
            let a = Location::new();
            let b = Location::new();
            let c = Location::new();
            let d = Location::new();
            let e = Location::new();
            let f = Location::new();
            (
                a.clone(),
                (
                    vec![
                        (a.clone(), vec![b.clone()]),
                        (e.clone(), vec![d.clone()]),
                    ],
                    vec![
                        ((a.clone(), b.clone()), vec![c.clone()]),
                        ((b.clone(), c.clone()), vec![a.clone(),f.clone()]),
                        ((a.clone(), d.clone()), vec![e.clone()]),
                    ]
                ),
                vec![a,b,c,f]
            )
        };
        "mixed constraints"
    )]
    fn test_solving_constraints(
        initial_constraints_solution: (
            Location,
            (
                Vec<(Location, Vec<Location>)>,
                Vec<((Location, Location), Vec<Location>)>,
            ),
            Vec<Location>,
        ),
    ) {
        let (initial_solution, (single_constraints, double_constraints), expected_solution) =
            initial_constraints_solution;
        let mut optimizer = Optimizer::new();
        for (k, v) in single_constraints {
            optimizer.add_single_constraint(k, v);
        }
        for ((l1, l2), v) in double_constraints {
            optimizer.add_double_constraint(l1, l2, v);
        }
        let solution = optimizer.solve_constraints(initial_solution);
        assert_eq!(solution, HashSet::from_iter(expected_solution));
    }

    #[test_case(
        {
            let w = Location::new();
            let x = Location::new();
            let y = Location::new();
            let z = Location::new();
            let main = Location::new();
            let unused = Location::new();

            (
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: unused.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: false})).into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: x.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 3})).into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: w.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: -1})).into()
                                            ))
                                        }.into(),
                                        IntermediateAssignment{
                                            location: y.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    fn_: IntermediateBuiltIn::BuiltInFn(
                                                        Id::from("--"),
                                                        IntermediateFnType(
                                                            vec![AtomicTypeEnum::INT.into()],
                                                            Box::new(AtomicTypeEnum::INT.into())
                                                        ).into()
                                                    ).into(),
                                                    args: vec![
                                                        x.clone().into()
                                                    ]
                                                }.into(),
                                            ))
                                        }.into(),
                                        IntermediateAssignment{
                                            location: z.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    fn_: IntermediateBuiltIn::BuiltInFn(
                                                        Id::from("++"),
                                                        IntermediateFnType(
                                                            vec![AtomicTypeEnum::INT.into()],
                                                            Box::new(AtomicTypeEnum::INT.into())
                                                        ).into()
                                                    ).into(),
                                                    args: vec![
                                                        w.clone().into()
                                                    ]
                                                }.into(),
                                            ))
                                        }.into(),
                                    ],
                                    ret: (
                                        y.clone().into(),
                                        AtomicTypeEnum::INT.into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    types: Vec::new(),
                    main: main.clone().into()
                },
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: x.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 3})).into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: y.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    fn_: IntermediateBuiltIn::BuiltInFn(
                                                        Id::from("--"),
                                                        IntermediateFnType(
                                                            vec![AtomicTypeEnum::INT.into()],
                                                            Box::new(AtomicTypeEnum::INT.into())
                                                        ).into()
                                                    ).into(),
                                                    args: vec![
                                                        x.clone().into()
                                                    ]
                                                }.into(),
                                            ))
                                        }.into(),
                                    ],
                                    ret: (
                                        y.clone().into(),
                                        AtomicTypeEnum::INT.into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    types: Vec::new(),
                    main: main.clone().into()
                },
            )
        };
        "unused variables"
    )]
    #[test_case(
        {
            let c = Location::new();
            let w = Location::new();
            let x = Location::new();
            let y = Location::new();
            let z = Location::new();
            let main = Location::new();
            (
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: c.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into()
                            ))
                        }.into(),
                        IntermediateIfStatement{
                            condition: c.clone().into(),
                            branches: (
                                vec![
                                    IntermediateAssignment{
                                        location: x.clone().into(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                        ))
                                    }.into(),
                                    IntermediateAssignment{
                                        location: z.clone().into(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                        ))
                                    }.into(),
                                ],
                                vec![
                                    IntermediateAssignment{
                                        location: y.clone(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 4})).into()
                                        ))
                                    }.into(),
                                    IntermediateAssignment{
                                        location: w.clone().into(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 7})).into()
                                        ))
                                    }.into(),
                                    IntermediateAssignment{
                                        location: z.clone().into(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(y.clone()).into()
                                        ))
                                    }.into(),
                                ],
                            )
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: Vec::new(),
                                    ret: (
                                        z.clone().into(),
                                        AtomicTypeEnum::INT.into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: Vec::new()
                },
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: c.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into()
                            ))
                        }.into(),
                        IntermediateIfStatement{
                            condition: c.clone().into(),
                            branches: (
                                vec![
                                    IntermediateAssignment{
                                        location: z.clone().into(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                        ))
                                    }.into(),
                                ],
                                vec![
                                    IntermediateAssignment{
                                        location: y.clone(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 4})).into()
                                        ))
                                    }.into(),
                                    IntermediateAssignment{
                                        location: z.clone().into(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(y.clone()).into()
                                        ))
                                    }.into(),
                                ],
                            )
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: Vec::new(),
                                    ret: (
                                        z.clone().into(),
                                        AtomicTypeEnum::INT.into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: Vec::new()
                }
            )
        };
        "unused in if statement"
    )]
    #[test_case(
        {
            let c = Location::new();
            let x = Location::new();
            let y = Location::new();
            let main = Location::new();
            (
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: c.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateValue::from(IntermediateBuiltIn::from(Boolean{value: true})).into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: y.clone().into(),
                            expression: Rc::new(RefCell::new(
                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 2})).into()
                            ))
                        }.into(),
                        IntermediateIfStatement{
                            condition: c.clone().into(),
                            branches: (
                                vec![
                                    IntermediateAssignment{
                                        location: x.clone().into(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                        ))
                                    }.into(),
                                ],
                                vec![
                                    IntermediateAssignment{
                                        location: x.clone().into(),
                                        expression: Rc::new(RefCell::new(
                                            IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 1})).into()
                                        ))
                                    }.into(),
                                ],
                            )
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: Vec::new(),
                                    ret: (
                                        y.clone().into(),
                                        AtomicTypeEnum::INT.into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: Vec::new()
                },
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: y.clone().into(),
                            expression: Rc::new(RefCell::new(
                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 2})).into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: Vec::new(),
                                    ret: (
                                        y.clone().into(),
                                        AtomicTypeEnum::INT.into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: Vec::new()
                },
            )
        };
        "unused function argument"
    )]
    #[test_case(
        {
            let s = Location::new();
            let w = Location::new();
            let x = Location::new();
            let y = Location::new();
            let z = Location::new();
            let main = Location::new();
            let types = vec![
                Rc::new(RefCell::new(IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into()),]).into()))
            ];
            (
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: s.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateCtorCall{
                                    idx: 0,
                                    data: None,
                                    type_: IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into()),])
                                }.into()
                            ))
                        }.into(),
                        IntermediateMatchStatement{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch {
                                    target: Some(
                                        IntermediateArg {
                                            type_: AtomicTypeEnum::INT.into(),
                                            location: Location::new()
                                        }
                                    ),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: x.clone().into(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                            ))
                                        }.into(),
                                        IntermediateAssignment{
                                            location: z.clone().into(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                            ))
                                        }.into(),
                                    ],
                                },
                                IntermediateMatchBranch {
                                    target: Some(IntermediateArg {
                                        type_: AtomicTypeEnum::INT.into(),
                                        location: y.clone()
                                    }),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: w.clone().into(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 7})).into()
                                            ))
                                        }.into(),
                                        IntermediateAssignment{
                                            location: z.clone().into(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(y.clone()).into()
                                            ))
                                        }.into(),
                                    ],
                                }
                            ],
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: Vec::new(),
                                    ret: (
                                        z.clone().into(),
                                        AtomicTypeEnum::INT.into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: types.clone()
                },
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: s.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateCtorCall{
                                    idx: 0,
                                    data: None,
                                    type_: IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into()),])
                                }.into()
                            ))
                        }.into(),
                        IntermediateMatchStatement{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch {
                                    target: None,
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: z.clone().into(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(IntermediateBuiltIn::from(Integer{value: 0})).into()
                                            ))
                                        }.into(),
                                    ],
                                },
                                IntermediateMatchBranch {
                                    target: Some(IntermediateArg {
                                        type_: AtomicTypeEnum::INT.into(),
                                        location: y.clone()
                                    }),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: z.clone().into(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(y.clone()).into()
                                            ))
                                        }.into(),
                                    ],
                                }
                            ],
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: Vec::new(),
                                    ret: (
                                        z.clone().into(),
                                        AtomicTypeEnum::INT.into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: types.clone()
                },
            )
        };
        "unused in match statement"
    )]
    #[test_case(
        {
            let s = Location::new();
            let x = Location::new();
            let y = Location::new();
            let z = Location::new();
            let main = Location::new();
            let types = vec![
                Rc::new(RefCell::new(IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into()),]).into()))
            ];
            (
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: s.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateCtorCall{
                                    idx: 0,
                                    data: None,
                                    type_: IntermediateUnionType(vec![Some(AtomicTypeEnum::INT.into()),Some(AtomicTypeEnum::INT.into()),])
                                }.into()
                            ))
                        }.into(),
                        IntermediateMatchStatement{
                            subject: s.clone().into(),
                            branches: vec![
                                IntermediateMatchBranch {
                                    target: Some(
                                        IntermediateArg {
                                            type_: AtomicTypeEnum::INT.into(),
                                            location: x.clone()
                                        }
                                    ),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: z.clone().into(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(x.clone()).into()
                                            ))
                                        }.into(),
                                    ],
                                },
                                IntermediateMatchBranch {
                                    target: Some(IntermediateArg {
                                        type_: AtomicTypeEnum::INT.into(),
                                        location: y.clone()
                                    }),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: z.clone().into(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateValue::from(y.clone()).into()
                                            ))
                                        }.into(),
                                    ],
                                }
                            ],
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: Vec::new(),
                                    ret: (
                                        IntermediateBuiltIn::from(Integer{value: -8}).into(),
                                        AtomicTypeEnum::INT.into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: types.clone()
                },
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: Vec::new(),
                                    ret: (
                                        IntermediateBuiltIn::from(Integer{value: -8}).into(),
                                        AtomicTypeEnum::INT.into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: types.clone()
                },
            )
        };
        "unused match statement"
    )]
    #[test_case(
        {
            let foo = Location::new();
            let foo_opt = Location::new();
            let foo_opt_call = Location::new();
            let foo_call = Location::new();
            let foo_main_call = Location::new();
            let arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let apply = Location::new();
            let apply_call = Location::new();
            let f_call = Location::new();
            let f = IntermediateArg::from(IntermediateType::from(IntermediateFnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into()),
            )));
            let x = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let tuple = Location::new();
            let main = Location::new();
            (
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: foo.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: vec![arg.clone()],
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: foo_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: vec![arg.clone().into()],
                                                    fn_: foo.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (foo_call.clone().into(), AtomicTypeEnum::INT.into())
                                }.into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: apply.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: vec![f.clone(), x.clone()],
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: f_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: vec![x.clone().into()],
                                                    fn_: f.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (f_call.clone().into(), AtomicTypeEnum::INT.into())
                                }.into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: foo_main_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: vec![IntermediateBuiltIn::from(Integer{value: 3}).into()],
                                                    fn_: foo.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                        IntermediateAssignment{
                                            location: apply_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: vec![
                                                        foo.clone().into(),
                                                        IntermediateBuiltIn::from(Integer{value: 3}).into()
                                                    ],
                                                    fn_: apply.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                        IntermediateAssignment{
                                            location: tuple.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateTupleExpression(vec![
                                                    foo_main_call.clone().into(),
                                                    apply_call.clone().into(),
                                                ]).into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (
                                        tuple.clone().into(),
                                        IntermediateTupleType(vec![
                                            AtomicTypeEnum::INT.into(),
                                            AtomicTypeEnum::INT.into(),
                                        ]).into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: Vec::new()
                },
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: foo_opt.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: foo_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: Vec::new(),
                                                    fn_: foo_opt.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (foo_call.clone().into(), AtomicTypeEnum::INT.into())
                                }.into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: foo.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: vec![arg.clone().into()],
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: foo_opt_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: Vec::new(),
                                                    fn_: foo_opt.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (foo_opt_call.clone().into(), AtomicTypeEnum::INT.into())
                                }.into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: apply.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: vec![f.clone(), x.clone()],
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: f_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: vec![x.clone().into()],
                                                    fn_: f.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (f_call.clone().into(), AtomicTypeEnum::INT.into())
                                }.into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: foo_main_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: Vec::new(),
                                                    fn_: foo_opt.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                        IntermediateAssignment{
                                            location: apply_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: vec![
                                                        foo.clone().into(),
                                                        IntermediateBuiltIn::from(Integer{value: 3}).into()
                                                    ],
                                                    fn_: apply.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                        IntermediateAssignment{
                                            location: tuple.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateTupleExpression(vec![
                                                    foo_main_call.clone().into(),
                                                    apply_call.clone().into(),
                                                ]).into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (
                                        tuple.clone().into(),
                                        IntermediateTupleType(vec![
                                            AtomicTypeEnum::INT.into(),
                                            AtomicTypeEnum::INT.into(),
                                        ]).into()
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: Vec::new()
                },
            )
        };
        "unused argument"
    )]
    #[test_case(
        {
            let foo = Location::new();
            let foo_opt = Location::new();
            let foo_un_opt_call = Location::new();
            let foo_call = Location::new();
            let main_call = Location::new();
            let bar = Location::new();
            let bar_opt = Location::new();
            let bar_un_opt_call = Location::new();
            let bar_call = Location::new();
            let foo_arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let bar_arg = IntermediateArg::from(IntermediateType::from(AtomicTypeEnum::INT));
            let main = Location::new();
            (
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: foo.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: vec![foo_arg.clone()],
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: foo_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: vec![foo_arg.clone().into()],
                                                    fn_: bar.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (foo_call.clone().into(), AtomicTypeEnum::INT.into())
                                }.into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: bar.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: vec![bar_arg.clone()],
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: bar_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: vec![bar_arg.clone().into()],
                                                    fn_: foo.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (bar_call.clone().into(), AtomicTypeEnum::INT.into())
                                }.into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: main_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: vec![IntermediateBuiltIn::from(Integer{value: 3}).into()],
                                                    fn_: foo.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (
                                        main_call.clone().into(),
                                        AtomicTypeEnum::INT.into(),
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: Vec::new()
                },
                IntermediateProgram{
                    statements: vec![
                        IntermediateAssignment{
                            location: foo_opt.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: foo_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: Vec::new(),
                                                    fn_: bar_opt.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (foo_call.clone().into(), AtomicTypeEnum::INT.into())
                                }.into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: foo.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: vec![foo_arg.clone().into()],
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: foo_un_opt_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: Vec::new(),
                                                    fn_: foo_opt.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (foo_un_opt_call.clone().into(), AtomicTypeEnum::INT.into())
                                }.into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: bar_opt.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: bar_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: Vec::new(),
                                                    fn_: foo_opt.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (bar_call.clone().into(), AtomicTypeEnum::INT.into())
                                }.into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: bar.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: vec![bar_arg.clone().into()],
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: bar_un_opt_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: Vec::new(),
                                                    fn_: bar_opt.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (bar_un_opt_call.clone().into(), AtomicTypeEnum::INT.into())
                                }.into()
                            ))
                        }.into(),
                        IntermediateAssignment{
                            location: main.clone(),
                            expression: Rc::new(RefCell::new(
                                IntermediateFnDef{
                                    args: Vec::new(),
                                    statements: vec![
                                        IntermediateAssignment{
                                            location: main_call.clone(),
                                            expression: Rc::new(RefCell::new(
                                                IntermediateFnCall{
                                                    args: Vec::new(),
                                                    fn_: foo_opt.clone().into()
                                                }.into()
                                            ))
                                        }.into(),
                                    ],
                                    ret: (
                                        main_call.clone().into(),
                                        AtomicTypeEnum::INT.into(),
                                    )
                                }.into()
                            ))
                        }.into(),
                    ],
                    main: main.clone().into(),
                    types: Vec::new()
                },
            )
        };
        "unused shared arguments"
    )]
    fn test_remove_program_dead_code(program_expected: (IntermediateProgram, IntermediateProgram)) {
        let (program, expected_program) = program_expected;
        let optimized_program = Optimizer::remove_dead_code(program);
        dbg!(&optimized_program);
        dbg!(&expected_program);
        let optimized_fn = IntermediateFnDef {
            args: Vec::new(),
            statements: optimized_program.statements,
            ret: (
                optimized_program.main,
                IntermediateTupleType(Vec::new()).into(),
            ),
        }
        .into();
        let expected_fn = IntermediateFnDef {
            args: Vec::new(),
            statements: expected_program.statements,
            ret: (
                expected_program.main,
                IntermediateTupleType(Vec::new()).into(),
            ),
        }
        .into();
        assert_eq!(optimized_program.types, expected_program.types);
        assert!(ExpressionEqualityChecker::equal(
            &optimized_fn,
            &expected_fn
        ))
    }
}
