use gcollections::ops::*;
use interval::ops::*;
use interval::Interval;
use std::ops::Add;

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

                pub fn add(&self, other: Self) -> Self {
                    Self {
                        $($fields: self.$fields.add(other.$fields),)*
                    }
                }

                pub fn hull(&self, other: Self) -> Self {
                    Self {
                        $($fields: self.$fields.hull(&other.$fields),)*
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
    fn new_test() {
        define_vector_interval!(TestClass);
        assert_eq!(TestClassInterval::new(), TestClassInterval::new());
    }

    #[test]
    fn attribute_test() {
        define_vector_interval!(TestClass, field);
        assert_eq!(TestClassInterval::new(), TestClassInterval::new());
        assert_ne!(TestClassInterval::field(), TestClassInterval::new());
        assert_eq!(TestClassInterval::field(), TestClassInterval::field());
    }

    #[test]
    fn multiple_attributes_test() {
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
    fn test_add() {
        define_vector_interval!(TestClass, field1, field2, field3);
        let a = TestClassInterval {
            field1: Interval::singleton(1),
            field2: Interval::singleton(2),
            field3: Interval::singleton(3),
        };
        let b = TestClassInterval {
            field1: Interval::singleton(4),
            field2: Interval::singleton(5),
            field3: Interval::singleton(6),
        };
        let c = TestClassInterval {
            field1: Interval::singleton(5),
            field2: Interval::singleton(7),
            field3: Interval::singleton(9),
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
}
