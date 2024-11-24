import sys
from parser.GrammarLexer import GrammarLexer
from parser.GrammarParser import GrammarParser

from antlr4 import *
from antlr4.tree.Trees import Trees


def main(argv):
    input_stream = FileStream(argv[1])
    lexer = GrammarLexer(input_stream)
    stream = CommonTokenStream(lexer)
    parser = GrammarParser(stream)
    tree = parser.program()
    print(Trees.toStringTree(tree, None, parser))


if __name__ == "__main__":
    main(sys.argv)
