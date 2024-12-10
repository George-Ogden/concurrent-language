.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

PARSER := parser
GRAMMAR := parser/grammar
TYPE_CHECKER := type-checker/target/debug/type-checker
TYPE_CHECKER_MANIFEST := type-checker/Cargo.toml

$(TYPE_CHECKER): $(PARSER) $(wildcard type-checker/src/*)
	cargo build --manifest-path $(TYPE_CHECKER_MANIFEST)

$(PARSER): $(GRAMMAR)
	touch $@

$(GRAMMAR): Grammar.g4
	antlr4 -v 4.13.0 -no-listener -visitor -Dlanguage=Python3 $^  -o $@
	touch $@/__init__.py
	touch $@

parse: $(GRAMMAR)
	cat samples/grammar.txt | xargs -0 python $(PARSER)

type-check: $(TYPE_CHECKER)
	cat samples/triangular.txt | xargs -0 -t python $(PARSER) | ./$(TYPE_CHECKER)

test: $(PARSER)
	pytest . -vv
	cargo test --manifest-path $(TYPE_CHECKER_MANIFEST) -vv

clean:
	rm -rf $(GRAMMAR)

.PHONY: clean parse type-check
