.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

parse: parser
	cat sample.txt | xargs -0 python parse.py

parser: Grammar.g4
	antlr4 -no-listener -visitor -Dlanguage=Python3 $^  -o $@
	touch $@/__init__.py
	touch $@

test: parser
	pytest .

clean:
	rm -rf parser

.PHONY: clean parse
