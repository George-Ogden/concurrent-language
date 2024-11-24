import sys
from parser.GrammarLexer import GrammarLexer
from parser.GrammarParser import GrammarParser
from parser.GrammarVisitor import GrammarVisitor
from typing import Optional

from antlr4 import *
from antlr4.tree.Trees import Trees

from ast_nodes import (
    ASTNode,
    AtomicType,
    AtomicTypeEnum,
    GenericVariable,
    Integer,
    TupleType,
)


def main(argv):
    input_stream = InputStream(argv[1])
    target = sys.argv[2]
    lexer = GrammarLexer(input_stream)
    stream = CommonTokenStream(lexer)
    parser = GrammarParser(stream)
    assert target in parser.ruleNames
    tree = getattr(parser, target).__call__()
    print(Trees.toStringTree(tree, None, parser))


class Visitor(GrammarVisitor):
    def visitInteger(self, ctx: GrammarParser.IntegerContext):
        value = int(ctx.getText())
        return Integer(value)

    def visitId(self, ctx: GrammarParser.IdContext):
        return ctx.getText()

    def visitAtomic_type(self, ctx: GrammarParser.Atomic_typeContext):
        type_name = ctx.getText().upper()
        type = AtomicTypeEnum[type_name]
        return AtomicType(type)

    def visitType_instance(self, ctx: GrammarParser.Type_instanceContext):
        if ctx.type_instance() is not None:
            return self.visitType_instance(ctx.type_instance())
        return super().visitType_instance(ctx)

    def visitGeneric_list(self, ctx: GrammarParser.Generic_listContext):
        children = (self.visit(child) for child in ctx.getChildren())
        return [child for child in children if child is not None]

    def visitGeneric_instance(self, ctx: GrammarParser.Generic_instanceContext):
        id = self.visitId(ctx.id_())
        generic_list = (
            [] if ctx.generic_list() is None else self.visitGeneric_list(ctx.generic_list())
        )
        return GenericVariable(id, generic_list)

    def visitType_list(self, ctx: GrammarParser.Type_listContext):
        children = (self.visit(child) for child in ctx.getChildren())
        return [child for child in children if child is not None]

    def visitTuple_type(self, ctx: GrammarParser.Tuple_typeContext):
        return TupleType(self.visit(ctx.type_list()))

    def visitInfix_free_expr(self, ctx: GrammarParser.Infix_free_exprContext):
        [child] = ctx.getChildren()
        if ctx.generic_instance() is not None:
            return self.visitGeneric_instance(child)
        return super().visitInfix_free_expr(ctx)


class Parser:
    @staticmethod
    def parse(code: str, target: str) -> Optional[ASTNode]:
        input_stream = InputStream(code)
        lexer = GrammarLexer(input_stream)
        stream = CommonTokenStream(lexer)
        parser = GrammarParser(stream)
        if target in parser.ruleNames:
            tree = getattr(parser, target).__call__()
            if stream.LA(1) != Token.EOF:
                return None
            visitor = Visitor()
            return visitor.visit(tree)


if __name__ == "__main__":
    main(sys.argv)
