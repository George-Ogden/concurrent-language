use itertools::Itertools;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    iter,
    rc::Rc,
};

use crate::{
    Allocation, Assignment, ClosureInstantiation, Declaration, Expression, FnDef, Id, IfStatement,
    MachineType, MatchBranch, MatchStatement, Memory, Name, Program, Statement, TupleExpression,
    Value,
};

type Node = Memory;
type Cycles = HashMap<Node, Rc<RefCell<Vec<Node>>>>;
type Graph = HashMap<Node, Vec<Node>>;
type Translation = HashMap<Memory, (Memory, Name)>;

#[derive(Debug, Clone, PartialEq)]
/// Structure for storing information about cyclic closures.
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
    fn update(&mut self, cycles: ClosureCycles) {
        self.fn_translation
            .extend(cycles.fn_translation.into_iter());
        self.cycles.extend(cycles.cycles.into_iter());
    }
}

pub struct Weakener {}

impl Weakener {
    /// Add weak fn types and allocators to a program with recursive closures.
    pub fn weaken(program: Program) -> Program {
        let Program { type_defs, fn_defs } = program;
        // Find all cyclic closures in the program.
        let mut closure_cycles = ClosureCycles::new();
        for fn_def in &fn_defs {
            closure_cycles.update(Self::detect_closure_cycles(&fn_def.statements));
        }
        // Add allocators.
        let (fn_defs, weak_fns): (Vec<_>, Vec<_>) = fn_defs
            .into_iter()
            .map(
                |FnDef {
                     name,
                     arguments,
                     statements,
                     ret,
                     env,
                     size_bounds,
                     is_recursive,
                 }| {
                    let (statements, weak_fns) = Self::add_allocations(statements, &closure_cycles);
                    (
                        FnDef {
                            name,
                            arguments,
                            statements,
                            ret,
                            env,
                            size_bounds,
                            is_recursive,
                        },
                        weak_fns,
                    )
                },
            )
            .collect();
        // Replace fns with weak fns.
        let weak_fns = weak_fns.into_iter().flatten().collect();
        let fn_defs = fn_defs
            .into_iter()
            .map(|fn_def| Self::weaken_fn_def(fn_def, &weak_fns))
            .collect();
        Program { type_defs, fn_defs }
    }
    /// Construct graph of all references that may be cyclic.
    fn construct_graph(statements: &Vec<Statement>) -> (Graph, HashSet<Memory>, Translation) {
        let mut graph = Graph::new();
        let mut fns = HashSet::new();
        let mut translation = Translation::new();
        for statement in statements {
            match statement {
                Statement::Allocation(_)
                | Statement::Await(_)
                | Statement::Declaration(_)
                | Statement::Enqueue(_) => {}
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
                                        translation
                                            .insert(memory.clone(), (target.clone(), name.clone()));
                                    }
                                    fns.insert(target.clone());
                                    vec![value]
                                }
                                None => Vec::new(),
                            }
                        }
                        // All other expressions may contain references but they won't form cycles of fn instances.
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
                        let graph_fns_translation = Self::construct_graph(statements);
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
                        let graph_fns_translation = Self::construct_graph(&branch.statements);
                        graph.extend(graph_fns_translation.0);
                        fns.extend(graph_fns_translation.1);
                        translation.extend(graph_fns_translation.2);
                    }
                }
            }
        }
        (graph, fns, translation)
    }
    /// Transpose the adjacency matrix of the reference graph.
    fn transpose(graph: &Graph) -> Graph {
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
    fn detect_closure_cycles(statements: &Vec<Statement>) -> ClosureCycles {
        let mut cycles = ClosureCycles::new();
        let graph;
        let fns;
        (graph, fns, cycles.fn_translation) = Self::construct_graph(statements);
        let mut visited = HashSet::new();
        let mut order = Vec::new();
        // Topologically sort nodes.
        for node in graph.keys().cloned().collect_vec() {
            if !visited.contains(&node) {
                Self::topsort(&graph, &node, &mut visited, &mut order);
            }
        }

        order.reverse();
        let graph = Self::transpose(&graph);
        visited = HashSet::new();

        for node in order {
            if !visited.contains(&node) {
                // Any nodes that are reachable in the reverse of the topological sort must form cycles.
                let mut nodes = Vec::new();
                Self::topsort(&graph, &node, &mut visited, &mut nodes);
                if nodes.len() > 1
                    || graph
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
    /// Topologically sort nodes, storing the result into `order`.
    fn topsort(graph: &Graph, node: &Node, visited: &mut HashSet<Node>, order: &mut Vec<Node>) {
        visited.insert(node.clone());
        for neighbor in graph.get(&node).cloned().unwrap_or_default() {
            if !visited.contains(&neighbor) {
                Self::topsort(graph, &neighbor, visited, order);
            }
        }
        order.push(node.clone());
    }

    /// Insert allocators for cyclic references into the statements, returning any fns that need weak ptrs.
    fn add_allocations(
        statements: Vec<Statement>,
        closure_cycles: &ClosureCycles,
    ) -> (Vec<Statement>, HashSet<(Name, usize)>) {
        let ClosureCycles {
            fn_translation,
            cycles,
        } = &closure_cycles;
        let mut cyclic_closures: HashSet<_> = cycles.keys().cloned().collect();
        let mut weak_fns = HashSet::new();
        let name_translation: HashMap<Memory, Name> = fn_translation.values().cloned().collect();
        let statements = statements
            .into_iter()
            .flat_map(|statement| match statement {
                Statement::Await(await_) => vec![await_.into()],
                Statement::Allocation(allocation) => vec![allocation.into()],
                Statement::Enqueue(enqueue) => vec![enqueue.into()],
                Statement::Assignment(assignment) => {
                    if let Assignment {
                        target,
                        value: Expression::TupleExpression(TupleExpression(values)),
                    } = &assignment
                    {
                        if let Some((closure, fn_name)) = fn_translation.get(target) {
                            if let Some(vars) = cycles.get(closure) {
                                let closed_values: HashSet<_> =
                                    HashSet::from_iter(vars.borrow().clone());
                                for (i, value) in values.iter().enumerate() {
                                    if let Value::Memory(memory) = value {
                                        if closed_values.contains(memory) {
                                            weak_fns.insert((fn_name.clone(), i));
                                        }
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
                            let fns = cycle
                                .iter()
                                .map(|memory| (memory.clone(), name_translation[&memory].clone()))
                                .collect_vec();
                            // Include all names in the allocator to ensure uniqueness.
                            let name = iter::once(Name::from("Allocator"))
                                .chain(cycle.into_iter().map(|Memory(id)| id))
                                .join("_");
                            vec![Allocation {
                                target: Memory(Id::from(format!("{name}_"))),
                                name,
                                fns,
                            }
                            .into()]
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
                        .map(|branch| Self::add_allocations(branch, &closure_cycles))
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
                                Self::add_allocations(statements, &closure_cycles);
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

    /// Update the signature of the function's closure to contain a weak ptr to the fn.
    fn weaken_fn_def(fn_def: FnDef, weak_fns: &HashSet<(Name, usize)>) -> FnDef {
        let FnDef {
            name,
            arguments,
            statements,
            ret,
            env,
            size_bounds,
            is_recursive,
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
            size_bounds,
            is_recursive,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Allocation, Assignment, Await, BuiltIn, ClosureInstantiation, Declaration, FnDef, FnType,
        Id, IfStatement, MachineType, MatchBranch, MatchStatement, Memory, Name, Program,
        Statement, TupleExpression, TupleType, TypeDef, UnionType,
    };

    use super::*;
    use lowering::{AtomicTypeEnum, Boolean, Integer};
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
                (Memory(Id::from("env")), (Memory(Id::from("closure")), Name::from("f"))),
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
                (Memory(Id::from("env")), (Memory(Id::from("closure")), Name::from("f")))
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
                (Memory(Id::from("env0")), (Memory(Id::from("closure0")), Name::from("f0"))),
                (Memory(Id::from("env1")), (Memory(Id::from("closure1")), Name::from("f1"))),
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
                (Memory(Id::from("env0")), (Memory(Id::from("closure0")), Name::from("f0"))),
                (Memory(Id::from("env1")), (Memory(Id::from("closure1")), Name::from("f1"))),
                (Memory(Id::from("env2")), (Memory(Id::from("closure2")), Name::from("f2"))),
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
                (Memory(Id::from("env0")), (Memory(Id::from("closure0")), Name::from("f0"))),
                (Memory(Id::from("env1")), (Memory(Id::from("closure1")), Name::from("f1"))),
                (Memory(Id::from("env2")), (Memory(Id::from("closure2")), Name::from("f2"))),
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
                (Memory(Id::from("env0")), (Memory(Id::from("closure0")), Name::from("f0"))),
                (Memory(Id::from("env1")), (Memory(Id::from("closure1")), Name::from("f1"))),
                (Memory(Id::from("env2")), (Memory(Id::from("closure2")), Name::from("f2"))),
                (Memory(Id::from("env3")), (Memory(Id::from("closure3")), Name::from("f3"))),
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
                (Memory(Id::from("env0")), (Memory(Id::from("closure0")), Name::from("f0"))),
                (Memory(Id::from("env1")), (Memory(Id::from("closure1")), Name::from("f1"))),
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
                (Memory(Id::from("env0")), (Memory(Id::from("closure0")), Name::from("f0"))),
                (Memory(Id::from("env1")), (Memory(Id::from("closure1")), Name::from("f1"))),
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
                (Memory(Id::from("env0")), (Memory(Id::from("closure0")), Name::from("f0"))),
                (Memory(Id::from("env1")), (Memory(Id::from("closure1")), Name::from("f1"))),
                (Memory(Id::from("env2")), (Memory(Id::from("closure2")), Name::from("f2"))),
                (Memory(Id::from("env3")), (Memory(Id::from("closure3")), Name::from("f3"))),
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
        let cycles = Weakener::detect_closure_cycles(&statements);
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
                        Allocation{
                            fns: vec![
                                (Memory(Id::from("closure0")),Name::from("f0")),
                                (Memory(Id::from("closure1")),Name::from("f1")),
                            ],
                            name: Name::from("Allocator_closure0_closure1"),
                            target: Memory(Id::from("Allocator_closure0_closure1_"))
                        }.into(),
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
                            Allocation{
                                fns: vec![
                                    (Memory(Id::from("closure0")),Name::from("f0")),
                                    (Memory(Id::from("closure1")),Name::from("f1")),
                                ],
                                target: Memory(Id::from("Allocator_closure0_closure1_")),
                                name: Name::from("Allocator_closure0_closure1")
                            }.into(),
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
            Allocation{
                fns: vec![
                    (Memory(Id::from("closure0")),Name::from("f0")),
                    (Memory(Id::from("closure1")),Name::from("f1")),
                ],
                name: Name::from("Allocator_closure0_closure1"),
                target: Memory(Id::from("Allocator_closure0_closure1_"))
            }.into(),
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
            Allocation{
                fns: vec![
                    (Memory(Id::from("closure2")),Name::from("f2")),
                    (Memory(Id::from("closure3")),Name::from("f3")),
                ],
                name: Name::from("Allocator_closure2_closure3"),
                target: Memory(Id::from("Allocator_closure2_closure3_"))
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
            Allocation{
                fns: vec![
                    (Memory(Id::from("closure0")),Name::from("f0")),
                    (Memory(Id::from("closure1")),Name::from("f1")),
                    (Memory(Id::from("closure2")),Name::from("f2")),
                    (Memory(Id::from("closure3")),Name::from("f3")),
                ],
                name: Name::from("Allocator_closure0_closure1_closure2_closure3"),
                target: Memory(Id::from("Allocator_closure0_closure1_closure2_closure3_"))
            }.into(),
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
        HashSet::from([
            (Name::from("f1"), 0),
        ]);
        "extra self cycle"
    )]
    fn test_add_allocations(
        statements: Vec<Statement>,
        expected_statements: Vec<Statement>,
        expected_weak_fns: HashSet<(Name, usize)>,
    ) {
        let cycles = Weakener::detect_closure_cycles(&statements);
        let (statements, weak_fns) = Weakener::add_allocations(statements, &cycles);
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
            env: vec![
                AtomicTypeEnum::BOOL.into(),
                FnType(
                    vec![AtomicTypeEnum::INT.into()],
                    Box::new(AtomicTypeEnum::INT.into()),
                ).into()
            ],
            size_bounds: (10, 100),
            is_recursive: false
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
            ],
            size_bounds: (10, 100),
            is_recursive: false
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
        let weak_fn_def = Weakener::weaken_fn_def(fn_def.clone(), &weak_fns);
        assert_eq!(fn_def.name, weak_fn_def.name);
        assert_eq!(fn_def.arguments, weak_fn_def.arguments);
        assert_eq!(fn_def.statements, weak_fn_def.statements);
        assert_eq!(fn_def.ret, weak_fn_def.ret);
        assert_eq!(expected_env, weak_fn_def.env);
    }

    #[test_case(
        Program {
            fn_defs: vec![
                FnDef {
                    name: Name::from("Main"),
                    arguments: Vec::new(),
                    ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
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
                    size_bounds: (10, 100),
                    is_recursive: false
                },
                FnDef {
                    name: Name::from("f0"),
                    arguments: vec![(Memory(Id::from("a0")), AtomicTypeEnum::INT.into())],
                    ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                    env: vec![
                        FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into()),
                        ).into(),
                    ],
                    statements: Vec::new(),
                    size_bounds: (10, 100),
                    is_recursive: false
                },
                FnDef {
                    name: Name::from("f1"),
                    arguments: vec![(Memory(Id::from("a1")), AtomicTypeEnum::INT.into())],
                    ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                    env: vec![
                        FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into()),
                        ).into(),
                        FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into()),
                        ).into(),
                    ],
                    statements: Vec::new(),
                    size_bounds: (10, 100),
                    is_recursive: false
                },
                FnDef {
                    name: Name::from("f2"),
                    arguments: vec![(Memory(Id::from("a2")), AtomicTypeEnum::INT.into())],
                    ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                    env: vec![
                        FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into()),
                        ).into(),
                    ],
                    statements: Vec::new(),
                    size_bounds: (10, 100),
                    is_recursive: false
                },
                FnDef {
                    name: Name::from("f3"),
                    arguments: vec![(Memory(Id::from("a3")), AtomicTypeEnum::INT.into())],
                    ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                    env: vec![
                        FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into()),
                        ).into(),
                    ],
                    statements: Vec::new(),
                    size_bounds: (10, 100),
                    is_recursive: false
                },
            ],
            type_defs: vec![
                TypeDef{
                    name: Name::from("T0"),
                    constructors: vec![
                        (Name::from("T0C0"), None)
                    ]
                }
            ]
        },
        vec![
            FnDef {
                name: Name::from("Main"),
                arguments: Vec::new(),
                ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                env: Vec::new(),
                statements: vec![
                    Allocation{
                        fns: vec![
                            (Memory(Id::from("closure0")),Name::from("f0")),
                            (Memory(Id::from("closure1")),Name::from("f1")),
                            (Memory(Id::from("closure2")),Name::from("f2")),
                            (Memory(Id::from("closure3")),Name::from("f3")),
                        ],
                        name: Name::from("Allocator_closure0_closure1_closure2_closure3"),
                        target: Memory(Id::from("Allocator_closure0_closure1_closure2_closure3_"))
                    }.into(),
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
                size_bounds: (10, 100),
                is_recursive: false
            },
            FnDef {
                name: Name::from("f0"),
                arguments: vec![(Memory(Id::from("a0")), AtomicTypeEnum::INT.into())],
                ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                env: vec![
                    MachineType::WeakFnType(FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    )),
                ],
                statements: Vec::new(),
                size_bounds: (10, 100),
                is_recursive: false
            },
            FnDef {
                name: Name::from("f1"),
                arguments: vec![(Memory(Id::from("a1")), AtomicTypeEnum::INT.into())],
                ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                env: vec![
                    MachineType::WeakFnType(FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    )),
                    MachineType::WeakFnType(FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    )),
                ],
                statements: Vec::new(),
                size_bounds: (10, 100),
                is_recursive: false
            },
            FnDef {
                name: Name::from("f2"),
                arguments: vec![(Memory(Id::from("a2")), AtomicTypeEnum::INT.into())],
                ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                env: vec![
                    MachineType::WeakFnType(FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    )),
                ],
                statements: Vec::new(),
                size_bounds: (10, 100),
                is_recursive: false
            },
            FnDef {
                name: Name::from("f3"),
                arguments: vec![(Memory(Id::from("a3")), AtomicTypeEnum::INT.into())],
                ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                env: vec![
                    MachineType::WeakFnType(FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    )),
                ],
                statements: Vec::new(),
                size_bounds: (10, 100),
                is_recursive: false
            },
        ];
        "overlapping cycles"
    )]
    #[test_case(
        Program {
            fn_defs: vec![
                FnDef {
                    name: Name::from("Main"),
                    arguments: Vec::new(),
                    ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                    env: Vec::new(),
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
                    size_bounds: (10, 100),
                    is_recursive: false
                },
                FnDef {
                    name: Name::from("f0"),
                    arguments: vec![(Memory(Id::from("a0")), AtomicTypeEnum::INT.into())],
                    ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                    env: vec![
                        FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into()),
                        ).into(),
                    ],
                    statements: Vec::new(),
                    size_bounds: (10, 100),
                    is_recursive: false
                },
                FnDef {
                    name: Name::from("f1"),
                    arguments: vec![(Memory(Id::from("a1")), AtomicTypeEnum::INT.into())],
                    ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                    env: vec![
                        FnType(
                            vec![AtomicTypeEnum::INT.into()],
                            Box::new(AtomicTypeEnum::INT.into()),
                        ).into(),
                    ],
                    statements: Vec::new(),
                    size_bounds: (10, 100),
                    is_recursive: false
                },
            ],
            type_defs: vec![
                TypeDef{
                    name: Name::from("T0"),
                    constructors: vec![
                        (Name::from("T0C0"), None)
                    ]
                }
            ]
        },
        vec![
            FnDef {
                name: Name::from("Main"),
                arguments: Vec::new(),
                ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                env: Vec::new(),
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
                size_bounds: (10, 100),
                is_recursive: false
            },
            FnDef {
                name: Name::from("f0"),
                arguments: vec![(Memory(Id::from("a0")), AtomicTypeEnum::INT.into())],
                ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                env: vec![
                    FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    ).into(),
                ],
                statements: Vec::new(),
                size_bounds: (10, 100),
                is_recursive: false
            },
            FnDef {
                name: Name::from("f1"),
                arguments: vec![(Memory(Id::from("a1")), AtomicTypeEnum::INT.into())],
                ret: (BuiltIn::from(Integer{value: 0}).into(), AtomicTypeEnum::INT.into()),
                env: vec![
                    MachineType::WeakFnType(FnType(
                        vec![AtomicTypeEnum::INT.into()],
                        Box::new(AtomicTypeEnum::INT.into()),
                    )),
                ],
                statements: Vec::new(),
                size_bounds: (10, 100),
                is_recursive: false
            },
        ];
        "extra self cycle"
    )]
    fn test_weaken_program(program: Program, expected_fn_defs: Vec<FnDef>) {
        let weak_program = Weakener::weaken(program.clone());
        assert_eq!(weak_program.fn_defs, expected_fn_defs);
        assert_eq!(weak_program.type_defs, program.type_defs);
    }
}
