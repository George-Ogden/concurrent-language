use itertools::Itertools;
use lowering::Id;
use std::collections::HashMap;
use std::fs;
use std::iter::Sum;
use std::ops::{Add, Mul};
use std::path::Path;

#[macro_export]
macro_rules! define_named_vector{
    (@count ) => {0usize};
    (@count $head:ident $($tail:ident)*) => {1usize + define_named_vector!(@count $($tail)*)};
    ($name:ident $(, $fields:ident )*,) => {
        define_named_vector!($name $(, $fields )*);
    };
    ($name:ident $(, $fields:ident )*) => {
        #[derive(PartialEq, Clone, Debug)]
        pub struct $name {
            $(pub $fields: usize,)*
            pub operators: HashMap<Id, usize>
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
            pub fn save(&self, filepath: &Path) -> std::io::Result<()> {
                if let Some(dir) = filepath.parent() {
                    fs::create_dir_all(dir)?;
                }
                fs::write(filepath, self.to_string())
            }

            pub fn to_string(&self) -> String {
                let (operator_names, operator_values): (Vec<_>, Vec<_>) = self.operators.clone().into_iter().sorted().unzip();
                let fields: [&str; define_named_vector!(@count $($fields)*)] = [$(stringify!($fields),)*];
                let field_names: Vec<String> = fields.into_iter().map(String::from).collect();
                let header = field_names.into_iter().chain(operator_names).collect::<Vec<_>>().join("\t");

                let contents = [$(self.$fields,)*].into_iter().chain(operator_values).map(|x| x.to_string()).join("\t");
                format!("{header}\n{contents}")

            }
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

        impl Sum for $name {
            fn sum<I>(iter: I) -> Self where I: Iterator<Item=Self>{
                iter.fold(Self::new(), Self::add)
            }

        }
    };
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rstest::{fixture, rstest};
    use tempfile::TempDir;

    use super::*;

    fn new_constant_test() {
        define_named_vector!(TestClass);
        assert_eq!(TestClass::new(), TestClass::new());
    }

    #[test]
    fn constant_attribute_test() {
        define_named_vector!(TestClass, field);
        assert_eq!(TestClass::new(), TestClass::new());
        assert_ne!(TestClass::field(), TestClass::new());
        assert_eq!(TestClass::field(), TestClass::field());
    }

    #[test]
    fn multiple_constant_attributes_test() {
        define_named_vector!(TestClass, field1, field2);
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
        define_named_vector!(TestClass, field1, field2, field3);
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
        define_named_vector!(TestClass, field1, field2);
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
        define_named_vector!(TestClass, field1, field2);
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

    #[test]
    fn test_sum() {
        define_named_vector!(TestClass, field1, field2);
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
        let c = TestClass {
            field1: 5,
            field2: 7,
            operators: HashMap::from([
                (Id::from("<>"), 14),
                (Id::from("--"), 2),
                (Id::from("<=>"), 1),
            ]),
        };
        assert_eq!(
            [a.clone(), b.clone(), c.clone()]
                .into_iter()
                .sum::<TestClass>(),
            a + b + c
        )
    }

    #[test]
    fn test_to_string() {
        define_named_vector!(TestClass, field_a, field_b);
        let vector = TestClass {
            field_a: 3,
            field_b: 9,
            operators: HashMap::from([
                (Id::from("j"), 10),
                (Id::from("a"), 12),
                (Id::from("c"), 10),
            ]),
        };
        assert_eq!(
            vector.to_string(),
            "field_a\tfield_b\ta\tc\tj\n3\t9\t12\t10\t10"
        )
    }

    #[fixture]
    fn temporary_filename() -> PathBuf {
        let tmp_dir = TempDir::new().expect("Could not create temp dir.");
        let tmp = tmp_dir.path().join("filename");
        tmp
    }

    #[rstest]
    fn test_save(temporary_filename: PathBuf) {
        define_named_vector!(TestClass, field_a, field_b);
        let vector = TestClass {
            field_a: 3,
            field_b: 9,
            operators: HashMap::from([
                (Id::from("j"), 10),
                (Id::from("a"), 12),
                (Id::from("c"), 10),
            ]),
        };
        dbg!(&temporary_filename);
        let result = vector.save(temporary_filename.as_path());
        if !result.is_ok() {
            dbg!(&result);
            assert!(result.is_ok())
        }
        let contents = fs::read_to_string(temporary_filename).expect("Failed to read file.");
        assert_eq!(contents, vector.to_string())
    }
}
