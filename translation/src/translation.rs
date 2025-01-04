use lowering::{Atomic, AtomicTypeEnum, MachineType};
use regex::{Captures, Regex};

struct Translator {}

impl Translator {
    fn translate_type(type_: MachineType) -> String {
        return String::from("Int");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    fn normalize_code(code: String) -> String {
        let regex = Regex::new(r"((^|[^[:space:]])[[:space:]]+([^[:space:][:word:]]|$))|((^|[^[:space:][:word:]])[[:space:]]+([^[:space:]]|$))")
        .unwrap();

        let mut result = code;
        let mut code = String::new();
        while (result != code) {
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

    #[test]
    fn test_type_translation() {
        let type_: MachineType = Atomic(AtomicTypeEnum::INT).into();
        let code = Translator::translate_type(type_);
        let expected_code = String::from("Int");
        assert_eq_code(code, expected_code);
    }
}
