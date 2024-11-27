import sys
from parser import Parser


def main(argv):
    code = argv[1]
    target = sys.argv[2] if len(sys.argv) >= 3 else "program"
    ast = Parser.parse(code, target=target)
    print(ast)


if __name__ == "__main__":
    main(sys.argv)
