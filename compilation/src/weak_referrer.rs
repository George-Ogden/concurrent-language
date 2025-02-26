use itertools::Itertools;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::{
    Allocation, Assignment, ClosureInstantiation, Declaration, Expression, FnDef, IfStatement,
    MachineType, MatchBranch, MatchStatement, Memory, Name, Statement, TupleExpression, Value,
};

type Node = Memory;
type Cycles = HashMap<Node, Rc<RefCell<Vec<Node>>>>;
type Graph = HashMap<Node, Vec<Node>>;
type Translation = HashMap<Memory, Name>;

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
    fn construct_graph(
        &self,
        statements: &Vec<Statement>,
    ) -> (Graph, HashSet<Memory>, Translation) {
        let mut graph = Graph::new();
        let mut fns = HashSet::new();
        let mut translation = Translation::new();
        for statement in statements {
            match statement {
                Statement::Allocation(_) | Statement::Await(_) | Statement::Declaration(_) => {}
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
                            match env {
                                Some(value) => {
                                    if let Value::Memory(memory) = value {
                                        translation.insert(memory.clone(), name.clone());
                                    }
                                    fns.insert(target.clone());
                                    vec![value]
                                }
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
                Statement::IfStatement(IfStatement {
                    condition: _,
                    branches,
                }) => {
                    for statements in [&branches.0, &branches.1] {
                        let graph_fns_translation = self.construct_graph(statements);
                        graph.extend(graph_fns_translation.0);
                        fns.extend(graph_fns_translation.1);
                        translation.extend(graph_fns_translation.2);
                    }
                }
                Statement::MatchStatement(MatchStatement {
                    expression: _,
                    branches,
                    auxiliary_memory: _,
                }) => {
                    for branch in branches {
                        let graph_fns_translation = self.construct_graph(&branch.statements);
                        graph.extend(graph_fns_translation.0);
                        fns.extend(graph_fns_translation.1);
                        translation.extend(graph_fns_translation.2);
                    }
                }
            }
        }
        (graph, fns, translation)
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
        let fns;
        (self.graph, fns, cycles.fn_translation) = self.construct_graph(statements);
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
                    let nodes = nodes
                        .iter()
                        .filter(|&node| fns.contains(node))
                        .cloned()
                        .collect_vec();
                    let cycle = Rc::new(RefCell::new(
                        nodes.clone().into_iter().sorted().collect_vec(),
                    ));
                    for node in nodes {
                        cycles.cycles.insert(node.clone(), cycle.clone());
                    }
                }
            }
        }
        cycles
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

    fn add_allocations(
        &self,
        statements: Vec<Statement>,
        closure_cycles: &ClosureCycles,
    ) -> (Vec<Statement>, HashSet<(Name, usize)>) {
        let ClosureCycles {
            fn_translation,
            cycles,
        } = &closure_cycles;
        let mut cyclic_closures: HashSet<_> = cycles.keys().cloned().collect();
        let mut weak_fns = HashSet::new();
        let statements = statements
            .into_iter()
            .flat_map(|statement| match statement {
                Statement::Await(await_) => vec![await_.into()],
                Statement::Allocation(allocation) => vec![allocation.into()],
                Statement::Assignment(assignment) => {
                    if let Assignment {
                        target,
                        value: Expression::TupleExpression(TupleExpression(values)),
                    } = &assignment
                    {
                        if let Some(fn_name) = fn_translation.get(target) {
                            for (i, value) in values.iter().enumerate() {
                                if let Value::Memory(memory) = value {
                                    if cycles.contains_key(memory) {
                                        weak_fns.insert((fn_name.clone(), i));
                                    }
                                }
                            }
                        }
                    };
                    vec![assignment.into()]
                }
                Statement::Declaration(Declaration { type_, memory }) => {
                    let mut statements = if cyclic_closures.contains(&memory) {
                        let cycle = cycles[&memory].borrow().clone();
                        for memory in &cycle {
                            cyclic_closures.remove(&memory);
                        }
                        if cycle.len() > 1 {
                            vec![Allocation(cycle).into()]
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };
                    statements.push(Declaration { type_, memory }.into());
                    statements
                }
                Statement::IfStatement(IfStatement {
                    condition,
                    branches,
                }) => {
                    let (branches, extra_fns) = [branches.0, branches.1]
                        .into_iter()
                        .map(|branch| self.add_allocations(branch, &closure_cycles))
                        .collect::<(Vec<_>, Vec<_>)>();
                    for fns in extra_fns {
                        weak_fns.extend(fns.into_iter());
                    }
                    let branches: [Vec<Statement>; 2] = branches.try_into().unwrap();
                    vec![IfStatement {
                        condition,
                        branches: branches.into(),
                    }
                    .into()]
                }
                Statement::MatchStatement(MatchStatement {
                    expression,
                    branches,
                    auxiliary_memory,
                }) => {
                    let (branches, extra_fns) = branches
                        .into_iter()
                        .map(|MatchBranch { target, statements }| {
                            let (statements, weak_fns) =
                                self.add_allocations(statements, &closure_cycles);
                            (MatchBranch { target, statements }, weak_fns)
                        })
                        .collect::<(Vec<_>, Vec<_>)>();
                    for fns in extra_fns {
                        weak_fns.extend(fns.into_iter());
                    }
                    vec![MatchStatement {
                        branches,
                        expression,
                        auxiliary_memory,
                    }
                    .into()]
                }
            })
            .collect_vec();

        (statements, weak_fns)
    }

    fn weaken_fn_def(&self, fn_def: FnDef, weak_fns: &HashSet<(Name, usize)>) -> FnDef {
        let FnDef {
            name,
            arguments,
            statements,
            ret,
            env,
            allocations,
        } = fn_def;
        let env = env
            .into_iter()
            .enumerate()
            .map(|(i, type_)| {
                if let MachineType::FnType(fn_type) = type_ {
                    if weak_fns.contains(&(name.clone(), i)) {
                        MachineType::WeakFnType(fn_type)
                    } else {
                        fn_type.into()
                    }
                } else {
                    type_
                }
            })
            .collect();
        FnDef {
            name,
            arguments,
            statements,
            ret,
            env,
            allocations,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Allocation, Assignment, Await, BuiltIn, ClosureInstantiation, Declaration, FnDef, FnType,
        Id, IfStatement, MachineType, MatchBranch, MatchStatement, Memory, Name, Statement,
        TupleExpression, TupleType, UnionType,
    };

    use super::*;
    use lowering::{AtomicTypeEnum, Boolean};
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
            fn_translation: HashMap::new(),
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
                (Memory(Id::from("env")), Name::from("f")),
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
                (Memory(Id::from("env")), Name::from("f"))
            ]),
            cycles: HashMap::from([
                (Memory(Id::from("closure")), Rc::new(RefCell::new(vec![Memory(Id::from("closure"))])))
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
                (Memory(Id::from("env0")), Name::from("f0")),
                (Memory(Id::from("env1")), Name::from("f1")),
            ]),
            cycles: {
                let cycles = Rc::new(RefCell::new(vec![
                    Memory(Id::from("closure1")),
                ]));
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
                (Memory(Id::from("env0")), Name::from("f0")),
                (Memory(Id::from("env1")), Name::from("f1")),
                (Memory(Id::from("env2")), Name::from("f2")),
            ]),
            cycles: {
                let cycles = Rc::new(RefCell::new(vec![
                    Memory(Id::from("closure0")),
                    Memory(Id::from("closure1")),
                    Memory(Id::from("closure2")),
                ]));
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
                (Memory(Id::from("env0")), Name::from("f0")),
                (Memory(Id::from("env1")), Name::from("f1")),
                (Memory(Id::from("env2")), Name::from("f2")),
            ]),
            cycles: {
                let cycles = Rc::new(RefCell::new(vec![
                    Memory(Id::from("closure0")),
                    Memory(Id::from("closure1")),
                ]));
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
                (Memory(Id::from("env0")), Name::from("f0")),
                (Memory(Id::from("env1")), Name::from("f1")),
                (Memory(Id::from("env2")), Name::from("f2")),
                (Memory(Id::from("env3")), Name::from("f3")),
            ]),
            cycles: {
                let cycles = Rc::new(RefCell::new(vec![
                    Memory(Id::from("closure0")),
                    Memory(Id::from("closure1")),
                    Memory(Id::from("closure2")),
                    Memory(Id::from("closure3")),
                ]));
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
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("condition"))]).into(),
            IfStatement {
                condition: Memory(Id::from("condition")).into(),
                branches: (
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
                    ],
                    Vec::new()
                )
            }.into(),
        ],
        ClosureCycles{
            fn_translation: HashMap::from([
                (Memory(Id::from("env0")), Name::from("f0")),
                (Memory(Id::from("env1")), Name::from("f1")),
            ]),
            cycles: {
                let cycles = Rc::new(RefCell::new(vec![
                    Memory(Id::from("closure0")),
                    Memory(Id::from("closure1")),
                ]));
                HashMap::from([
                    (Memory(Id::from("closure0")), cycles.clone()),
                    (Memory(Id::from("closure1")), cycles.clone()),
                ])
            }
        };
        "if statement cycle"
    )]
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("subject"))]).into(),
            MatchStatement {
                expression: (Memory(Id::from("subject")).into(), UnionType(vec![Name::from("S0")])),
                auxiliary_memory: Memory(Id::from("extra")),
                branches: vec![
                    MatchBranch{
                        target: None,
                        statements: vec![
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
                        ]
                    }
                ]
            }.into(),
        ],
        ClosureCycles{
            fn_translation: HashMap::from([
                (Memory(Id::from("env0")), Name::from("f0")),
                (Memory(Id::from("env1")), Name::from("f1")),
            ]),
            cycles: {
                let cycles = Rc::new(RefCell::new(vec![
                    Memory(Id::from("closure0")),
                    Memory(Id::from("closure1")),
                ]));
                HashMap::from([
                    (Memory(Id::from("closure0")), cycles.clone()),
                    (Memory(Id::from("closure1")), cycles.clone()),
                ])
            }
        };
        "match statement cycle"
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
                memory: Memory(Id::from("env2")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env3")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env2")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure3")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env3")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure2")).into()]
                ).into()
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
                (Memory(Id::from("env0")), Name::from("f0")),
                (Memory(Id::from("env1")), Name::from("f1")),
                (Memory(Id::from("env2")), Name::from("f2")),
                (Memory(Id::from("env3")), Name::from("f3")),
            ]),
            cycles: {
                let cycle0 = Rc::new(RefCell::new(vec![
                    Memory(Id::from("closure0")),
                    Memory(Id::from("closure1")),
                ]));
                let cycle1 = Rc::new(RefCell::new(vec![
                    Memory(Id::from("closure2")),
                    Memory(Id::from("closure3")),
                ]));
                HashMap::from([
                    (Memory(Id::from("closure0")), cycle0.clone()),
                    (Memory(Id::from("closure1")), cycle0.clone()),
                    (Memory(Id::from("closure2")), cycle1.clone()),
                    (Memory(Id::from("closure3")), cycle1.clone()),
                ])
            }
        };
        "separate cycles"
    )]
    fn test_detect_cycles(statements: Vec<Statement>, expected_cycles: ClosureCycles) {
        let mut referrer = WeakReferrer::new();
        let cycles = referrer.detect_closure_cycles(&statements);
        assert_eq!(cycles, expected_cycles)
    }

    #[test_case(
        vec![
            Await(vec![Memory(Id::from("condition"))]).into(),
            IfStatement {
                condition: Memory(Id::from("condition")).into(),
                branches: (
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
                    ],
                    Vec::new()
                )
            }.into(),
        ],
        vec![
            Await(vec![Memory(Id::from("condition"))]).into(),
            IfStatement {
                condition: Memory(Id::from("condition")).into(),
                branches: (
                    vec![
                        Allocation(vec![
                            Memory(Id::from("closure0")),
                            Memory(Id::from("closure1")),
                        ]).into(),
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
                    ],
                    Vec::new()
                )
            }.into(),
        ],
        HashSet::from([
            (Name::from("f0"), 0),
            (Name::from("f1"), 0),
        ]);
        "if statement"
    )]
    #[test_case(
        vec![
            Await(vec![Memory(Id::from("subject"))]).into(),
            MatchStatement {
                expression: (Memory(Id::from("subject")).into(), UnionType(vec![Name::from("S0")])),
                auxiliary_memory: Memory(Id::from("extra")),
                branches: vec![
                    MatchBranch{
                        target: None,
                        statements: vec![
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
                                    AtomicTypeEnum::BOOL.into(),
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
                                    vec![
                                        BuiltIn::from(Boolean{value: true}).into(),
                                        Memory(Id::from("closure1")).into()
                                    ]
                                ).into()
                            }.into(),
                            Assignment{
                                target: Memory(Id::from("env1")),
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
                        ]
                    }
                ]
            }.into(),
        ],
        vec![
            Await(vec![Memory(Id::from("subject"))]).into(),
            MatchStatement {
                expression: (Memory(Id::from("subject")).into(), UnionType(vec![Name::from("S0")])),
                auxiliary_memory: Memory(Id::from("extra")),
                branches: vec![
                    MatchBranch{
                        target: None,
                        statements: vec![
                            Allocation(vec![
                                Memory(Id::from("closure0")),
                                Memory(Id::from("closure1")),
                            ]).into(),
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
                                    AtomicTypeEnum::BOOL.into(),
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
                                    vec![
                                        BuiltIn::from(Boolean{value: true}).into(),
                                        Memory(Id::from("closure1")).into()
                                    ]
                                ).into()
                            }.into(),
                            Assignment{
                                target: Memory(Id::from("env1")),
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
                        ]
                    }
                ]
            }.into(),
        ],
        HashSet::from([
            (Name::from("f0"), 1),
            (Name::from("f1"), 0),
        ]);
        "match statement"
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
        HashSet::from([
            (Name::from("f"), 1)
        ]);
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
                memory: Memory(Id::from("env2")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env3")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env2")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure3")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env3")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure2")).into()]
                ).into()
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
        vec![
            Allocation(vec![
                Memory(Id::from("closure0")),
                Memory(Id::from("closure1")),
            ]).into(),
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
            Allocation(vec![
                Memory(Id::from("closure2")),
                Memory(Id::from("closure3")),
            ]).into(),
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
                memory: Memory(Id::from("env2")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Declaration{
                memory: Memory(Id::from("env3")),
                type_: TupleType(vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into()
                ]).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env2")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure3")).into()]
                ).into()
            }.into(),
            Assignment{
                target: Memory(Id::from("env3")),
                value: TupleExpression(
                    vec![Memory(Id::from("closure2")).into()]
                ).into()
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
        HashSet::from([
            (Name::from("f0"), 0),
            (Name::from("f1"), 0),
            (Name::from("f2"), 0),
            (Name::from("f3"), 0),
        ]);
        "separate cycles"
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
        vec![
            Allocation(vec![
                Memory(Id::from("closure0")),
                Memory(Id::from("closure1")),
                Memory(Id::from("closure2")),
                Memory(Id::from("closure3")),
            ]).into(),
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
        HashSet::from([
            (Name::from("f0"), 0),
            (Name::from("f1"), 0),
            (Name::from("f1"), 1),
            (Name::from("f2"), 0),
            (Name::from("f3"), 0),
        ]);
        "overlapping cycles"
    )]
    fn test_add_allocations(
        statements: Vec<Statement>,
        expected_statements: Vec<Statement>,
        expected_weak_fns: HashSet<(Name, usize)>,
    ) {
        let mut referrer = WeakReferrer::new();
        let cycles = referrer.detect_closure_cycles(&statements);
        let (statements, weak_fns) = referrer.add_allocations(statements, &cycles);
        assert_eq!(statements, expected_statements);
        assert_eq!(weak_fns, expected_weak_fns);
    }

    #[test_case(
        FnDef {
            name: Name::from("f"),
            statements: vec![
                Assignment {
                    target: Memory(Id::from("x")),
                    value: Value::from(Memory(Id::from("arg"))).into()
                }.into()
            ],
            arguments: vec![
                (Memory(Id::from("arg")), AtomicTypeEnum::INT.into())
            ],
            ret: (Memory(Id::from("x")).into(), AtomicTypeEnum::INT.into()),
            allocations: vec![
                Declaration {
                    memory: Memory(Id::from("x")),
                    type_: AtomicTypeEnum::INT.into()
                }
            ],
            env: vec![
                AtomicTypeEnum::BOOL.into(),
                FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            ]
        },
        HashSet::from([
            (Name::from("g"), 0),
        ]),
        vec![
            AtomicTypeEnum::BOOL.into(),
            FnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into()),
            ).into()
        ];
        "no replacement"
    )]
    #[test_case(
        FnDef {
            name: Name::from("f"),
            statements: vec![
                Assignment {
                    target: Memory(Id::from("x")),
                    value: Value::from(Memory(Id::from("arg"))).into()
                }.into()
            ],
            arguments: vec![
                (Memory(Id::from("arg")), AtomicTypeEnum::INT.into())
            ],
            ret: (Memory(Id::from("x")).into(), AtomicTypeEnum::INT.into()),
            allocations: vec![
                Declaration {
                    memory: Memory(Id::from("x")),
                    type_: AtomicTypeEnum::INT.into()
                }
            ],
            env: vec![
                FnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::BOOL.into()),
                ).into(),
                FnType(
                    Vec::new(),
                    Box::new(AtomicTypeEnum::BOOL.into()),
                ).into(),
                FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            ]
        },
        HashSet::from([
            (Name::from("f"), 1),
            (Name::from("f"), 2),
        ]),
        vec![
            FnType(
                Vec::new(),
                Box::new(AtomicTypeEnum::BOOL.into()),
            ).into(),
            MachineType::WeakFnType(FnType(
                Vec::new(),
                Box::new(AtomicTypeEnum::BOOL.into()),
            )),
            MachineType::WeakFnType(FnType(
                vec![AtomicTypeEnum::INT.into()],
                Box::new(AtomicTypeEnum::INT.into()),
            ))
        ];
        "replacement"
    )]
    fn test_fn_weakening(
        fn_def: FnDef,
        weak_fns: HashSet<(Name, usize)>,
        expected_env: Vec<MachineType>,
    ) {
        let referrer = WeakReferrer::new();
        let weak_fn_def = referrer.weaken_fn_def(fn_def.clone(), &weak_fns);
        assert_eq!(fn_def.name, weak_fn_def.name);
        assert_eq!(fn_def.arguments, weak_fn_def.arguments);
        assert_eq!(fn_def.statements, weak_fn_def.statements);
        assert_eq!(fn_def.ret, weak_fn_def.ret);
        assert_eq!(fn_def.allocations, weak_fn_def.allocations);
        assert_eq!(expected_env, weak_fn_def.env);
    }
}
