import enum
import re


class Associativity(enum.IntEnum):
    LEFT = enum.auto()
    RIGHT = enum.auto()
    NONE = enum.auto()


class OperatorManager:
    OPERATOR_PRECEDENCE = {
        "@": 2,
        "**": 3,
        "*": 4,
        "/": 5,
        "%": 6,
        "+": 7,
        "-": 7,
        ">>": 8,
        "<<": 8,
        "::": 9,
        "++": 9,
        "--": 9,
        "<=>": 10,
        "<": 11,
        "<=": 11,
        ">": 11,
        ">=": 11,
        "==": 11,
        "!=": 11,
        "&": 12,
        "^": 13,
        "|": 14,
        "&&": 15,
        "||": 16,
        "|>": 17,
        "$": 18,
    }

    LEFT_ASSOCIATIVE_OPERATORS = {"$", "@", "::", "**", "++", "--"}
    NON_ASSOCIATIVE_OPERATORS = {"<", ">", "<=", ">=", "<=>", "==", "!="}
    OPERATOR_REGEX = r"^[&!+/\-^$<>@:*|%=.]+$"

    @classmethod
    def check_operator(cls, operator: str) -> bool:
        """Returns whether `operator` could be a valid operator (grammatically)."""
        return re.match(cls.OPERATOR_REGEX, operator) is not None

    @classmethod
    def get_precedence(cls, operator: str):
        if not cls.check_operator(operator):
            return -2
        return cls.OPERATOR_PRECEDENCE.get(operator, -1)

    @classmethod
    def get_associativity(cls, operator: str):
        if operator in cls.LEFT_ASSOCIATIVE_OPERATORS or not cls.check_operator(operator):
            return Associativity.LEFT
        elif operator in cls.NON_ASSOCIATIVE_OPERATORS:
            return Associativity.NONE
        else:
            return Associativity.RIGHT
