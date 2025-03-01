use gcollections::ops::*;
use interval::ops::*;
use interval::Interval;
use std::ops::{Add, Mul};

macro_rules! define_vector_interval{
    ($name:ident $(, $fields:ident )*) => {
        paste::paste! {
            #[derive(PartialEq, Clone, Debug)]
            struct [<$name Interval>] {
                $($fields: Interval<usize>,)*
            }
            impl [<$name Interval>] {
                pub fn new() -> Self {
                    Self {
                        $($fields: Interval::singleton(0),)*
                    }
                }
                $(
                    pub fn $fields() -> Self {
                        let mut instance = Self::new();
                        instance.$fields = instance.$fields + 1;
                        instance
                    }
                )*

                pub fn hull(&self, other: Self) -> Self {
                    Self {
                        $($fields: self.$fields.hull(&other.$fields),)*
                    }
                }
            }

            impl Add<[<$name Interval>]> for [<$name Interval>] {
                type Output = Self;
                fn add(self, other: Self) -> Self {
                    Self {
                        $($fields: self.$fields.add(other.$fields),)*
                    }
                }
            }

            impl Add<[<$name Constant>]> for [<$name Interval>] {
                type Output = Self;
                fn add(self, other: [<$name Constant>]) -> Self {
                    Self {
                        $($fields: self.$fields.add(other.$fields),)*
                    }
                }
            }

            impl Mul<[<$name Constant>]> for [<$name Interval>] {
                type Output = Interval<usize>;
                fn mul(self, other: [<$name Constant>]) -> Self::Output {
                    Interval::singleton(0) $(+ self.$fields.mul(other.$fields))*
                }
            }

            impl From<[<$name Constant>]> for [<$name Interval>] {
                fn from(value: [<$name Constant>]) -> Self {
                    Self {
                        $($fields: Interval::singleton(value.$fields),)*
                    }
                }
            }

            #[derive(PartialEq, Clone, Debug)]
            struct [<$name Constant>] {
                $($fields: usize,)*
            }
            impl [<$name Constant>] {
                pub fn new() -> Self {
                    Self {
                        $($fields: 0,)*
                    }
                }
                $(
                    pub fn $fields() -> Self {
                        let mut instance = Self::new();
                        instance.$fields = instance.$fields + 1;
                        instance
                    }
                )*

                pub fn add(&self, other: Self) -> Self {
                    Self {
                        $($fields: self.$fields.add(other.$fields),)*
                    }
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_interval_test() {
        define_vector_interval!(TestClass);
        assert_eq!(TestClassInterval::new(), TestClassInterval::new());
    }

    fn new_constant_test() {
        define_vector_interval!(TestClass);
        assert_eq!(TestClassConstant::new(), TestClassConstant::new());
    }

    #[test]
    fn interval_attribute_test() {
        define_vector_interval!(TestClass, field);
        assert_eq!(TestClassInterval::new(), TestClassInterval::new());
        assert_ne!(TestClassInterval::field(), TestClassInterval::new());
        assert_eq!(TestClassInterval::field(), TestClassInterval::field());
    }

    #[test]
    fn constant_attribute_test() {
        define_vector_interval!(TestClass, field);
        assert_eq!(TestClassConstant::new(), TestClassConstant::new());
        assert_ne!(TestClassConstant::field(), TestClassConstant::new());
        assert_eq!(TestClassConstant::field(), TestClassConstant::field());
    }

    #[test]
    fn multiple_interval_attributes_test() {
        define_vector_interval!(TestClass, field1, field2, field3);
        assert_eq!(TestClassInterval::new(), TestClassInterval::new());
        assert_eq!(TestClassInterval::field1(), TestClassInterval::field1());
        assert_eq!(TestClassInterval::field2(), TestClassInterval::field2());
        assert_eq!(TestClassInterval::field3(), TestClassInterval::field3());

        assert_ne!(TestClassInterval::new(), TestClassInterval::field1());
        assert_ne!(TestClassInterval::field1(), TestClassInterval::field2());
        assert_ne!(TestClassInterval::field2(), TestClassInterval::field3());
        assert_ne!(TestClassInterval::field3(), TestClassInterval::new());
        assert_ne!(TestClassInterval::field1(), TestClassInterval::field3());
        assert_ne!(TestClassInterval::field2(), TestClassInterval::new());
    }

    #[test]
    fn multiple_constant_attributes_test() {
        define_vector_interval!(TestClass, field1, field2, field3);
        assert_eq!(TestClassConstant::new(), TestClassConstant::new());
        assert_eq!(TestClassConstant::field1(), TestClassConstant::field1());
        assert_eq!(TestClassConstant::field2(), TestClassConstant::field2());
        assert_eq!(TestClassConstant::field3(), TestClassConstant::field3());

        assert_ne!(TestClassConstant::new(), TestClassConstant::field1());
        assert_ne!(TestClassConstant::field1(), TestClassConstant::field2());
        assert_ne!(TestClassConstant::field2(), TestClassConstant::field3());
        assert_ne!(TestClassConstant::field3(), TestClassConstant::new());
        assert_ne!(TestClassConstant::field1(), TestClassConstant::field3());
        assert_ne!(TestClassConstant::field2(), TestClassConstant::new());
    }

    #[test]
    fn test_constant_add() {
        define_vector_interval!(TestClass, field1, field2, field3);
        let a = TestClassConstant {
            field1: 1,
            field2: 2,
            field3: 3,
        };
        let b = TestClassConstant {
            field1: 4,
            field2: 5,
            field3: 6,
        };
        let c = TestClassConstant {
            field1: 5,
            field2: 7,
            field3: 9,
        };
        assert_eq!(a.add(b), c)
    }

    #[test]
    fn test_interval_add() {
        define_vector_interval!(TestClass, field1, field2);
        let a = TestClassInterval {
            field1: Interval::new(1, 8),
            field2: Interval::new(2, 7),
        };
        let b = TestClassInterval {
            field1: Interval::new(2, 3),
            field2: Interval::new(5, 5),
        };
        let c = TestClassInterval {
            field1: Interval::new(3, 11),
            field2: Interval::new(7, 12),
        };
        assert_eq!(a.add(b), c)
    }

    #[test]
    fn test_mixed_add() {
        define_vector_interval!(TestClass, field1, field2);
        let a = TestClassInterval {
            field1: Interval::new(1, 8),
            field2: Interval::new(2, 7),
        };
        let b = TestClassConstant {
            field1: 3,
            field2: 5,
        };
        let c = TestClassInterval {
            field1: Interval::new(4, 11),
            field2: Interval::new(7, 12),
        };
        assert_eq!(a.add(b), c)
    }

    #[test]
    fn test_hull() {
        define_vector_interval!(TestClass, field1, field2, field3);
        let a = TestClassInterval {
            field1: Interval::new(1, 2),
            field2: Interval::new(8, 15),
            field3: Interval::new(5, 13),
        };
        let b = TestClassInterval {
            field1: Interval::new(5, 6),
            field2: Interval::new(5, 10),
            field3: Interval::new(10, 11),
        };
        let c = TestClassInterval {
            field1: Interval::new(1, 6),
            field2: Interval::new(5, 15),
            field3: Interval::new(5, 13),
        };
        assert_eq!(a.hull(b), c)
    }

    #[test]
    fn test_constant_conversion() {
        define_vector_interval!(TestClass, field1, field2, field3);
        let constant = TestClassConstant {
            field1: 1,
            field2: 2,
            field3: 3,
        };
        let interval = TestClassInterval {
            field1: Interval::singleton(1),
            field2: Interval::singleton(2),
            field3: Interval::singleton(3),
        };
        assert_eq!(interval, constant.into())
    }

    #[test]
    fn test_multiplication_conversion() {
        define_vector_interval!(TestClass, field1, field2);
        let a = TestClassInterval {
            field1: Interval::new(1, 8),
            field2: Interval::new(2, 7),
        };
        let b = TestClassConstant {
            field1: 3,
            field2: 5,
        };
        let c = Interval::new(13, 59);
        assert_eq!(a.mul(b), c)
    }
}
