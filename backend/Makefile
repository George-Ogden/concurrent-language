CXX := g++
EXTRA_FLAGS :=
CXXFLAGS := -Wall -pedantic -std=c++20 -Wfatal-errors $(EXTRA_FLAGS)
OPTIMIZATION := -Ofast
LIBS :=
INCLUDE := -Iinclude

BUILD_DIR := build_
WRAP_DIR := wrap
USER_FILE := include/main/user.hpp
WRAP := -Wl,--wrap=pthread_setschedparam,--wrap=pthread_setaffinity_np

TARGET_DIR := bin
SRC_DIR := src
TARGET := $(TARGET_DIR)/main
TARGETS := $(patsubst $(SRC_DIR)/%.cpp, $(TARGET_DIR)/%, $(wildcard $(SRC_DIR)/*.cpp))
HEADERS := $(wildcard include/*.hpp include/**/*.hpp include/**/*.tpp) $(USER_FILE)

WRAP_USER_FILE := wrap/user.hpp
WRAP_BENCHMARK_FILE := wrap/benchmark.hpp
# Use different wrappers for user file and benchmarking.
ifeq ($(USER_FLAG), 1)
	WRAP_FILE := $(WRAP_USER_FILE)
else ifeq ($(USER_FLAG), -1)
	WRAP_FILE := $(WRAP_BENCHMARK_FILE)
endif

ifneq ($(filter $(USER_FLAG),1 -1),)
	CXXFLAGS += $(WRAP)
	HEADERS += $(WRAP_FILE)
	INCLUDE += -I.
endif

# Determine current user.
USER := $(shell user=$$(who -m | awk '{print $$1}'); [ -z "$$user" ] && whoami || echo $$user)
EXTRA_FLAGS_HASH := $(shell echo '$(EXTRA_FLAGS)' | sha256sum 2> /dev/null | cut -d ' ' -f1)
COMPILER_PREFIX := $(BUILD_DIR)/.compiled-with-
# Generate a filename based on the flags that forces rebuild when changed.
COMPILER_FILE := $(COMPILER_PREFIX)$(CXX)$(USER_FLAG)$(EXTRA_FLAGS_HASH)

TEST_DIR := test
TEST_LIBS := -lgtest -lgtest_main
TEST_BUILDDIR := $(BUILD_DIR)/test
TEST_FLAGS := -g -fsanitize=address $(WRAP) -DFN_CACHING
TEST_INCLUDE := $(wildcard $(TEST_DIR)/*.hpp) $(WRAP_USER_FILE)
TEST_FILE := $(TEST_BUILDDIR)/test.cpp
TEST_EXECS := $(patsubst $(TEST_DIR)/test_%.hpp, $(TARGET_DIR)/test_%, $(filter $(TEST_DIR)/test_%.hpp, $(TEST_INCLUDE)))
TEST_EXEC := $(TARGET_DIR)/test

# Include Makefile in dependencies.
.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

default: $(TARGET)

all: $(TARGETS) $(TEST_EXEC) $(TEST_EXECS)

$(TEST_BUILDDIR) $(BUILD_DIR) $(TARGET_DIR):
	# Create directories then change permissions if run with sudo.
	if [ `whoami` = "$(USER)" ]; \
	then \
		mkdir -p $@; \
	else \
		FULLDIR=""; \
		for DIR in $(subst /, ,$@); do \
			FULLDIR="$$FULLDIR$$DIR/"; \
			install -d -m 0755 -o $(USER) -g $(USER) $$FULLDIR; \
		done; \
	fi

$(USER_FILE): $(WRAP_FILE) $(COMPILER_FILE)
	# Include the wrap user file if `USER_FLAG` is set.
	$(if $(filter $(USER_FLAG),1 -1), echo "#include \"$(WRAP_FILE)\"" > $@, echo "" > $@)

$(BUILD_DIR)/%.o: $(SRC_DIR)/%.cpp $(HEADERS) $(COMPILER_FILE) | $(BUILD_DIR)
	$(CXX) $(CXXFLAGS) $(OPTIMIZATION) $(INCLUDE) $(LIBS) -c -o $@ $<

$(TARGET_DIR)/%: $(BUILD_DIR)/%.o | $(TARGET_DIR)
	$(CXX) $^ $(CXXFLAGS) $(LIBS) -o $@ $(OPTIMIZATION)

$(TEST_BUILDDIR)/: $(TEST_INCLUDE) $(HEADERS) | $(TEST_BUILDDIR)
	echo $(TEST_DIR)/%.hpp > $@

$(TEST_FILE): $(TEST_INCLUDE) $(HEADERS) | $(TEST_BUILDDIR)
	# Include all test files.
	echo "" > $@; \
	for file in $(TEST_INCLUDE); do \
		echo "#include \"$$file\"" >> $@; \
	done

$(TEST_BUILDDIR)/test_%.cpp: $(TEST_INCLUDE) $(HEADERS) | $(TEST_BUILDDIR)
	# Only include selected test file when testing a single file.
	echo "" > $@; \
	for file in $(TEST_DIR)/test_$*.hpp $(WRAP_USER_FILE); do \
		echo "#include \"$$file\"" >> $@; \
	done

$(TEST_EXEC)_%: $(TEST_BUILDDIR)/test_%.cpp | $(TARGET_DIR)
	$(CXX) $^ $(CXXFLAGS) $(TEST_FLAGS) -I. $(INCLUDE) $(LIBS) -o $@ $(TEST_LIBS)

$(TEST_EXEC): $(TEST_FILE) | $(TARGET_DIR)
	$(CXX) $^ $(CXXFLAGS) $(TEST_FLAGS) -I. $(INCLUDE) $(LIBS) -o $@ $(TEST_LIBS)

clean:
	rm -rf $(TARGET) $(BUILD_DIR) $(TARGET_DIR)

empty:
	find . -not -path '*/.*' -type d -empty -delete

$(COMPILER_FILE): | $(BUILD_DIR)
	rm -f $(COMPILER_PREFIX)*
	touch $@

build: $(TARGET)

run: build
	./$(TARGET) $(INPUT)

%: $(TARGET_DIR)/%
	./$<

.PHONY: all build clean default run
