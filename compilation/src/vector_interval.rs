use lowering::Id;
use std::collections::HashMap;
use std::ops::{Add, Mul};

#[macro_export]
macro_rules! define_vector_interval{
    ($name:ident $(, $fields:ident )*) => {
        #[derive(PartialEq, Clone, Debug)]
        struct $name {
            $($fields: usize,)*
            operators: HashMap<Id, usize>
        }
        impl $name {
            pub fn new() -> Self {
                Self {
                    $($fields: 0,)*
                    operators: HashMap::new()
                }
            }
            pub fn operator(operator: Id) -> Self {
                let mut instance = Self::new();
                instance.operators.insert(operator, 1);
                instance
            }
            $(
                pub fn $fields() -> Self {
                    let mut instance = Self::new();
                    instance.$fields = instance.$fields + 1;
                    instance
                }
            )*
        }

        impl Add<$name> for $name {
            type Output = Self;
            fn add(self, other: Self) -> Self {
                Self {
                    $($fields: self.$fields.add(other.$fields),)*
                    operators: HashMap::from_iter(
                        self.operators.keys().chain(other.operators.keys()).map(
                            |key| (key.clone(), self.operators.get(key).cloned().unwrap_or(0) + other.operators.get(key).cloned().unwrap_or(0))
                        )
                    )
                }
            }
        }

        impl Mul<usize> for $name {
            type Output = Self;
            fn mul(self, other: usize) -> Self {
                Self {
                    $($fields: self.$fields * other,)*
                    operators: HashMap::from_iter(
                        self.operators.into_iter().map(
                            |(key, value)| (key, value * other)
                        )
                    )
                }
            }
        }

        impl Mul<$name> for $name {
            type Output = usize;
            fn mul(self, other: Self) -> Self::Output {
                $(self.$fields * other.$fields +)*
                self.operators.keys().map(
                    |key| self.operators[key].clone() * other.operators.get(key).cloned().unwrap_or(0)
                ).sum::<usize>()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_constant_test() {
        define_vector_interval!(TestClass);
        assert_eq!(TestClass::new(), TestClass::new());
    }

    #[test]
    fn constant_attribute_test() {
        define_vector_interval!(TestClass, field);
        assert_eq!(TestClass::new(), TestClass::new());
        assert_ne!(TestClass::field(), TestClass::new());
        assert_eq!(TestClass::field(), TestClass::field());
    }

    #[test]
    fn multiple_constant_attributes_test() {
        define_vector_interval!(TestClass, field1, field2);
        assert_eq!(TestClass::new(), TestClass::new());
        assert_eq!(TestClass::field1(), TestClass::field1());
        assert_eq!(TestClass::field2(), TestClass::field2());
        assert_eq!(
            TestClass::operator(Id::from("-")),
            TestClass::operator(Id::from("-"))
        );
        assert_eq!(
            TestClass::operator(Id::from("<")),
            TestClass::operator(Id::from("<"))
        );

        assert_ne!(TestClass::new(), TestClass::field1());
        assert_ne!(TestClass::field1(), TestClass::field2());
        assert_ne!(
            TestClass::operator(Id::from("-")),
            TestClass::operator(Id::from("<"))
        );
        assert_ne!(TestClass::operator(Id::from("-")), TestClass::new());
        assert_ne!(TestClass::field1(), TestClass::operator(Id::from("<")));
        assert_ne!(TestClass::field2(), TestClass::new());
    }

    #[test]
    fn test_constant_add() {
        define_vector_interval!(TestClass, field1, field2, field3);
        let a = TestClass {
            field1: 1,
            field2: 2,
            field3: 3,
            operators: HashMap::from([
                (Id::from("+"), 8),
                (Id::from("*"), 12),
                (Id::from("--"), 2),
            ]),
        };
        let b = TestClass {
            field1: 4,
            field2: 5,
            field3: 6,
            operators: HashMap::from([(Id::from("+"), 6), (Id::from("<=>"), 1)]),
        };
        let c = TestClass {
            field1: 5,
            field2: 7,
            field3: 9,
            operators: HashMap::from([
                (Id::from("+"), 14),
                (Id::from("*"), 12),
                (Id::from("--"), 2),
                (Id::from("<=>"), 1),
            ]),
        };
        assert_eq!(a.add(b), c)
    }

    #[test]
    fn test_multiplication() {
        define_vector_interval!(TestClass, field1, field2);
        let a = TestClass {
            field1: 8,
            field2: 6,
            operators: HashMap::from([(Id::from("<=>"), 3), (Id::from("--"), 2)]),
        };
        let b = 2;
        let c = TestClass {
            field1: 16,
            field2: 12,
            operators: HashMap::from([(Id::from("<=>"), 6), (Id::from("--"), 4)]),
        };
        assert_eq!(a.mul(b), c)
    }

    #[test]
    fn test_dot_product() {
        define_vector_interval!(TestClass, field1, field2);
        let a = TestClass {
            field1: 8,
            field2: 6,
            operators: HashMap::from([(Id::from("<=>"), 3), (Id::from("--"), 2)]),
        };
        let b = TestClass {
            field1: 3,
            field2: 5,
            operators: HashMap::from([
                (Id::from("+"), 14),
                (Id::from("*"), 12),
                (Id::from("--"), 2),
                (Id::from("<=>"), 1),
            ]),
        };
        let c = 61;
        assert_eq!(a.mul(b), c)
    }
}
