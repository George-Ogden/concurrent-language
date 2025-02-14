CXX := g++
CXXFLAGS := -Wall -pedantic -std=c++20 -Wfatal-errors
OPTIMIZATION := -Ofast
LIBS := -lpthread
INCLUDE := -Iinclude

BUILD_DIR := build_
WRAP_DIR := wrap
WRAP_FILE := $(BUILD_DIR)/wrap.hpp
WRAP_FILES := $(wildcard $(WRAP_DIR)/*.hpp)
WRAP := -Wl,--wrap=sysconf,--wrap=pthread_setschedparam,--wrap=pthread_setaffinity_np

TARGET_DIR := bin
SRC_DIR := src
TARGET := $(TARGET_DIR)/main
TARGETS := $(patsubst $(SRC_DIR)/%.cpp, $(TARGET_DIR)/%, $(wildcard $(SRC_DIR)/*.cpp))
HEADERS := $(wildcard include/*.hpp include/**/*.hpp)

ifeq ($(USER_FLAG), 1)
	CXXFLAGS += $(WRAP)
	HEADERS += $(WRAP_FILE)
	INCLUDE += -I.
endif

USER := $(shell user=$$(who -m | awk '{print $$1}'); [ -z "$$user" ] && whoami || echo $$user)
COMPILER_PREFIX := $(BUILD_DIR)/.compiled-with-
COMPILER_FILE := $(COMPILER_PREFIX)$(CXX)$(USER_FLAG)

TEST_DIR := test
TEST_LIBS := -lgtest -lgtest_main
TEST_BUILDDIR := $(BUILD_DIR)/test
USER_FLAGS := -g -fsanitize=address $(WRAP)
TEST_INCLUDE := $(wildcard $(TEST_DIR)/*.hpp) $(WRAP_FILE)
TEST_FILE := $(TEST_BUILDDIR)/test.cpp
TEST_EXECS := $(patsubst $(TEST_DIR)/test_%.hpp, $(TARGET_DIR)/test_%, $(filter $(TEST_DIR)/test_%.hpp, $(TEST_INCLUDE)))
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

include/main/user.hpp: $(WRAP_FILE) $(COMPILER_FILE)
	$(if $(filter 1,$(USER_FLAG)), echo "#include \"$(WRAP_FILE)\"" > $@,)
	touch $@

$(WRAP_FILE): $(WRAP_FILES) | $(BUILD_DIR)
	echo "" > $@; \
	for file in $(WRAP_FILES); do \
		echo "#include \"$$file\"" >> $@; \
	done

$(BUILD_DIR)/%.o: $(SRC_DIR)/%.cpp $(HEADERS) $(COMPILER_FILE) | $(BUILD_DIR)
	$(CXX) $(CXXFLAGS) $(OPTIMIZATION) $(INCLUDE) $(LIBS) -c -o $@ $<

$(TARGET_DIR)/%: $(BUILD_DIR)/%.o | $(TARGET_DIR)
	$(CXX) $^ $(CXXFLAGS) $(LIBS) -o $@ $(OPTIMIZATION)

$(TEST_BUILDDIR)/: $(TEST_INCLUDE) $(HEADERS) | $(TEST_BUILDDIR)
	echo $(TEST_DIR)/%.hpp > $@

$(TEST_FILE): $(TEST_INCLUDE) $(HEADERS) | $(TEST_BUILDDIR)
	echo "" > $@; \
	for file in $(TEST_INCLUDE); do \
		echo "#include \"$$file\"" >> $@; \
	done

$(TEST_BUILDDIR)/test_%.cpp: $(TEST_INCLUDE) $(HEADERS) | $(TEST_BUILDDIR)
	echo "" > $@; \
	for file in $(TEST_DIR)/test_$*.hpp $(WRAP_FILE); do \
		echo "#include \"$$file\"" >> $@; \
	done

$(TEST_EXEC)_%: $(TEST_BUILDDIR)/test_%.cpp | $(TARGET_DIR)
	$(CXX) $^ $(CXXFLAGS) $(USER_FLAGS) -I. $(INCLUDE) $(LIBS) -o $@ $(TEST_LIBS)

$(TEST_EXEC): $(TEST_FILE) | $(TARGET_DIR)
	$(CXX) $^ $(CXXFLAGS) $(USER_FLAGS) -I. $(INCLUDE) $(LIBS) -o $@ $(TEST_LIBS)

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
	ASAN_OPTIONS=detect_stack_use_after_return=1:detect_leaks=0 ./$<

.PHONY: all build clean default run

.PRECIOUS: $(TEST_BUILDDIR)/test_%.cpp
