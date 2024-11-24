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
    FunctionType,
    GenericVariable,
    Integer,
    TupleExpression,
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
            return self.visit(ctx.type_instance())
        return super().visitType_instance(ctx)

    def visitGeneric_list(self, ctx: GrammarParser.Generic_listContext):
        children = (self.visit(child) for child in ctx.getChildren())
        return [child for child in children if child is not None]

    def visitGeneric_instance(self, ctx: GrammarParser.Generic_instanceContext):
        id = self.visitId(ctx.id_())
        generic_list = [] if ctx.generic_list() is None else self.visit(ctx.generic_list())
        return GenericVariable(id, generic_list)

    def visitType_list(self, ctx: GrammarParser.Type_listContext):
        children = (self.visit(child) for child in ctx.getChildren())
        return [child for child in children if child is not None]

    def visitTuple_type(self, ctx: GrammarParser.Tuple_typeContext):
        return TupleType(self.visit(ctx.type_list()))

    def visitFn_type_head(self, ctx: GrammarParser.Fn_type_headContext):
        if ctx.return_type() is not None:
            return self.visit(ctx.return_type())
        else:
            return self.visit(ctx.type_instance())

    def visitFn_type(self, ctx: GrammarParser.Fn_typeContext):
        argument_types = self.visit(ctx.fn_type_head())
        return_type = self.visit(ctx.fn_type_tail())
        return FunctionType(argument_types, return_type)

    def visitExpr_list(self, ctx: GrammarParser.Expr_listContext):
        children = (self.visit(child) for child in ctx.getChildren())
        return [child for child in children if child is not None]

    def visitTuple_expr(self, ctx: GrammarParser.Tuple_exprContext):
        expressions = self.visit(ctx.expr_list())
        return TupleExpression(expressions)

    def visitInfix_free_expr(self, ctx: GrammarParser.Infix_free_exprContext):
        if ctx.expr() is not None:
            return self.visit(ctx.expr())
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
