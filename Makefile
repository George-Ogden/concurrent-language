.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

parse: parser
	python parse.py sample.txt

parser:
	antlr4 -no-listener -visitor -Dlanguage=Python3 Grammar.g4  -o $@
	touch $@/__init__.py
	touch $@

test: parser
	pytest .

clean:
	rm -rf parser

.PHONY: clean parse
