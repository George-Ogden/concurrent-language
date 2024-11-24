import sys
from parser.GrammarLexer import GrammarLexer
from parser.GrammarParser import GrammarParser
from parser.GrammarVisitor import GrammarVisitor
from typing import Optional

from antlr4 import *
from antlr4.tree.Trees import Trees

from ast_nodes import ASTNode, GenericVariable, Integer


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
        return Integer(int(ctx.getText()))

    def visitId(self, ctx: GrammarParser.IdContext):
        return ctx.getText()

    def visitGeneric_list(self, ctx: GrammarParser.Generic_listContext):
        children = (self.visit(child) for child in ctx.getChildren())
        return [child for child in children if child is not None]

    def visitGeneric(self, ctx: GrammarParser.GenericContext):
        return self.visitGeneric_list(ctx.generic_list())

    def visitGenericVariable(self, ctx: GrammarParser.Generic_idContext):
        children = [self.visit(child) for child in ctx.getChildren()]
        return GenericVariable(*children)

    def visitInfix_free_expr(self, ctx: GrammarParser.Infix_free_exprContext):
        [child] = ctx.getChildren()
        match child.getRuleIndex():
            case GrammarParser.RULE_generic_id:
                return self.visitGenericVariable(child)
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
