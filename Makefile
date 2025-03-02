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
USER_FLAG := 0

run: build
	$(if $(filter 1,$(USER_FLAG)), , sudo) make -C backend run --quiet

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
	make -C backend USER_FLAG=$(USER_FLAG)
	touch $@

$(PARSER): $(GRAMMAR)
	touch $@

$(GRAMMAR): Grammar.g4
	antlr4 -v 4.13.0 -no-listener -visitor -Dlanguage=Python3 $^  -o $@
	touch $@/__init__.py
	touch $@

test: build
	pytest parsing -vv
	cargo test --manifest-path $(TYPE_CHECKER_MANIFEST) -vv --lib
	cargo test --manifest-path $(LOWERER_MANIFEST) -vv --lib
	cargo test --manifest-path $(COMPILER_MANIFEST) -vv --lib
	cargo test --manifest-path $(TRANSLATOR_MANIFEST) -vv --lib
	cargo test --manifest-path $(OPTIMIZER_MANIFEST) -vv --lib
	cargo test --manifest-path $(PIPELINE_MANIFEST) -vv
	make -C backend bin/test
	./backend/bin/test --gtest_repeat=10 --gtest_shuffle --gtest_random_seed=10 --gtest_brief=0 --gtest_print_time=1
	for sample in samples/*; do \
		if [ "$$sample" != "samples/grammar.txt" ]; then \
			make build FILE=$$sample || exit 1; \
		fi \
	done;
	for sample in benchmark/**; do \
		make build FILE=$$sample/main.txt USER_FLAG=1 || exit 1; \
		for i in `seq 1 10`; do \
			cat $$sample/input.txt | head -1 | xargs ./$(BACKEND) || exit 1; \
		done; \
	done;
	pytest scripts -vv

clean:
	rm -rf $(GRAMMAR)
	cargo clean --manifest-path $(TYPE_CHECKER_MANIFEST)
	make -C backend clean
	find -path '*/__pycache__*' -delete

LOG_DIR := logs/$(shell date +%Y%m%d%H%M%S%N)
REPEATS := 10
MAX_PRIORITY := $(shell chrt -m | awk -F '[:/]' '/SCHED_FIFO/ {print $$NF}')
PATTERN := Execution time: ([[:digit:]]+) ?ns.*

$(LOG_DIR):
	mkdir -p $@

benchmark: $(LOG_DIR)
	git log --format="%H" -n 1 > $^/git
	touch $^/title.txt

	echo "name\targs\tduration" > $(LOG_DIR)/log.tsv
	for i in `seq 1 $(REPEATS)`; do \
		for program in benchmark/**; do \
			make build FILE=$$program/main.txt USER_FLAG=-1;  \
			while read input; do  \
				echo $$program $$input; \
				make time --silent FILE=$$program/main.txt USER_FLAG=-1 INPUT="$$input" \
				| xargs printf '%s\t' \
					`echo $$program | sed 's/benchmark\///'| sed 's/\///g'` \
					`echo $$input | xargs printf '%s,' | sed 's/,$$//'` \
				| xargs -0 echo  >> $(LOG_DIR)/log.tsv; \
			done < $$program/input.txt; \
		done; \
	done;

LIMIT := 60
time: $(BACKEND)
	echo $(INPUT) | sudo setsid chrt -f $(MAX_PRIORITY) bash -c '\
		sleep $(LIMIT) & \
		SLEEP_PID=$$!; \
		cat <(xargs $(BACKEND) 2>&1 > /dev/null; kill $$SLEEP_PID) & \
		EXEC_PID=$$!; \
		wait $$SLEEP_PID || exit 0 && (kill -9 -- -$$EXEC_PID; exit 1) \
	' \
	| { if read -r output; then echo "$$output"; else echo; fi; } \
	| grep -E '$(PATTERN)' \
	| sed -E 's/$(PATTERN)/\1/'  \
	| grep . \
	|| echo nan \


.PHONY: all benchmark build clean run time
