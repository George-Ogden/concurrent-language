parse: parser
	python parse.py sample.txt

parser:
	antlr4 -no-listener -Dlanguage=Python3 Grammar.g4  -o $@
	touch $@/__init__.py

clean:
	rm -rf parser

.PHONY: clean parse
