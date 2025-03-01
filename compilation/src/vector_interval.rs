use gcollections::ops::*;
use interval::ops::*;
use interval::Interval;
use std::ops::Add;

macro_rules! define_vector_interval{
    ($name:ident $(, $fields:ident )*) => {
        #[derive(PartialEq, Clone, Debug)]
        struct $name {
            $($fields: Interval<usize>,)*
        }
        impl $name {
            pub fn new() -> Self {
                $name {
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
                $name {
                    $($fields: self.$fields.add(other.$fields),)*
                }
            }

            pub fn hull(&self, other: Self) -> Self {
                $name {
                    $($fields: self.$fields.hull(&other.$fields),)*
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
        assert_eq!(TestClass::new(), TestClass::new());
    }

    #[test]
    fn attribute_test() {
        define_vector_interval!(TestClass, field);
        assert_eq!(TestClass::new(), TestClass::new());
        assert_ne!(TestClass::field(), TestClass::new());
        assert_eq!(TestClass::field(), TestClass::field());
    }

    #[test]
    fn multiple_attributes_test() {
        define_vector_interval!(TestClass, field1, field2, field3);
        assert_eq!(TestClass::new(), TestClass::new());
        assert_eq!(TestClass::field1(), TestClass::field1());
        assert_eq!(TestClass::field2(), TestClass::field2());
        assert_eq!(TestClass::field3(), TestClass::field3());

        assert_ne!(TestClass::new(), TestClass::field1());
        assert_ne!(TestClass::field1(), TestClass::field2());
        assert_ne!(TestClass::field2(), TestClass::field3());
        assert_ne!(TestClass::field3(), TestClass::new());
        assert_ne!(TestClass::field1(), TestClass::field3());
        assert_ne!(TestClass::field2(), TestClass::new());
    }

    #[test]
    fn test_add() {
        define_vector_interval!(TestClass, field1, field2, field3);
        let a = TestClass {
            field1: Interval::singleton(1),
            field2: Interval::singleton(2),
            field3: Interval::singleton(3),
        };
        let b = TestClass {
            field1: Interval::singleton(4),
            field2: Interval::singleton(5),
            field3: Interval::singleton(6),
        };
        let c = TestClass {
            field1: Interval::singleton(5),
            field2: Interval::singleton(7),
            field3: Interval::singleton(9),
        };
        assert_eq!(a.add(b), c)
    }

    #[test]
    fn test_hull() {
        define_vector_interval!(TestClass, field1, field2, field3);
        let a = TestClass {
            field1: Interval::new(1, 2),
            field2: Interval::new(8, 15),
            field3: Interval::new(5, 13),
        };
        let b = TestClass {
            field1: Interval::new(5, 6),
            field2: Interval::new(5, 10),
            field3: Interval::new(10, 11),
        };
        let c = TestClass {
            field1: Interval::new(1, 6),
            field2: Interval::new(5, 15),
            field3: Interval::new(5, 13),
        };
        assert_eq!(a.hull(b), c)
    }
}
