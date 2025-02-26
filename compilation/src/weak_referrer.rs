use itertools::Itertools;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::{
    Assignment, ClosureInstantiation, Declaration, Expression, MachineType, Memory, Name,
    Statement, TupleExpression, Value,
};

type Node = Memory;
type Cycles = HashMap<Node, Rc<RefCell<HashSet<Node>>>>;
type Graph = HashMap<Node, Vec<Node>>;
type Translation = HashMap<Name, Node>;

#[derive(Debug, Clone, PartialEq)]
struct ClosureCycles {
    fn_translation: Translation,
    cycles: Cycles,
}

impl ClosureCycles {
    fn new() -> Self {
        ClosureCycles {
            fn_translation: HashMap::new(),
            cycles: HashMap::new(),
        }
    }
}

struct WeakReferrer {
    graph: Graph,
}

impl WeakReferrer {
    fn new() -> Self {
        WeakReferrer {
            graph: Graph::new(),
        }
    }
    fn construct_graph(&self, statements: &Vec<Statement>) -> (Graph, Translation) {
        let mut graph = Graph::new();
        let mut translation = Translation::new();
        for statement in statements {
            match statement {
                Statement::Await(_) => {}
                Statement::Declaration(declaration) => {}
                Statement::Assignment(Assignment {
                    target,
                    value: expression,
                }) => {
                    let values = match expression {
                        Expression::Value(value) => vec![value],
                        Expression::TupleExpression(TupleExpression(values)) => {
                            values.iter().collect()
                        }
                        Expression::ClosureInstantiation(ClosureInstantiation { name, env }) => {
                            translation.insert(name.clone(), target.clone());
                            match env {
                                Some(value) => vec![value],
                                None => Vec::new(),
                            }
                        }
                        _ => Vec::new(),
                    };
                    let memory_values = values.iter().filter_map(|&value| match value {
                        Value::BuiltIn(_) => None,
                        Value::Memory(memory) => Some(memory),
                    });
                    graph
                        .entry(target.clone())
                        .or_default()
                        .extend(memory_values.cloned());
                }
                Statement::IfStatement(if_statement) => todo!(),
                Statement::MatchStatement(match_statement) => todo!(),
            }
        }
        (graph, translation)
    }
    fn transpose(&self, graph: &Graph) -> Graph {
        let mut transpose = Graph::new();
        for node in graph.keys() {
            for neighbor in graph.get(node).cloned().unwrap_or_default() {
                transpose
                    .entry(neighbor.clone())
                    .or_default()
                    .push(node.clone());
            }
        }
        transpose
    }
    fn detect_closure_cycles(&mut self, statements: &Vec<Statement>) -> ClosureCycles {
        let mut cycles = ClosureCycles::new();
        (self.graph, cycles.fn_translation) = self.construct_graph(statements);
        let mut visited = HashSet::new();
        let mut order = Vec::new();
        for node in self.graph.keys().cloned().collect_vec() {
            if !visited.contains(&node) {
                self.topsort(&node, &mut visited, &mut order);
            }
        }

        order.reverse();
        self.graph = self.transpose(&self.graph);
        visited = HashSet::new();

        for node in order {
            if !visited.contains(&node) {
                let mut nodes = Vec::new();
                self.topsort(&node, &mut visited, &mut nodes);
                if nodes.len() > 1
                    || self
                        .graph
                        .get(&node)
                        .cloned()
                        .unwrap_or_default()
                        .contains(&node)
                {
                    let cycle = Rc::new(RefCell::new(HashSet::from_iter(nodes.clone())));
                    for node in nodes {
                        cycles.cycles.insert(node.clone(), cycle.clone());
                    }
                }
            }
        }
        self.remove_non_fns(cycles, statements)
    }
    fn topsort(&self, node: &Node, visited: &mut HashSet<Node>, order: &mut Vec<Node>) {
        visited.insert(node.clone());
        for neighbor in self.graph.get(&node).cloned().unwrap_or_default() {
            if !visited.contains(&neighbor) {
                self.topsort(&neighbor, visited, order);
            }
        }
        order.push(node.clone());
    }
    fn remove_non_fns(
        &self,
        ClosureCycles {
            fn_translation,
            cycles,
        }: ClosureCycles,
        statements: &Vec<Statement>,
    ) -> ClosureCycles {
        let fns = statements
            .iter()
            .filter_map(|statement| match statement {
                Statement::Declaration(Declaration {
                    type_: MachineType::FnType(_),
                    memory,
                }) => Some(memory),
                _ => None,
            })
            .collect::<HashSet<_>>();
        let mut filtered = HashSet::new();
        ClosureCycles {
            cycles: HashMap::from_iter(cycles.into_iter().filter_map(|(id, cycle)| {
                if fns.contains(&id) {
                    if !filtered.contains(&cycle.as_ptr()) {
                        filtered.insert(cycle.as_ptr());
                        let filtered_cycle = cycle
                            .borrow()
                            .clone()
                            .into_iter()
                            .filter(|id| fns.contains(id))
                            .collect::<HashSet<_>>();
                        *cycle.borrow_mut() = filtered_cycle;
                    };
                    Some((id, cycle))
                } else {
                    None
                }
            })),
            fn_translation,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Assignment, ClosureInstantiation, Declaration, FnType, Id, Memory, Name, Statement,
        TupleExpression, TupleType,
    };

    use super::*;
    use lowering::AtomicTypeEnum;
    use test_case::test_case;

    #[test_case(
        vec![
            Assignment{
                target: Memory(Id::from("closure")),
                value: ClosureInstantiation{
                    name: Name::from("f"),
                    env: None
                }.into()
            }.into()
        ],
        ClosureCycles{
            fn_translation: HashMap::from([
                (Name::from("f"), Memory(Id::from("closure")))
            ]),
            cycles: HashMap::new()
        };
        "no env"
    )]
    #[test_case(
        vec![
            Declaration{
                memory: Memory(Id::from("env")),
                type_: TupleType(vec![AtomicTypeEnum::INT.into(),AtomicTypeEnum::BOOL.into()]).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env")),
                value: TupleExpression(
                    vec![Memory(Id::from("x")).into(), Memory(Id::from("y")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure")),
                value: ClosureInstantiation{
                    name: Name::from("f"),
                    env: Some(Memory(Id::from("env")).into())
                }.into()
            }.into(),
        ],
        ClosureCycles{
            fn_translation: HashMap::from([
                (Name::from("f"), Memory(Id::from("closure")))
            ]),
            cycles: HashMap::new()
        };
        "no cycles"
    )]
    #[test_case(
        vec![
            Declaration{
                memory: Memory(Id::from("closure")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env")),
                type_: TupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env")),
                value: TupleExpression(
                    vec![Memory(Id::from("x")).into(), Memory(Id::from("closure")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure")),
                value: ClosureInstantiation{
                    name: Name::from("f"),
                    env: Some(Memory(Id::from("env")).into())
                }.into()
            }.into(),
        ],
        ClosureCycles{
            fn_translation: HashMap::from([
                (Name::from("f"), Memory(Id::from("closure")))
            ]),
            cycles: HashMap::from([
                (Memory(Id::from("closure")), Rc::new(RefCell::new(HashSet::from([Memory(Id::from("closure"))]))))
            ])
        };
        "self cycle"
    )]
    #[test_case(
        vec![
            Declaration{
                memory: Memory(Id::from("closure0")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("closure1")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env0")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env1")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env0")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure1")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env1")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure1")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure0")),
                value: ClosureInstantiation{
                    name: Name::from("f0"),
                    env: Some(Memory(Id::from("env0")).into())
                }.into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure1")),
                value: ClosureInstantiation{
                    name: Name::from("f1"),
                    env: Some(Memory(Id::from("env1")).into())
                }.into()
            }.into(),
        ],
        ClosureCycles{
            fn_translation: HashMap::from([
                (Name::from("f0"), Memory(Id::from("closure0"))),
                (Name::from("f1"), Memory(Id::from("closure1"))),
            ]),
            cycles: {
                let cycles = Rc::new(RefCell::new(HashSet::from([
                    Memory(Id::from("closure1")),
                ])));
                HashMap::from([
                    (Memory(Id::from("closure1")), cycles.clone()),
                ])
            }
        };
        "extra self cycle"
    )]
    #[test_case(
        vec![
            Declaration{
                memory: Memory(Id::from("closure0")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("closure1")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("closure2")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env0")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env1")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env2")),
                type_: TupleType(vec![
                    AtomicTypeEnum::INT.into(),
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env0")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure1")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env1")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure2")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env2")),
                value: TupleExpression(
                    vec![Memory(Id::from("x")).into(), Memory(Id::from("closure0")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure0")),
                value: ClosureInstantiation{
                    name: Name::from("f0"),
                    env: Some(Memory(Id::from("env0")).into())
                }.into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure1")),
                value: ClosureInstantiation{
                    name: Name::from("f1"),
                    env: Some(Memory(Id::from("env1")).into())
                }.into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure2")),
                value: ClosureInstantiation{
                    name: Name::from("f2"),
                    env: Some(Memory(Id::from("env2")).into())
                }.into()
            }.into(),
        ],
        ClosureCycles{
            fn_translation: HashMap::from([
                (Name::from("f0"), Memory(Id::from("closure0"))),
                (Name::from("f1"), Memory(Id::from("closure1"))),
                (Name::from("f2"), Memory(Id::from("closure2"))),
            ]),
            cycles: {
                let cycles = Rc::new(RefCell::new(HashSet::from([
                    Memory(Id::from("closure0")),
                    Memory(Id::from("closure1")),
                    Memory(Id::from("closure2")),
                ])));
                HashMap::from([
                    (Memory(Id::from("closure0")), cycles.clone()),
                    (Memory(Id::from("closure1")), cycles.clone()),
                    (Memory(Id::from("closure2")), cycles.clone()),
                ])
            }
        };
        "three cycle"
    )]
    #[test_case(
        vec![
            Declaration{
                memory: Memory(Id::from("closure0")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("closure1")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("closure2")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env0")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into(),
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env1")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into(),
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env2")),
                type_: TupleType(vec![
                    AtomicTypeEnum::INT.into(),
                ]).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env0")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure1")).into(),Memory(Id::from("closure2")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env1")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure0")).into(),Memory(Id::from("closure2")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env2")),
                value: TupleExpression(
                    vec![Memory(Id::from("x")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure0")),
                value: ClosureInstantiation{
                    name: Name::from("f0"),
                    env: Some(Memory(Id::from("env0")).into())
                }.into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure1")),
                value: ClosureInstantiation{
                    name: Name::from("f1"),
                    env: Some(Memory(Id::from("env1")).into())
                }.into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure2")),
                value: ClosureInstantiation{
                    name: Name::from("f2"),
                    env: Some(Memory(Id::from("env2")).into())
                }.into()
            }.into(),
        ],
        ClosureCycles{
            fn_translation: HashMap::from([
                (Name::from("f0"), Memory(Id::from("closure0"))),
                (Name::from("f1"), Memory(Id::from("closure1"))),
                (Name::from("f2"), Memory(Id::from("closure2"))),
            ]),
            cycles: {
                let cycles = Rc::new(RefCell::new(HashSet::from([
                    Memory(Id::from("closure0")),
                    Memory(Id::from("closure1")),
                ])));
                HashMap::from([
                    (Memory(Id::from("closure0")), cycles.clone()),
                    (Memory(Id::from("closure1")), cycles.clone()),
                ])
            }
        };
        "two cycle triangle"
    )]
    #[test_case(
        vec![
            Declaration{
                memory: Memory(Id::from("closure0")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("closure1")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("closure2")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("closure3")),
                type_: FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env0")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into(),
                ]).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env1")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into(),
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env2")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into(),
                ]).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env3")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into(),
                ]).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env0")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure1")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env1")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure2")).into(),Memory(Id::from("closure3")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env2")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure0")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env3")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure0")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure0")),
                value: ClosureInstantiation{
                    name: Name::from("f0"),
                    env: Some(Memory(Id::from("env0")).into())
                }.into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure1")),
                value: ClosureInstantiation{
                    name: Name::from("f1"),
                    env: Some(Memory(Id::from("env1")).into())
                }.into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure2")),
                value: ClosureInstantiation{
                    name: Name::from("f2"),
                    env: Some(Memory(Id::from("env2")).into())
                }.into()
            }.into(),
            Assignment{
                target: Memory(Id::from("closure3")),
                value: ClosureInstantiation{
                    name: Name::from("f3"),
                    env: Some(Memory(Id::from("env3")).into())
                }.into()
            }.into(),
        ],
        ClosureCycles{
            fn_translation: HashMap::from([
                (Name::from("f0"), Memory(Id::from("closure0"))),
                (Name::from("f1"), Memory(Id::from("closure1"))),
                (Name::from("f2"), Memory(Id::from("closure2"))),
                (Name::from("f3"), Memory(Id::from("closure3"))),
            ]),
            cycles: {
                let cycles = Rc::new(RefCell::new(HashSet::from([
                    Memory(Id::from("closure0")),
                    Memory(Id::from("closure1")),
                    Memory(Id::from("closure2")),
                    Memory(Id::from("closure3")),
                ])));
                HashMap::from([
                    (Memory(Id::from("closure0")), cycles.clone()),
                    (Memory(Id::from("closure1")), cycles.clone()),
                    (Memory(Id::from("closure2")), cycles.clone()),
                    (Memory(Id::from("closure3")), cycles.clone()),
                ])
            }
        };
        "overlapping cycles"
    )]
    fn test_detect_cycles(statements: Vec<Statement>, expected_cycles: ClosureCycles) {
        let mut referrer = WeakReferrer::new();
        let cycles = referrer.detect_closure_cycles(&statements);
        assert_eq!(cycles, expected_cycles)
    }
}
