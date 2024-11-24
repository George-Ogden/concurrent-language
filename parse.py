import sys
from parser.GrammarLexer import GrammarLexer
from parser.GrammarParser import GrammarParser
from parser.GrammarVisitor import GrammarVisitor
from typing import Optional

from antlr4 import *
from antlr4.tree.Trees import Trees

from ast_nodes import ASTNode


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
        return int(ctx.getText())


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
