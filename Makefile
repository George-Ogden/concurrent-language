.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

PARSER := parsing
GRAMMAR := parsing/grammar
TYPE_CHECKER := type-checker/target/release/libtype_checker.d
TYPE_CHECKER_MANIFEST := type-checker/Cargo.toml
TRANSLATOR := translation/target/release/libtranslation.d
TRANSLATOR_MANIFEST := translation/Cargo.toml
LOWERER := lowering/target/release/liblowering.d
LOWERER_MANIFEST := lowering/Cargo.toml
COMPILER := compilation/target/release/libcompilation.d
COMPILER_MANIFEST := compilation/Cargo.toml
OPTIMIZER := optimization/target/release/optimization
OPTIMIZER_MANIFEST := optimization/Cargo.toml
PIPELINE := pipeline/target/release/pipeline
PIPELINE_MANIFEST := pipeline/Cargo.toml
BACKEND := backend/bin/main
TARGET := backend/include/main/main.hpp

FRONTEND_FLAGS :=
BACKEND_FLAGS :=
FLAGS_HASH := $(shell echo '$(FRONTEND_FLAGS) / $(BACKEND_FLAGS)' | sha256sum - 2> /dev/null | cut -d' ' -f1)

FILE := samples/main.txt
LAST_FILE_PREFIX := .last-file-hash-
LAST_FILE_HASH = $(shell sha256sum '$(FILE)' 2>/dev/null | cut -d' ' -f1)
LAST_FILE := $(LAST_FILE_PREFIX)$(LAST_FILE_HASH)$(FLAGS_HASH)

all: $(PIPELINE) $(BACKEND)

USER_FLAG := 0

$(LAST_FILE):
	rm $(LAST_FILE_PREFIX)* -f
	touch $@

run: build
	$(if $(filter 1,$(USER_FLAG)), , sudo) make -C backend EXTRA_FLAGS='$(BACKEND_FLAGS)' run --quiet INPUT='$(INPUT)'

build: $(TARGET)
	make -C backend build EXTRA_FLAGS='$(BACKEND_FLAGS)'

$(TARGET): $(PIPELINE) $(FILE) $(LAST_FILE)
	cat $(FILE) | xargs -0 python $(PARSER) | ./$(PIPELINE) $(FRONTEND_FLAGS) > $(TARGET)

$(TYPE_CHECKER): $(wildcard type-checker/src/*) $(PARSER)
	cargo build --manifest-path $(TYPE_CHECKER_MANIFEST) --release
	touch $@

$(LOWERER): $(wildcard lowering/src/*) $(TYPE_CHECKER)
	cargo build --manifest-path $(LOWERER_MANIFEST) --release
	touch $@

$(COMPILER): $(wildcard compilation/src/*) $(LOWERER)
	cargo build --manifest-path $(COMPILER_MANIFEST) --release
	touch $@

$(TRANSLATOR): $(wildcard translation/src/*) $(COMPILER)
	cargo build --manifest-path $(TRANSLATOR_MANIFEST) --release
	touch $@

$(OPTIMIZER): $(wildcard optimization/src/*) $(LOWERER)
	cargo build --manifest-path $(OPTIMIZER_MANIFEST) --release
	touch $@

$(PIPELINE): $(wildcard pipeline/src/*) $(TRANSLATOR) $(OPTIMIZER)
	cargo build --manifest-path $(PIPELINE_MANIFEST) --release
	touch $@

$(BACKEND): $(TARGET)
	make -C backend build USER_FLAG=$(USER_FLAG) EXTRA_FLAGS='$(BACKEND_FLAGS)'
	touch $@

$(PARSER): $(GRAMMAR)
	touch $@

$(GRAMMAR): Grammar.g4
	antlr4 -v 4.13.0 -no-listener -visitor -Dlanguage=Python3 $^ -o $@
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
LOG_FILE := $(LOG_DIR)/log.tsv
VECTOR_FILE := $(LOG_DIR)/vector.tsv
TEMPFILE := $(shell mktemp)

$(LOG_DIR):
	mkdir -p $@
	git log --format="%H" -n 1 > $@/git

benchmark: | $(LOG_DIR)
	touch $(LOG_DIR)/title.txt

	echo "name\targs\tduration" > $(LOG_DIR)/log.tsv
	for program in benchmark/**; do \
		for i in `seq 1 $(REPEATS)`; do \
			make build FILE=$$program/main.txt USER_FLAG=-1; \
			while read input; do \
				echo $$program $$input; \
				make time --silent FILE=$$program/main.txt USER_FLAG=-1 INPUT="$$input" \
				| xargs printf '%s\t' \
					`echo $$program | sed 's/benchmark\///'| sed 's/\///g'` \
					`echo $$input | xargs printf '%s,' | sed 's/,$$//'` \
				| xargs -0 echo >> $(LOG_FILE); \
			done < $$program/input.txt; \
		done; \
	done;

python_benchmark: | $(LOG_DIR)
	echo "python benchmark" > $(LOG_DIR)/title.txt

	echo "name\targs\tduration" > $(LOG_DIR)/log.tsv
	for i in `seq 1 $(REPEATS)`; do \
		for program in benchmark/**; do \
			while read input; do \
				echo $$program $$input; \
				sudo chrt -f $(MAX_PRIORITY) python scripts/benchmark.py $$program/main.py "$$input" \
				| xargs printf '%s\t' \
					`echo $$program | sed 's/benchmark\///'| sed 's/\///g'` \
					`echo $$input | xargs printf '%s,' | sed 's/,$$//'` \
				| xargs -0 echo >> $(LOG_FILE); \
			done < $$program/input.txt; \
		done; \
	done;


$(VECTOR_FILE): | $(LOG_DIR)
	make $(TARGET) FRONTEND_FLAGS="--export-vector-file $(TEMPFILE)"
	head -1 $(TEMPFILE) | sed 's/$$/\ttime/' | sed 's/^/sample\t/' > $@

timings: $(VECTOR_FILE)
	for program in timing/**; do \
		for i in `seq 1 $(REPEATS)`; do \
			export program_name=`echo $$program | sed 's/.*\///'`; \
			make build FILE=$$program/main.txt FRONTEND_FLAGS="--export-vector-file $(TEMPFILE)"; \
			for input in `seq 0 64`; do \
				echo $$program $$input; \
				make time --silent FILE=$$program/main.txt INPUT="$$input" LIMIT=0 FRONTEND_FLAGS="--export-vector-file $(TEMPFILE)" \
				| sed "s/^/$$program_name\t`tail -1 $(TEMPFILE)`\t/" \
				>> $(VECTOR_FILE); \
			done; \
		done; \
	done;

LIMIT := 60
time: build
	if [ "$(LIMIT)" = "0" ]; then \
		sudo ./$(BACKEND) $(INPUT) 2>&1 > /dev/null; \
	else \
		sudo chrt -f $(MAX_PRIORITY) timeout $(LIMIT) chrt -f 1 ./$(BACKEND) $(INPUT) 2>&1 > /dev/null; \
	fi \
	| { if read -r output; then echo "$$output"; else echo; fi; } \
	| grep -E '$(PATTERN)' \
	| sed -E 's/$(PATTERN)/\1/' \
	| grep . \
	|| echo nan \

.PHONY: all benchmark build clean run time timings
