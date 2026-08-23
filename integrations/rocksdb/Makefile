# Makefile for Expanse RocksDB MemTable Integration

CXX ?= clang++
CXXFLAGS ?= -std=c++20 -O3 -Wall -Wextra -pthread

ROOT_DIR := ../..
INCLUDE_DIRS := -I./include -I$(ROOT_DIR)/crates/expanse-capi/include -I$(ROOT_DIR)/include
LIB_DIR := $(ROOT_DIR)/target/debug

# Detect OS for dynamic library extension
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
    LIBEXPANSE := $(LIB_DIR)/libexpanse.dylib
else
    LIBEXPANSE := $(LIB_DIR)/libexpanse.so
endif

# Fallback to static lib if dynamic not found
ifeq ($(wildcard $(LIBEXPANSE)),)
    LIBEXPANSE := $(LIB_DIR)/libexpanse.a
endif

BUILD_DIR := build
TARGET_LIB := $(BUILD_DIR)/librocksdb_expanse.a
TEST_BIN := $(BUILD_DIR)/test_expanse_memtable
BENCH_BIN := $(BUILD_DIR)/bench_memtable

.PHONY: all test bench clean build_cargo

all: $(TARGET_LIB) $(TEST_BIN) $(BENCH_BIN)

build_cargo:
	@cd $(ROOT_DIR) && cargo build -p expanse-capi

$(BUILD_DIR):
	@mkdir -p $(BUILD_DIR)

$(BUILD_DIR)/expanse_memtable.o: src/expanse_memtable.cc include/expanse_memtable.h | $(BUILD_DIR)
	$(CXX) $(CXXFLAGS) $(INCLUDE_DIRS) -c $< -o $@

$(TARGET_LIB): $(BUILD_DIR)/expanse_memtable.o
	ar rcs $@ $^

$(TEST_BIN): tests/test_expanse_memtable.cc $(TARGET_LIB) | $(BUILD_DIR)
	$(CXX) $(CXXFLAGS) $(INCLUDE_DIRS) $< -L$(BUILD_DIR) -lrocksdb_expanse $(LIBEXPANSE) -o $@

$(BENCH_BIN): benches/bench_memtable.cc $(TARGET_LIB) | $(BUILD_DIR)
	$(CXX) $(CXXFLAGS) $(INCLUDE_DIRS) $< -L$(BUILD_DIR) -lrocksdb_expanse $(LIBEXPANSE) -o $@

test: $(TEST_BIN)
	@echo "Running Expanse RocksDB MemTable tests..."
	@$(TEST_BIN)

bench: $(BENCH_BIN)
	@echo "Running Expanse RocksDB MemTable microbenchmarks..."
	@$(BENCH_BIN)

clean:
	rm -rf $(BUILD_DIR)
