.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

PARSER := parser
GRAMMAR := parser/grammar
TYPE_CHECKER := type-checker/target/debug/type_checker
TYPE_CHECKER_MANIFEST := type-checker/Cargo.toml
BACKEND := backend/bin/main

all: $(TYPE_CHECKER) $(PARSER) $(BACKEND)

$(TYPE_CHECKER): $(PARSER) $(wildcard type-checker/src/*)
	cargo build --manifest-path $(TYPE_CHECKER_MANIFEST)

$(BACKEND):
	make -C backend

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

test: $(PARSER) $(TYPE_CHECKER)
	pytest . -vv
	cargo test --manifest-path $(TYPE_CHECKER_MANIFEST) -vv
	make -C backend bin/test
	ASAN_OPTIONS=detect_leaks=0 ./backend/bin/test --gtest_repeat=10 --gtest_shuffle --gtest_random_seed=10 --gtest_brief=0 --gtest_print_time=1
	for sample in samples/triangular.txt samples/list.txt; do \
		cat $$sample | xargs -0 -t python $(PARSER) | ./$(TYPE_CHECKER); \
	done;

clean:
	rm -rf $(GRAMMAR)
	cargo clean --manifest-path $(TYPE_CHECKER_MANIFEST)
	make -C backend clean
	find -path '*/__pycache__*' -delete

.PHONY: all clean parse type-check
