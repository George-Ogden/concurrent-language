.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

parse: grammar
	cat sample.txt | xargs -0 python parser

grammar: Grammar.g4
	antlr4 -no-listener -visitor -Dlanguage=Python3 $^  -o parser/$@
	touch parser/$@/__init__.py
	touch parser/$@

test: parser
	pytest .

clean:
	rm -rf parser

.PHONY: clean parse
