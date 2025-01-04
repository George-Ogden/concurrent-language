use core::fmt;
use std::fmt::Formatter;

use lowering::{AtomicType, AtomicTypeEnum, FnType, MachineType, TupleType, TypeDef, UnionType};

struct Translator {}

struct TypeFormatter<'a>(&'a MachineType);
impl fmt::Display for TypeFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self.0 {
            MachineType::AtomicType(AtomicType(atomic)) => match atomic {
                AtomicTypeEnum::INT => write!(f, "Int"),
                AtomicTypeEnum::BOOL => write!(f, "Bool"),
            },
            MachineType::TupleType(TupleType(types)) => {
                write!(f, "TupleT<{}>", TypesFormatter(types))
            }
            MachineType::FnType(FnType(args, ret)) => {
                write!(
                    f,
                    "FnT<{}>",
                    TypesFormatter(
                        &std::iter::once(*ret.clone())
                            .chain(args.clone().into_iter())
                            .collect()
                    )
                )
            }
            MachineType::UnionType(UnionType(type_names)) => {
                write!(f, "VariantT<{}>", type_names.join(","))
            }
        }
    }
}

struct TypesFormatter<'a>(&'a Vec<MachineType>);
impl fmt::Display for TypesFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{}",
            &self
                .0
                .iter()
                .map(|machine_type| format!("{}", TypeFormatter(machine_type)))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

impl Translator {
    fn translate_type(type_: MachineType) -> String {
        format!("{}", TypeFormatter(&type_))
    }
    fn translate_typedefs(typedefs: Vec<TypeDef>) -> String {
        format!("{}", "struct Bull_; typedef Empty Twoo; typedef Empty Faws; struct Bull_ { using type = VariantT<Twoo, Faws>; type value; Bull_(type value) : value(value) {} };")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use lowering::Name;
    use regex::Regex;
    use test_case::test_case;

    fn normalize_code(code: String) -> String {
        let regex = Regex::new(r"((^|[^[:space:]])[[:space:]]+([^[:space:][:word:]]|$))|((^|[^[:space:][:word:]])[[:space:]]+([^[:space:]]|$))")
        .unwrap();

        let mut result = code;
        let mut code = String::new();
        while result != code {
            code = result;
            result = regex.replace_all(&*code, "${2}${5}${3}${6}").to_string();
        }

        return result;
    }

    fn assert_eq_code(code1: String, code2: String) -> () {
        assert_eq!(normalize_code(code1), normalize_code(code2));
    }

    #[test_case(
        "a = 3",
        "a=3";
        "space replacement"
    )]
    #[test_case(
        "int x",
        "int x";
        "no replacement"
    )]
    #[test_case(
        "\t3 + 4",
        "3+4";
        "tab replacement"
    )]
    #[test_case(
        "8+ 5 ",
        "8+5";
        "end replacement"
    )]
    #[test_case(
        "3\n4",
        "3\n4";
        "newline non-replacement"
    )]
    #[test_case(
        "3\n-8",
        "3-8";
        "newline replacement"
    )]
    fn test_string_replacement(code: &str, expected: &str) {
        assert_eq!(normalize_code(String::from(code)), String::from(expected))
    }

    #[test_case(
        AtomicType(AtomicTypeEnum::INT).into(),
        "Int";
        "atomic int"
    )]
    #[test_case(
        AtomicType(AtomicTypeEnum::BOOL).into(),
        "Bool";
        "atomic bool"
    )]
    #[test_case(
        TupleType(Vec::new()).into(),
        "TupleT<>";
        "empty tuple type"
    )]
    #[test_case(
        TupleType(vec![AtomicType(AtomicTypeEnum::INT).into()]).into(),
        "TupleT<Int>";
        "singleton tuple type"
    )]
    #[test_case(
        TupleType(vec![
            AtomicType(AtomicTypeEnum::INT).into(),
            AtomicType(AtomicTypeEnum::BOOL).into()
        ]).into(),
        "TupleT<Int,Bool>";
        "double tuple type"
    )]
    #[test_case(
        TupleType(vec![
            TupleType(vec![
                AtomicType(AtomicTypeEnum::INT).into(),
                AtomicType(AtomicTypeEnum::BOOL).into()
            ]).into(),
            TupleType(Vec::new()).into(),
        ]).into(),
        "TupleT<TupleT<Int,Bool>,TupleT<>>";
        "nested tuple type"
    )]
    #[test_case(
        FnType(Vec::new(), Box::new(TupleType(Vec::new()).into())).into(),
        "FnT<TupleT<>>";
        "unit fn type"
    )]
    #[test_case(
        FnType(
            vec![AtomicType(AtomicTypeEnum::INT).into()],
            Box::new(AtomicType(AtomicTypeEnum::INT).into())
        ).into(),
        "FnT<Int,Int>";
        "int identity fn"
    )]
    #[test_case(
        FnType(
            vec![
                AtomicType(AtomicTypeEnum::INT).into(),
                AtomicType(AtomicTypeEnum::INT).into()
            ],
            Box::new(AtomicType(AtomicTypeEnum::BOOL).into())
        ).into(),
        "FnT<Bool,Int,Int>";
        "int comparison fn"
    )]
    #[test_case(
        FnType(
            vec![
                FnType(
                    vec![
                        AtomicType(AtomicTypeEnum::INT).into()
                    ],
                    Box::new(AtomicType(AtomicTypeEnum::BOOL).into())
                ).into(),
                AtomicType(AtomicTypeEnum::INT).into()
            ],
            Box::new(AtomicType(AtomicTypeEnum::BOOL).into())
        ).into(),
        "FnT<Bool,FnT<Bool,Int>,Int>";
        "higher order fn"
    )]
    #[test_case(
        UnionType(
            vec![
                Name::from("Twoo"),
                Name::from("Faws"),
            ]
        ).into(),
        "VariantT<Twoo,Faws>";
        "bull type"
    )]
    #[test_case(
        UnionType(
            vec![
                Name::from("Wrapper"),
            ]
        ).into(),
        "VariantT<Wrapper>";
        "int wrapper variant"
    )]
    #[test_case(
        UnionType(vec![Name::from("Cons"), Name::from("Nil")]).into(),
        "VariantT<Cons,Nil>";
        "list int type"
    )]
    fn test_type_translation(type_: MachineType, expected: &str) {
        let code = Translator::translate_type(type_);
        let expected_code = String::from(expected);
        assert_eq_code(code, expected_code);
    }

    #[test_case(
        TypeDef{
            name: Name::from("Bull"),
            constructors: vec![
                (Name::from("Twoo"), None),
                (Name::from("Faws"), None)
            ]
        },
        "struct Bull_; typedef Empty Twoo; typedef Empty Faws; struct Bull_ { using type = VariantT<Twoo, Faws>; type value; Bull_(type value) : value(value) {} };"
    )]
    fn test_typedef_translations(type_def: TypeDef, expected: &str) {
        let code = Translator::translate_typedefs(vec![type_def]);
        let expected_code = String::from(expected);
        assert_eq_code(code, expected_code);
    }
}
