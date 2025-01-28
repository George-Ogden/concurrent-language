use std::{
    cmp::{max, min},
    collections::{HashMap, HashSet},
};

use itertools::zip_eq;
use lowering::{
    IntermediateArg, IntermediateExpression, IntermediateFnCall, IntermediateFnDef,
    IntermediateMemory, IntermediateStatement, Location,
};

struct Optimizer {
    arg_translation: HashMap<IntermediateArg, Location>,
    single_constraints: HashMap<Location, HashSet<Location>>,
    double_constraints: HashMap<(Location, Location), HashSet<Location>>,
    fn_args: HashMap<Location, Vec<Location>>,
}

impl Optimizer {
    fn new() -> Self {
        Optimizer {
            arg_translation: HashMap::new(),
            single_constraints: HashMap::new(),
            double_constraints: HashMap::new(),
            fn_args: HashMap::new(),
        }
    }
    fn translate_arg(&self, arg: IntermediateArg) -> Location {
        self.arg_translation[&arg].clone()
    }
    fn find_used_values(&mut self, expression: &IntermediateExpression) -> Vec<Location> {
        let values = expression.values();
        values
            .into_iter()
            .filter_map(|value| match value {
                lowering::IntermediateValue::IntermediateMemory(location) => Some(location),
                lowering::IntermediateValue::IntermediateArg(arg) => Some(self.translate_arg(arg)),
                lowering::IntermediateValue::IntermediateBuiltIn(_) => None,
            })
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
                IntermediateStatement::Assignment(IntermediateMemory {
                    expression,
                    location,
                }) => match &expression.borrow().clone() {
                    IntermediateExpression::IntermediateFnDef(IntermediateFnDef {
                        args,
                        statements,
                        ret,
                    }) => {
                        for arg in args {
                            self.arg_translation.insert(arg.clone(), Location::new());
                        }
                        let args = args
                            .into_iter()
                            .map(|arg| self.translate_arg(arg.clone()))
                            .collect();
                        self.fn_args.insert(location.clone(), args);
                        self.generate_constraints(statements);
                        let dependents = self.find_used_values(&ret.0.clone().into());
                        self.add_single_constraint(location.clone(), dependents);
                    }
                    IntermediateExpression::IntermediateFnCall(IntermediateFnCall {
                        fn_,
                        args,
                    }) => match fn_ {
                        lowering::IntermediateValue::IntermediateBuiltIn(_) => {
                            todo!()
                        }
                        lowering::IntermediateValue::IntermediateMemory(fn_) => {
                            self.add_single_constraint(location.clone(), vec![fn_.clone()]);
                            for (loc, arg) in zip_eq(self.fn_args[&fn_].clone(), args) {
                                let dependents = self.find_used_values(&arg.clone().into());
                                self.add_double_constraint(loc, location.clone(), dependents)
                            }
                        }
                        lowering::IntermediateValue::IntermediateArg(_) => todo!(),
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
        IntermediateMemory, IntermediateStatement, IntermediateTupleExpression, IntermediateType,
        IntermediateValue,
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
                Vec::new(),
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
                Vec::new(),
                vec![arg.clone()],
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
                vec![location.clone()],
                vec![arg.clone()],
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
                Vec::new(),
            )
        };
        "element access"
    )]
    fn test_find_used_values(
        expression_locations_args: (IntermediateExpression, Vec<Location>, Vec<IntermediateArg>),
    ) {
        let (expression, expected_locations, expected_args) = expression_locations_args;
        let mut optimizer = Optimizer::new();

        for value in expression.values() {
            match value {
                IntermediateValue::IntermediateArg(arg) => {
                    optimizer
                        .arg_translation
                        .insert(arg.clone(), Location::new());
                }
                _ => {}
            }
        }

        let expected: HashSet<_> = expected_locations
            .into_iter()
            .chain(
                expected_args
                    .into_iter()
                    .map(|arg| optimizer.translate_arg(arg)),
            )
            .collect();
        let locations = optimizer.find_used_values(&expression);
        assert_eq!(HashSet::from_iter(locations), expected);
    }

    #[test_case(
        (
            vec![
                IntermediateStatement::Assignment(IntermediateMemory{
                    expression: Rc::new(RefCell::new(IntermediateValue::from(
                        IntermediateBuiltIn::from(Integer{
                            value: 8
                        })
                    ).into())),
                    location: Location::new()
                })
            ],
            Vec::new(),
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
                    IntermediateStatement::Assignment(IntermediateMemory{
                        expression: Rc::new(RefCell::new(IntermediateTupleExpression(vec![
                            var1.clone().into(), var2.clone().into()
                        ]).into())),
                        location: tuple.clone()
                    }),
                    IntermediateStatement::Assignment(IntermediateMemory{
                        expression: Rc::new(RefCell::new(IntermediateElementAccess{
                            value: tuple.clone().into(),
                            idx: 0
                        }.into())),
                        location: res.clone()
                    })
                ],
                vec![
                    (tuple.clone(), vec![var1.clone(), var2.clone()]),
                    (res.clone(), vec![tuple.clone()]),
                ],
                Vec::new(),
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
            dbg!(&id, &arg, &x, &y);
            (
                vec![
                    IntermediateStatement::Assignment(IntermediateMemory{
                        expression: Rc::new(RefCell::new(IntermediateFnDef{
                            args: vec![arg.clone()],
                            statements: Vec::new(),
                            ret: (arg.clone().into(), AtomicTypeEnum::INT.into())
                        }.into())),
                        location: id.clone()
                    }),
                    IntermediateStatement::Assignment(IntermediateMemory{
                        expression: Rc::new(RefCell::new(IntermediateFnCall{
                            fn_: id.clone().into(),
                            args: vec![
                                x.clone().into()
                            ]
                        }.into())),
                        location: y.clone()
                    })
                ],
                vec![
                    (y.clone(), vec![id.clone()]),
                ],
                vec![
                    (id.clone(), vec![arg.clone()]),
                ],
                vec![
                    (
                        (y.clone(), arg.clone()),
                        vec![x.clone()]
                    )
                ]
            )
        };
        "identity fn"
    )]
    fn test_constraint_generation(
        statements_singles_args_doubles: (
            Vec<IntermediateStatement>,
            Vec<(Location, Vec<Location>)>,
            Vec<(Location, Vec<IntermediateArg>)>,
            Vec<((Location, IntermediateArg), Vec<Location>)>,
        ),
    ) {
        let (
            statements,
            expected_single_constraints,
            expected_arg_constraints,
            expected_double_constraints,
        ) = statements_singles_args_doubles;
        let mut optimizer = Optimizer::new();

        optimizer.generate_constraints(&statements);

        let mut expected_single_constraints = HashMap::from_iter(
            expected_single_constraints
                .into_iter()
                .map(|(k, v)| (k, HashSet::from_iter(v))),
        );
        for (location, values) in expected_arg_constraints {
            if !expected_single_constraints.contains_key(&location) {
                expected_single_constraints.insert(location.clone(), HashSet::new());
            }
            for value in values {
                expected_single_constraints
                    .get_mut(&location)
                    .unwrap()
                    .insert(optimizer.translate_arg(value));
            }
        }
        let expected_double_constraints = HashMap::from_iter(
            expected_double_constraints
                .into_iter()
                .map(|((loc, arg), v)| {
                    let loc2 = optimizer.translate_arg(arg);
                    (
                        (min(loc.clone(), loc2.clone()), max(loc, loc2)),
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
