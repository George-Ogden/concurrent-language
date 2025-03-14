use std::collections::HashMap;

use crate::{
    fn_inst::{FnDefs, FnInst},
    IntermediateAssignment, IntermediateExpression, IntermediateFnCall, IntermediateLambda,
    IntermediateMemory, IntermediateProgram, IntermediateStatement, IntermediateValue, Location,
};
use itertools::Either::{Left, Right};

type RecursiveFns = HashMap<Location, bool>;

struct RecursiveFnFinder {
    fn_defs: FnDefs,
}

impl RecursiveFnFinder {
    pub fn recursive_fns(program: IntermediateProgram) -> RecursiveFns {
        let lambda = &program.main;
        let mut finder = RecursiveFnFinder {
            fn_defs: FnDefs::new(),
        };
        FnInst::collect_fn_defs_from_statements(&lambda.block.statements, &mut finder.fn_defs);
        let mut recursive_fns = RecursiveFns::new();
        for fn_ in finder.fn_defs.keys() {
            let is_recursive = finder.find(&fn_, &mut recursive_fns);
            recursive_fns.insert(fn_.clone(), is_recursive);
        }
        recursive_fns
    }
    fn find(&self, fn_: &Location, recursive_fns: &mut RecursiveFns) -> bool {
        if recursive_fns.contains_key(fn_) {
            return recursive_fns[fn_];
        }
        recursive_fns.insert(fn_.clone(), true);
        match FnInst::get_root_fn(&self.fn_defs, fn_) {
            Some(Left((location, lambda))) => {
                let is_recursive = self.check(&lambda.block.statements, recursive_fns);
                recursive_fns.insert(location, is_recursive);
                is_recursive
            }
            Some(Right(_)) => false,
            None => true,
        }
    }
    fn check(
        &self,
        statements: &Vec<IntermediateStatement>,
        recursive_fns: &mut HashMap<Location, bool>,
    ) -> bool {
        statements.iter().any(|statement| match statement {
            IntermediateStatement::IntermediateAssignment(IntermediateAssignment {
                expression,
                location: _,
            }) => match expression {
                IntermediateExpression::IntermediateFnCall(IntermediateFnCall { fn_, args: _ }) => {
                    match fn_ {
                        IntermediateValue::IntermediateBuiltIn(_) => false,
                        IntermediateValue::IntermediateMemory(IntermediateMemory {
                            type_: _,
                            location,
                        }) => self.find(location, recursive_fns),
                        IntermediateValue::IntermediateArg(_) => true,
                    }
                }
                IntermediateExpression::IntermediateIf(if_) => {
                    self.check(&if_.branches.0.statements, recursive_fns)
                        || self.check(&if_.branches.1.statements, recursive_fns)
                }
                IntermediateExpression::IntermediateMatch(match_) => match_
                    .branches
                    .iter()
                    .any(|branch| self.check(&branch.block.statements, recursive_fns)),
                _ => false,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        IntermediateArg, IntermediateAssignment, IntermediateBlock, IntermediateFnCall,
        IntermediateFnType, IntermediateIf, IntermediateMemory, IntermediateStatement,
        IntermediateType, Location,
    };

    use super::*;

    use test_case::test_case;
    use type_checker::{AtomicTypeEnum, Boolean, Integer};

    #[test_case(
        {
            (
                Vec::new(),
                HashMap::new()
            )
        };
        "empty fns"
    )]
    #[test_case(
        {
            let identity = Location::new();
            let arg: IntermediateArg = IntermediateType::from(AtomicTypeEnum::BOOL).into();
            (
                vec![
                    IntermediateAssignment{
                        location: identity.clone(),
                        expression: IntermediateLambda {
                            args: vec![arg.clone()],
                            block: IntermediateBlock {
                                statements: Vec::new(),
                                ret: arg.clone().into()
                            },
                        }.into()
                    }.into(),
                ],
                HashMap::from([
                    (identity, false)
                ])
            )
        };
        "identity fn"
    )]
    #[test_case(
        {
            let call = IntermediateMemory{
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new()
            };
            let fn_ = Location::new();
            (
                vec![
                    IntermediateAssignment{
                        location: fn_.clone(),
                        expression: IntermediateLambda {
                            block: IntermediateBlock{
                                statements: vec![
                                    IntermediateAssignment{
                                        location: call.location.clone(),
                                        expression: IntermediateFnCall{
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
                                    }.into()
                                ],
                                ret: call.clone().into()
                            },
                            args: Vec::new()
                        }.into()
                    }.into()
                ],
                HashMap::from([
                    (fn_, true)
                ])
            )
        };
        "higher order fn"
    )]
    #[test_case(
        {
            let id_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let id = IntermediateLambda {
                args: vec![id_arg.clone()],
                block: IntermediateBlock {
                    statements: Vec::new(),
                    ret: id_arg.clone().into(),
                },
            };
            let id_fn = IntermediateMemory {
                type_: IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                )
                .into(),
                location: Location::new(),
            };

            let idea_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let idea_ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let idea_fn = IntermediateLambda {
                args: vec![idea_arg.clone()],
                block: IntermediateBlock {
                    statements: vec![
                        IntermediateAssignment {
                            location: id_fn.location.clone(),
                            expression: id.clone().into(),
                        }
                        .into(),
                        IntermediateAssignment {
                            location: idea_ret.location.clone(),
                            expression: IntermediateFnCall {
                                fn_: id_fn.clone().into(),
                                args: vec![idea_arg.clone().into()],
                            }
                            .into(),
                        }
                        .into(),
                    ],
                    ret: idea_ret.clone().into(),
                },
            };
            let idea = Location::new();
            (
                vec![
                    IntermediateAssignment{
                        location: idea.clone(),
                        expression: idea_fn.into()
                    }.into()
                ],
                HashMap::from([
                    (idea.clone(), false),
                    (id_fn.location.clone(), false)
                ])
            )
        };
        "internal non-recursive fn call"
    )]
    #[test_case(
        {
            let id_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let id = IntermediateLambda {
                args: vec![id_arg.clone()],
                block: IntermediateBlock {
                    statements: Vec::new(),
                    ret: id_arg.clone().into(),
                },
            };
            let id_fn = IntermediateMemory {
                type_: IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                )
                .into(),
                location: Location::new(),
            };

            let idea_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let idea_ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let idea_fn = IntermediateLambda {
                args: vec![idea_arg.clone()],
                block: IntermediateBlock {
                    statements: vec![
                        IntermediateAssignment {
                            location: idea_ret.location.clone(),
                            expression: IntermediateFnCall {
                                fn_: id_fn.clone().into(),
                                args: vec![idea_arg.clone().into()],
                            }
                            .into(),
                        }
                        .into(),
                    ],
                    ret: idea_ret.clone().into(),
                },
            };
            let idea = Location::new();
            (
                vec![
                    IntermediateAssignment {
                        location: id_fn.location.clone(),
                        expression: id.clone().into(),
                    }
                    .into(),
                    IntermediateAssignment{
                        location: idea.clone(),
                        expression: idea_fn.into()
                    }.into()
                ],
                HashMap::from([
                    (idea.clone(), false),
                    (id_fn.location.clone(), false)
                ])
            )
        };
        "external non-recursive fn call"
    )]
    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        location: foo_call.location.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: foo.clone().into(),
                                            args: Vec::new()
                                        }.into()
                                    }.into()
                                ],
                                ret: foo_call.clone().into()
                            },
                        }.into(),
                        location: foo.location.clone()
                    }.into(),
                ],
                HashMap::from([
                    (foo.location.clone(), true)
                ])
            )
        };
        "self-recursive fn"
    )]
    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let bar_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        location: bar_call.location.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: bar.clone().into(),
                                            args: Vec::new()
                                        }.into()
                                    }.into()
                                ],
                                ret: bar_call.clone().into()
                            },
                        }.into(),
                        location: foo.location.clone()
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        location: foo_call.location.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: foo.clone().into(),
                                            args: Vec::new()
                                        }.into()
                                    }.into()
                                ],
                                ret: foo_call.clone().into()
                            },
                        }.into(),
                        location: bar.location.clone()
                    }.into(),
                ],
                HashMap::from([
                    (foo.location.clone(), true),
                    (bar.location.clone(), true)
                ])
            )
        };
        "mutually recursive fns"
    )]
    #[test_case(
        {
            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));

            let idea_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let idea_ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let idea_fn = IntermediateLambda {
                args: vec![idea_arg.clone()],
                block: IntermediateBlock {
                    statements: vec![
                        IntermediateAssignment {
                            location: idea_ret.location.clone(),
                            expression: IntermediateFnCall {
                                fn_: foo.clone().into(),
                                args: Vec::new(),
                            }
                            .into(),
                        }
                        .into(),
                    ],
                    ret: idea_ret.clone().into(),
                },
            };
            let idea = Location::new();
            (
                vec![
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        location: foo_call.location.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: foo.clone().into(),
                                            args: Vec::new()
                                        }.into()
                                    }.into()
                                ],
                                ret: foo_call.clone().into()
                            },
                        }.into(),
                        location: foo.location.clone()
                    }.into(),
                    IntermediateAssignment{
                        location: idea.clone(),
                        expression: idea_fn.into()
                    }.into()
                ],
                HashMap::from([
                    (idea.clone(), true),
                    (foo.location.clone(), true)
                ])
            )
        };
        "recursive fn call"
    )]
    #[test_case(
        {
            let id_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let id = IntermediateLambda {
                args: vec![id_arg.clone()],
                block: IntermediateBlock {
                    statements: Vec::new(),
                    ret: id_arg.clone().into(),
                },
            };
            let id_fn = IntermediateMemory {
                type_: IntermediateFnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                )
                .into(),
                location: Location::new(),
            };

            let foo = IntermediateMemory::from(
                IntermediateType::from(IntermediateFnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::INT.into()),
                ))
            );
            let foo_call = IntermediateMemory::from(IntermediateType::from(AtomicTypeEnum::INT));

            let idea_arg = IntermediateArg {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let idea_ret = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let call_a = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let call_b = IntermediateMemory {
                type_: AtomicTypeEnum::INT.into(),
                location: Location::new(),
            };
            let idea_fn = IntermediateLambda {
                args: vec![idea_arg.clone()],
                block: IntermediateBlock {
                    statements: vec![
                        IntermediateAssignment {
                            location: idea_ret.location.clone(),
                            expression: IntermediateIf {
                                condition: Boolean{value: true}.into(),
                                branches: (
                                    IntermediateBlock{
                                        statements: vec![
                                            IntermediateAssignment{
                                                location: call_a.location.clone(),
                                                expression: IntermediateFnCall{
                                                    fn_: foo.clone().into(),
                                                    args: Vec::new()
                                                }.into()
                                            }.into()
                                        ],
                                        ret: call_a.clone().into()
                                    },
                                    IntermediateBlock{
                                        statements: vec![
                                            IntermediateAssignment{
                                                location: call_b.location.clone(),
                                                expression: IntermediateFnCall{
                                                    fn_: id_fn.clone().into(),
                                                    args: vec![
                                                        idea_arg.clone().into()
                                                    ]
                                                }.into()
                                            }.into()
                                        ],
                                        ret: call_b.clone().into()
                                    },
                                )
                            }.into()
                        }
                        .into(),
                    ],
                    ret: idea_ret.clone().into(),
                },
            };
            let idea = Location::new();
            (
                vec![
                    IntermediateAssignment{
                        expression: id.clone().into(),
                        location: id_fn.location.clone(),
                    }.into(),
                    IntermediateAssignment{
                        expression: IntermediateLambda{
                            args: Vec::new(),
                            block: IntermediateBlock {
                                statements: vec![
                                    IntermediateAssignment{
                                        location: foo_call.location.clone(),
                                        expression: IntermediateFnCall{
                                            fn_: foo.clone().into(),
                                            args: Vec::new()
                                        }.into()
                                    }.into()
                                ],
                                ret: foo_call.clone().into()
                            },
                        }.into(),
                        location: foo.location.clone()
                    }.into(),
                    IntermediateAssignment{
                        location: idea.clone(),
                        expression: idea_fn.into()
                    }.into()
                ],
                HashMap::from([
                    (idea.clone(), true),
                    (id_fn.location.clone(),false),
                    (foo.location.clone(), true)
                ])
            )
        };
        "mixed fn call"
    )]
    fn test_recursive_fn_finder(fns: (Vec<IntermediateStatement>, RecursiveFns)) {
        let (statements, recursive) = fns;
        let recursive_fns = RecursiveFnFinder::recursive_fns(IntermediateProgram {
            main: IntermediateLambda {
                args: Vec::new(),
                block: IntermediateBlock {
                    statements,
                    ret: Integer { value: 0 }.into(),
                },
            },
            types: Vec::new(),
        });
        assert_eq!(recursive_fns, recursive);
    }
}
