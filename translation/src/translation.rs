use lowering::{Atomic, AtomicTypeEnum, MachineType};

struct Translator {}

impl Translator {
    fn translate_type(type_: MachineType) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalize_code(code: String) -> String {
        return String::new();
    }

    fn assert_eq_code(code1: String, code2: String) -> () {
        assert_eq!(normalize_code(code1), normalize_code(code2));
    }

    #[test]
    fn test_type_translation() {
        let type_: MachineType = Atomic(AtomicTypeEnum::INT).into();
        let code = Translator::translate_type(type_);
        let expected_code = String::from("Int");
        assert_eq_code(code, expected_code);
    }
}
