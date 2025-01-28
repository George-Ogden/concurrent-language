use std::{
    cmp::{max, min},
    collections::{HashMap, HashSet},
};

use itertools::zip_eq;
use lowering::{
    IntermediateArg, IntermediateAssignment, IntermediateExpression, IntermediateFnCall,
    IntermediateFnDef, IntermediateStatement, IntermediateValue, Location,
};

struct Optimizer {
    single_constraints: HashMap<Location, HashSet<Location>>,
    double_constraints: HashMap<(Location, Location), HashSet<Location>>,
    fn_args: HashMap<Location, Vec<Location>>,
}

impl Optimizer {
    fn new() -> Self {
        Optimizer {
            single_constraints: HashMap::new(),
            double_constraints: HashMap::new(),
            fn_args: HashMap::new(),
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
        let key = (min(arg.clone(), location.clone()), max(arg, location));
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
                        let dependents =
                            self.used_value(&ret.0).map(|v| vec![v]).unwrap_or_default();
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
                            for (loc, arg) in zip_eq(self.fn_args[&fn_].clone(), args) {
                                let dependents =
                                    self.used_value(arg).map(|x| vec![x]).unwrap_or_default();
                                self.add_double_constraint(loc, location.clone(), dependents)
                            }
                        }
                        lowering::IntermediateValue::IntermediateArg(IntermediateArg {
                            type_: _,
                            location: arg_loc,
                        }) => {
                            self.add_single_constraint(location.clone(), vec![arg_loc.clone()]);
                            let dependents = args
                                .iter()
                                .filter_map(|value| self.used_value(value))
                                .collect();
                            self.add_single_constraint(location.clone(), dependents);
                        }
                    },
                    expression => {
                        let used_values = self.find_used_values(&expression);
                        self.add_single_constraint(location.clone(), used_values)
                    }
                },
                IntermediateStatement::IntermediateIfStatement(_) => todo!(),
                IntermediateStatement::IntermediateMatchStatement(_) => todo!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use std::{
        cell::RefCell,
        cmp::{max, min},
        rc::Rc,
    };

    use super::*;

    use lowering::{
        AtomicTypeEnum, Boolean, Id, Integer, IntermediateArg, IntermediateBuiltIn,
        IntermediateElementAccess, IntermediateFnCall, IntermediateFnDef, IntermediateFnType,
        IntermediateStatement, IntermediateTupleExpression, IntermediateType, IntermediateValue,
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
                .map(|((loc1, loc2), v)| {
                    (
                        (min(loc1.clone(), loc2.clone()), max(loc1, loc2)),
                        HashSet::from_iter(v),
                    )
                }),
        );

        let single_constraints: HashMap<_, _> = optimizer
            .single_constraints
            .into_iter()
            .filter_map(|(k, v)| if v.len() > 0 { Some((k, v)) } else { None })
            .collect();
        assert_eq!(single_constraints, expected_single_constraints);
        assert_eq!(optimizer.double_constraints, expected_double_constraints);
    }
}
