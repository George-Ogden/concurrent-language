# Include Makefile in dependencies.
.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

PARSER := parsing
GRAMMAR := parsing/grammar
TYPE_CHECKER := type-checker/target/release/libtype_checker.d
TYPE_CHECKER_MANIFEST := type-checker/Cargo.toml
EMITTER := emission/target/release/libemission.d
EMITTER_MANIFEST := emission/Cargo.toml
LOWERER := lowering/target/release/liblowering.d
LOWERER_MANIFEST := lowering/Cargo.toml
TRANSLATOR := translation/target/release/libtranslation.d
TRANSLATOR_MANIFEST := translation/Cargo.toml
OPTIMIZER := optimization/target/release/optimization
OPTIMIZER_MANIFEST := optimization/Cargo.toml
PIPELINE := pipeline/target/release/pipeline
PIPELINE_MANIFEST := pipeline/Cargo.toml
BACKEND := backend/bin/main
TARGET := backend/include/main/main.hpp

FRONTEND_FLAGS :=
BACKEND_FLAGS :=
FLAGS_HASH := $(shell echo '$(FRONTEND_FLAGS)' | sha256sum - 2> /dev/null | cut -d' ' -f1)

FILE := samples/main.apfl
LAST_FILE_PREFIX := .last-file-hash-
LAST_FILE_HASH = $(shell sha256sum '$(FILE)' 2>/dev/null | cut -d' ' -f1)
# Create file containing flags and rebuild if it changes.
LAST_FILE := $(LAST_FILE_PREFIX)$(LAST_FILE_HASH)$(FLAGS_HASH)

all: $(PIPELINE) $(BACKEND)

# USER_FLAG = 1 compiles with no priorities.
# USER_FLAG = -1 compiles with the max priority minus one (for benchmarking).
USER_FLAG := 0

$(LAST_FILE):
	rm $(LAST_FILE_PREFIX)* -f
	touch $@

run: build
	# Check if sudo is needed to make (USER_FLAG != 1).
	$(if $(filter 1,$(USER_FLAG)), , sudo -E) make -C backend EXTRA_FLAGS='$(BACKEND_FLAGS)' run --quiet INPUT='$(INPUT)'

build: $(TARGET)
	make -C backend build EXTRA_FLAGS='$(BACKEND_FLAGS)'

$(TARGET): $(PIPELINE) $(FILE) $(LAST_FILE)
	cat $(FILE) | xargs -0 python $(PARSER) | ./$(PIPELINE) $(FRONTEND_FLAGS) > $(TEMPFILE) && mv $(TEMPFILE) $(TARGET)

$(TYPE_CHECKER): $(wildcard type-checker/src/*) $(PARSER)
	cargo build --manifest-path $(TYPE_CHECKER_MANIFEST) --release
	touch $@

$(LOWERER): $(wildcard lowering/src/*) $(TYPE_CHECKER)
	cargo build --manifest-path $(LOWERER_MANIFEST) --release
	touch $@

$(TRANSLATOR): $(wildcard translation/src/*) $(LOWERER)
	cargo build --manifest-path $(TRANSLATOR_MANIFEST) --release
	touch $@

$(EMITTER): $(wildcard emission/src/*) $(TRANSLATOR)
	cargo build --manifest-path $(EMITTER_MANIFEST) --release
	touch $@

$(OPTIMIZER): $(wildcard optimization/src/*) $(LOWERER)
	cargo build --manifest-path $(OPTIMIZER_MANIFEST) --release
	touch $@

$(PIPELINE): $(wildcard pipeline/src/*) $(EMITTER) $(OPTIMIZER)
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
	# Build stages are tested in order.
	pytest parsing -vv
	cargo test --manifest-path $(TYPE_CHECKER_MANIFEST) -vv --lib
	cargo test --manifest-path $(LOWERER_MANIFEST) -vv --lib
	cargo test --manifest-path $(TRANSLATOR_MANIFEST) -vv --lib
	cargo test --manifest-path $(EMITTER_MANIFEST) -vv --lib
	cargo test --manifest-path $(OPTIMIZER_MANIFEST) -vv --lib
	cargo test --manifest-path $(PIPELINE_MANIFEST) -vv
	make -C backend bin/test
	./backend/bin/test --gtest_repeat=10 --gtest_shuffle --gtest_random_seed=10 --gtest_brief=0 --gtest_print_time=1
	# Build all samples.
	for sample in samples/*; do \
		if [ "$$sample" != "samples/grammar.apfl" ]; then \
			make build FILE=$$sample || exit 1; \
		fi \
	done;
	# Build all benchmark programs and run 10 times with the smallest input.
	for sample in benchmark/**; do \
		make build FILE=$$sample/main.apfl USER_FLAG=1 || exit 1; \
		for i in `seq 1 10`; do \
			cat $$sample/input.txt | head -1 | xargs ./$(BACKEND) || exit 1; \
		done; \
	done;
	# Run any script tests.
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
	# Run each program repeatedly with all inputs, writing timing information into the log file.
	for program in benchmark/**; do \
		for i in `seq 1 $(REPEATS)`; do \
			make build FILE=$$program/main.apfl USER_FLAG=-1; \
			while read input; do \
				echo $$program $$input; \
				make time --silent FILE=$$program/main.apfl USER_FLAG=-1 INPUT="$$input" \
				| xargs printf '%s\t' \
					`echo $$program | sed 's/benchmark\///'| sed 's/\///g'` \
					`echo $$input | xargs printf '%s,' | sed 's/,$$//'` \
				| xargs -0 echo >> $(LOG_FILE); \
			done < $$program/input.txt; \
		done; \
	done;

python_benchmark: | $(LOG_DIR)
	echo "python benchmark" > $(LOG_DIR)/title.txt

	# Run the python programs repeatedly with all inputs, writing timing information into the log file.
	echo "name\targs\tduration" > $(LOG_DIR)/log.tsv
	for i in `seq 1 $(REPEATS)`; do \
		for program in benchmark/**; do \
			while read input; do \
				echo $$program $$input; \
				export PYTHON=`which python`; \
				sudo -E chrt -f $(MAX_PRIORITY) timeout $(LIMIT) chrt -f 1 sudo -E $$PYTHON scripts/benchmark.py $$program/main.py "$$input" \
				| xargs printf '%s\t' \
					`echo $$program | sed 's/benchmark\///'| sed 's/\///g'` \
					`echo $$input | xargs printf '%s,' | sed 's/,$$//'` \
				| xargs -0 echo >> $(LOG_FILE); \
			done < $$program/input.txt; \
		done; \
	done;


$(VECTOR_FILE): | $(LOG_DIR)
	make $(TARGET) FRONTEND_FLAGS="--export-vector-file $(TEMPFILE)"
	# Copy the header of the vector file.
	head -1 $(TEMPFILE) | sed 's/$$/\ttime/' | sed 's/^/sample\t/' > $@

timings: $(VECTOR_FILE)
	# Build and run the timing files multiple times with multiple inputs, writing times and vectors into the log file.
	for program in timing/**; do \
		for i in `seq 1 $(REPEATS)`; do \
			export program_name=`echo $$program | sed 's/.*\///'`; \
			make build FILE=$$program/main.apfl FRONTEND_FLAGS="--export-vector-file $(TEMPFILE)"; \
			for input in `seq 0 64`; do \
				echo $$program $$input; \
				make time --silent FILE=$$program/main.apfl INPUT="$$input" LIMIT=0 FRONTEND_FLAGS="--export-vector-file $(TEMPFILE)" \
				| sed "s/^/$$program_name\t`tail -1 $(TEMPFILE)`\t/" \
				>> $(VECTOR_FILE); \
			done; \
		done; \
	done;

LIMIT := 60
time: build
	# Run with a higher priority timeout if there is a limit.
	if [ "$(LIMIT)" = "0" ]; then \
		sudo -E ./$(BACKEND) $(INPUT) 2>&1 > /dev/null; \
	else \
		sudo -E chrt -f $(MAX_PRIORITY) timeout $(LIMIT) chrt -f 1 ./$(BACKEND) $(INPUT) 2>&1 > /dev/null; \
	fi \
	| { if read -r output; then echo "$$output"; else echo; fi; } \
	| grep -E '$(PATTERN)' \
	| sed -E 's/$(PATTERN)/\1/' \
	| grep . \
	|| echo nan
	# Parse the error message to determine the time or display nan if there is no output.

.PHONY: all benchmark build clean run time timings
