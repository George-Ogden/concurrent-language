CXX := g++
CXXFLAGS := -Wall -pedantic -std=c++20 -Wfatal-errors
OPTIMIZATION := -Ofast
LIBS := -lpthread
INCLUDE := -Iinclude

TARGET_DIR := bin
SRC_DIR := src
TARGET := $(TARGET_DIR)/main
TARGETS := $(patsubst $(SRC_DIR)/%.cpp, $(TARGET_DIR)/%, $(wildcard $(SRC_DIR)/*.cpp))
BUILD_DIR := build_
HEADERS := $(wildcard include/*.hpp include/**/*.hpp)

USER := $(shell user=$$(who -m | awk '{print $$1}'); [ -z "$$user" ] && whoami || echo $$user)
COMPILER_PREFIX := $(BUILD_DIR)/.compiled-with-
COMPILER_FILE := $(COMPILER_PREFIX)$(CXX)

TESTDIR := test
TEST_LIBS := -lgtest -lgtest_main
TEST_BUILDDIR := $(BUILD_DIR)/test
TEST_FLAGS := -g -fsanitize=address -Wl,--wrap=sysconf,--wrap=pthread_setschedparam,--wrap=pthread_setaffinity_np
TEST_INCLUDE := $(wildcard $(TESTDIR)/*.hpp)
TEST_FILE := $(TEST_BUILDDIR)/test.cpp
TEST_EXECS := $(patsubst $(TESTDIR)/test_%.hpp, $(TARGET_DIR)/test_%, $(filter $(TESTDIR)/test_%.hpp, $(TEST_INCLUDE)))
TEST_EXEC := $(TARGET_DIR)/test

.EXTRA_PREREQS := $(abspath $(lastword $(MAKEFILE_LIST)))

default: $(TARGET)

all: $(TARGETS) $(TEST_EXEC) $(TEST_EXECS)

$(TEST_BUILDDIR) $(BUILD_DIR) $(TARGET_DIR):
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

$(BUILD_DIR)/%.o: $(SRC_DIR)/%.cpp $(HEADERS) $(COMPILER_FILE) | $(BUILD_DIR)
	$(CXX) $(CXXFLAGS) $(OPTIMIZATION) $(INCLUDE) $(LIBS) -c -o $@ $<

$(TARGET_DIR)/%: $(BUILD_DIR)/%.o | $(TARGET_DIR)
	$(CXX) $^ $(CXXFLAGS) $(LIBS) -o $@ $(OPTIMIZATION)

$(TEST_BUILDDIR)/: $(TEST_INCLUDE) $(HEADERS) | $(TEST_BUILDDIR)
	echo $(TESTDIR)/%.hpp > $@

$(TEST_FILE): $(TEST_INCLUDE) $(HEADERS) | $(TEST_BUILDDIR)
	echo "" > $@; \
	for file in $(TEST_INCLUDE); do \
		echo "#include \"$$file\"" >> $@; \
	done

$(TEST_BUILDDIR)/test_%.cpp: $(TEST_INCLUDE) $(HEADERS) | $(TEST_BUILDDIR)
	echo "" > $@; \
	for file in $(TESTDIR)/test_$*.hpp $(filter-out $(TESTDIR)/test_%.hpp,$(TEST_INCLUDE))	; do \
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
	./$(TARGET) $(FLAGS)

%: $(TARGET_DIR)/%
	ASAN_OPTIONS=detect_leaks=0 ./$<

.PHONY: all build clean default run
