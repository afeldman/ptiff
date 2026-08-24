# bindings/cpp: ptiff-cpp — modern C++ wrapper over the PTIFF C ABI.
#
# The wrapper (include/ptiff/) is header-only over libptiff_c; this Makefile
# compiles the round-trip + API-parity test and links against the Rust-built
# C ABI (crates/ptiff-c -> target/release/libptiff_c), the same shared object
# the Go/Python/Ruby/Octave bindings link. It never compiles any C++ from the
# (deleted) C++ reference; the C++ type names are reproduced over the C ABI.
#
#   make test   build + run the round-trip/API-parity test
#   make run    run the already-built test binary
#   make clean  remove the built binary + runtime temp files

CXX      ?= c++
CXXFLAGS ?= -O2 -std=c++23 -Wall -Wextra -Iinclude -I../../target

# Resolve the Rust-built libptiff_c in the workspace release dir (as the SWIG
# bindings do). Override PTIFF_C_LIB_DIR to point elsewhere.
# bindings/cpp -> bindings -> <workspace-root>; target sits at <root>/target.
PTIFF_C_LIB_DIR ?= ../../target/release
LIBDIR := $(realpath $(PTIFF_C_LIB_DIR))

BIN := ptiff_cpp_test
SRC := tests/test_ptiff_cpp.cpp

UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
RPATH := -Wl,-rpath,$(LIBDIR)
else
RPATH := -Wl,-rpath,$(LIBDIR)
endif

.PHONY: all test run clean staticlib

all: build

# Ensure the Rust C ABI static/shared lib exists before linking.
staticlib:
	@test -f $(LIBDIR)/libptiff_c.dylib -o -f $(LIBDIR)/libptiff_c.so -o -f $(LIBDIR)/libptiff_c.a || \
		(cd ../.. && cargo build -p ptiff-c --release)

$(BIN): $(SRC) $(wildcard include/ptiff/*.hpp include/ptiff/detail/*.hpp) Makefile
	$(CXX) $(CXXFLAGS) $(SRC) -L$(LIBDIR) -lptiff_c $(RPATH) -o $@

build: staticlib $(BIN)

test: build
	./$(BIN)

run: $(BIN)
	./$(BIN)

clean:
	rm -f $(BIN) ./ptiff_cpp_roundtrip.tif
