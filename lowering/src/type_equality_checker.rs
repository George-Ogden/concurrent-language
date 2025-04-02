use std::collections::HashMap;

use itertools::Itertools;

use crate::{IntermediateFnType, IntermediateTupleType, IntermediateType, IntermediateUnionType};

/// Check whether two types are equivalent, keeping track of references equal references.
pub struct TypeEqualityChecker {
    equal_references: HashMap<*mut IntermediateType, *mut IntermediateType>,
}

impl TypeEqualityChecker {
    pub fn new() -> Self {
        TypeEqualityChecker {
            equal_references: HashMap::new(),
        }
    }
    pub fn equal_type(&mut self, t1: &IntermediateType, t2: &IntermediateType) -> bool {
        match (t1, t2) {
            (IntermediateType::AtomicType(a1), IntermediateType::AtomicType(a2)) => a1 == a2,
            (
                IntermediateType::IntermediateTupleType(IntermediateTupleType(t1)),
                IntermediateType::IntermediateTupleType(IntermediateTupleType(t2)),
            ) => self.equal_types(t1, t2),
            (
                IntermediateType::IntermediateFnType(IntermediateFnType(a1, r1)),
                IntermediateType::IntermediateFnType(IntermediateFnType(a2, r2)),
            ) => self.equal_types(a1, a2) && self.equal_type(r1, r2),
            (
                IntermediateType::IntermediateUnionType(IntermediateUnionType(t1)),
                IntermediateType::IntermediateUnionType(IntermediateUnionType(t2)),
            ) => {
                t1.len() == t2.len()
                    && t1.iter().zip_eq(t2.iter()).all(|(t1, t2)| match (t1, t2) {
                        (None, None) => true,
                        (Some(t1), Some(t2)) => self.equal_type(t1, t2),
                        _ => false,
                    })
            }
            (IntermediateType::Reference(r1), IntermediateType::Reference(r2)) => {
                let p1 = r1.as_ptr();
                let p2 = r2.as_ptr();
                if self.equal_references.get(&p1) == Some(&p2) {
                    // Equal references
                    true
                } else if matches!(self.equal_references.get(&p1), Some(_))
                    || matches!(self.equal_references.get(&p2), Some(_))
                {
                    // Unequal references.
                    false
                } else {
                    // Check whether references are equal (assume they are).
                    self.equal_references.insert(p1, p2);
                    self.equal_references.insert(p2, p1);
                    self.equal_type(&r1.borrow().clone(), &r2.borrow().clone())
                }
            }
            _ => false,
        }
    }
    fn equal_types(&mut self, t1: &Vec<IntermediateType>, t2: &Vec<IntermediateType>) -> bool {
        t1.len() == t2.len()
            && t1
                .iter()
                .zip_eq(t2.iter())
                .all(|(t1, t2)| self.equal_type(t1, t2))
    }
}
