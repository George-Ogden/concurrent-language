.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

PARSER := parsing
GRAMMAR := parsing/grammar
TYPE_CHECKER := type-checker/target/debug/libtype_checker.d
TYPE_CHECKER_MANIFEST := type-checker/Cargo.toml
TRANSLATOR := translation/target/debug/libtranslation.d
TRANSLATOR_MANIFEST := translation/Cargo.toml
LOWERER := lowering/target/debug/liblowering.d
LOWERER_MANIFEST := lowering/Cargo.toml
COMPILER := compilation/target/debug/libcompilation.d
COMPILER_MANIFEST := compilation/Cargo.toml
OPTIMIZER := optimization/target/debug/optimization
OPTIMIZER_MANIFEST := optimization/Cargo.toml
PIPELINE := pipeline/target/debug/pipeline
PIPELINE_MANIFEST := pipeline/Cargo.toml
BACKEND := backend/bin/main
TARGET := backend/include/main/main.hpp

LAST_FILE_PREFIX := .last-file-hash-
LAST_FILE_HASH = $(shell sha256sum $(FILE) 2>/dev/null | cut -d' ' -f1)
LAST_FILE := $(LAST_FILE_PREFIX)$(LAST_FILE_HASH)

$(LAST_FILE):
	rm $(LAST_FILE_PREFIX)* -f
	touch $@

all: $(PIPELINE) $(BACKEND)

FILE := samples/samples.txt

run: build
	sudo make -C backend run --quiet

build: $(TARGET)
	make -C backend build

$(TARGET): $(PIPELINE) $(FILE) $(LAST_FILE)
	cat $(FILE) | xargs -0 python $(PARSER) | ./$(PIPELINE) > $(TARGET)

$(TYPE_CHECKER): $(wildcard type-checker/src/*) $(PARSER)
	cargo build --manifest-path $(TYPE_CHECKER_MANIFEST)
	touch $@

$(LOWERER): $(wildcard lowering/src/*) $(TYPE_CHECKER)
	cargo build --manifest-path $(LOWERER_MANIFEST)
	touch $@

$(COMPILER): $(wildcard compilation/src/*) $(LOWERER)
	cargo build --manifest-path $(COMPILER_MANIFEST)
	touch $@

$(TRANSLATOR): $(wildcard translation/src/*) $(COMPILER)
	cargo build --manifest-path $(TRANSLATOR_MANIFEST)
	touch $@

$(OPTIMIZER): $(wildcard optimization/src/*) $(LOWERER)
	cargo build --manifest-path $(OPTIMIZER_MANIFEST)
	touch $@

$(PIPELINE): $(wildcard pipeline/src/*) $(TRANSLATOR) $(OPTIMIZER)
	cargo build --manifest-path $(PIPELINE_MANIFEST)
	touch $@

$(BACKEND):
	make -C backend

$(PARSER): $(GRAMMAR)
	touch $@

$(GRAMMAR): Grammar.g4
	antlr4 -v 4.13.0 -no-listener -visitor -Dlanguage=Python3 $^  -o $@
	touch $@/__init__.py
	touch $@

test: build
	for sample in benchmark/**/main.txt; do \
		make build FILE=$$sample || exit 1; \
	done;
	pytest . -vv
	cargo test --manifest-path $(TYPE_CHECKER_MANIFEST) -vv --lib
	cargo test --manifest-path $(LOWERER_MANIFEST) -vv --lib
	cargo test --manifest-path $(COMPILER_MANIFEST) -vv --lib
	cargo test --manifest-path $(TRANSLATOR_MANIFEST) -vv --lib
	cargo test --manifest-path $(OPTIMIZER_MANIFEST) -vv --lib
	cargo test --manifest-path $(PIPELINE_MANIFEST) -vv
	make -C backend bin/test
	ASAN_OPTIONS=detect_stack_use_after_return=1:detect_leaks=0 ./backend/bin/test --gtest_repeat=10 --gtest_shuffle --gtest_random_seed=10 --gtest_brief=0 --gtest_print_time=1
	for sample in samples/*; do \
		if [ "$$sample" != "samples/grammar.txt" ]; then \
			make build FILE=$$sample || exit 1; \
		fi \
	done;

clean:
	rm -rf $(GRAMMAR)
	cargo clean --manifest-path $(TYPE_CHECKER_MANIFEST)
	make -C backend clean
	find -path '*/__pycache__*' -delete

LOG_DIR := logs/$(shell date +%Y%m%d%H%M%S%N)
REPEATS := 10

$(LOG_DIR):
	mkdir -p $@

benchmark: $(LOG_DIR)
	git log --format="%H" -n 1 > $^/git
	echo "name\targs\tduration" > $(LOG_DIR)/log.tsv
		for i in `seq 1 $(REPEATS)`; do \
	for program in benchmark/**; do \
		make build FILE=$$program/main.txt; \
			while read input; do  \
				echo $$program $$input; \
				sudo timeout 60 ./backend/bin/main $$input 2>&1 > /dev/null \
				| { if read -r output; then echo "$$output"; else echo "nan"; fi; } \
				| sed -E 's/Execution time: ([[:digit:]]+)ns.*/\1/' \
				| xargs printf '%s\t' \
					`echo $$program | sed 's/benchmark\///'| sed 's/\///g'` \
					`echo $$input | xargs printf '%s,' | sed 's/,$$//'` \
				| xargs -0 echo  >> $(LOG_DIR)/log.tsv; \
			done < $$program/input.txt; \
		done; \
	done;

.PHONY: all benchmark clean run
