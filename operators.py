import enum
import re


class Associativity(enum.IntEnum):
    LEFT = enum.auto()
    RIGHT = enum.auto()


class OperatorManager:
    OPERATOR_PRECEDENCE = {"$": 0, "+": 1, "*": 2}
    LEFT_ASSOCIATIVE_OPERATORS = {"$"}
    OPERATOR_REGEX = r"^[&!+-^$<>@:*|=]+$"

    @classmethod
    def get_precedence(cls, operator: str):
        return cls.OPERATOR_PRECEDENCE.get(operator, len(cls.OPERATOR_PRECEDENCE) + 1)

    @classmethod
    def get_associativity(cls, operator: str):
        if operator in cls.LEFT_ASSOCIATIVE_OPERATORS or not re.match(cls.OPERATOR_REGEX, operator):
            return Associativity.LEFT
        else:
            return Associativity.RIGHT
