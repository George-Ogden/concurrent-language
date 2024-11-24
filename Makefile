.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))



parse: parser
	python parse.py sample.txt

parser: Grammar.g4
	antlr4 -no-listener -visitor -Dlanguage=Python3 $^  -o $@
	touch $@/__init__.py
	touch $@

test: parser
	pytest . -vv

clean:
	rm -rf parser

.PHONY: clean parse
