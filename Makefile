.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

PARSER := parser
GRAMMAR := parser/grammar
TYPE_CHECKER := type-checker/target/debug/type_checker
TYPE_CHECKER_MANIFEST := type-checker/Cargo.toml
TRANSLATOR := translation/target/debug/translation
TRANSLATOR_MANIFEST := translation/Cargo.toml
LOWERER := lowering/target/debug/lowering
LOWERER_MANIFEST := lowering/Cargo.toml
COMPILER := compilation/target/debug/compilation
COMPILER_MANIFEST := compilation/Cargo.toml
PIPELINE := pipeline/target/debug/pipeline
PIPELINE_MANIFEST := pipeline/Cargo.toml
BACKEND := backend/bin/main

all: $(PIPELINE) $(BACKEND)

run: $(PIPELINE)
	cat samples/simple.txt | xargs -0 python $(PARSER) | ./$(PIPELINE) > backend/include/main/main.hpp
	sudo make -C backend run

$(TYPE_CHECKER): $(wildcard type-checker/src/*) $(PARSER)
	cargo build --manifest-path $(TYPE_CHECKER_MANIFEST)

$(LOWERER): $(wildcard lowering/src/*) $(TYPE_CHECKER)
	cargo build --manifest-path $(LOWERER_MANIFEST)

$(COMPILER): $(wildcard compilation/src/*) $(LOWERER)
	cargo build --manifest-path $(COMPILER_MANIFEST)

$(TRANSLATOR): $(wildcard translation/src/*) $(COMPILER)
	cargo build --manifest-path $(TRANSLATOR_MANIFEST)

$(PIPELINE): $(wildcard pipeline/src/*) $(TRANSLATOR)
	cargo build --manifest-path $(PIPELINE_MANIFEST)

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

translate: $(TRANSLATOR)
	echo '{"type_defs":[],"globals":[],"fn_defs":[{"name":"PreMain","arguments":[],"statements":[{"Assignment":{"allocation":{"Lazy":{"AtomicType":"INT"}},"target":"x","value":{"Wrap":[{"BuiltIn":{"Integer":{"value":0}}},{"AtomicType":"INT"}]}}}],"ret":[{"Memory":"x"},{"Lazy":{"AtomicType":"INT"}}],"env":null,"allocations":[["main",{"FnType":[[],{"Lazy":{"AtomicType":"INT"}}]}]]}]}' | ./$(TRANSLATOR) | tee backend/include/main/main.hpp

test: $(PARSER) $(TYPE_CHECKER)
	pytest . -vv
	cargo test --manifest-path $(TYPE_CHECKER_MANIFEST) -vv --lib
	cargo test --manifest-path $(LOWERER_MANIFEST) -vv --lib
	cargo test --manifest-path $(TRANSLATOR_MANIFEST) -vv
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
