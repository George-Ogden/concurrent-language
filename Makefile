.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

PARSER := parser
GRAMMAR := parser/grammar
TYPE_CHECKER := type-checker/target/debug/type-checker
TYPE_CHECKER_MANIFEST := type-checker/Cargo.toml

parse: $(GRAMMAR)
	cat sample.txt | xargs -0 python $(PARSER)

$(GRAMMAR): Grammar.g4
	antlr4 -v 4.13.0 -no-listener -visitor -Dlanguage=Python3 $^  -o $@
	touch $@/__init__.py
	touch $@

$(PARSER): $(GRAMMAR)
	touch $@

$(TYPE_CHECKER): $(PARSER) $(wildcard type-checker/src/*)
	cargo build --manifest-path $(TYPE_CHECKER_MANIFEST)

type-check: $(TYPE_CHECKER)
	cat sample.txt | xargs -0 -t python $(PARSER) | ./$(TYPE_CHECKER)


test: $(PARSER)
	pytest .
	cargo test --manifest-path $(TYPE_CHECKER_MANIFEST)

clean:
	rm -rf $(GRAMMAR)

.PHONY: clean parse
