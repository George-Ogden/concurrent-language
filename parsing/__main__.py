import json
import sys
from parser import Parser


def main(argv):
    """Expect input as `python main.py [CODE] [TARGET]?`, where the default TARGET is "program"."""
    code = argv[1]
    target = sys.argv[2] if len(sys.argv) >= 3 else "program"
    ast = Parser.parse(code, target=target)
    if ast:
        print(json.dumps(ast.to_json()))


if __name__ == "__main__":
    main(sys.argv)
