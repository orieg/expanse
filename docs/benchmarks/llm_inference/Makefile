CXX ?= clang++
REPO_ROOT ?= $(shell cd ../../.. && pwd)
CXXFLAGS ?= -std=c++20 -O3 -Wall -Wextra -I$(REPO_ROOT)/include
ALL_LDFLAGS = $(LDFLAGS) -L$(REPO_ROOT)/target/release -lexpanse -Wl,-rpath,$(REPO_ROOT)/target/release

TARGET = benches/bench_llama_lookup
SRCS = benches/bench_llama_lookup.cpp

all: $(TARGET)

$(TARGET): $(SRCS)
	$(CXX) $(CXXFLAGS) -o $@ $^ $(ALL_LDFLAGS)

clean:
	rm -f $(TARGET)

.PHONY: all clean
