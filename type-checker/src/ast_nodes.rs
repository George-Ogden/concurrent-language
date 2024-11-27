use serde::{Deserialize, Serialize};

type Id = String;

#[derive(Serialize, Deserialize, Debug)]
enum AtomicTypeEnum {
    INT,
    BOOL,
}

#[derive(Serialize, Deserialize, Debug)]
struct AtomicType {
    type_: AtomicTypeEnum,
}

#[derive(Serialize, Deserialize, Debug)]
struct GenericType {
    id: Id,
    type_variables: Vec<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug)]
struct TupleType {
    types: Vec<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug)]
struct FunctionType {
    argument_type: Box<TypeInstance>,
    return_type: Box<TypeInstance>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum TypeInstance {
    Function(FunctionType),
    Atomic(AtomicType),
    Tuple(TupleType),
    Generic(GenericType),
}
