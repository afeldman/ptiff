# Makefile for the ptiff-julia binding.
#
# Julia needs no C/C++ adapter (unlike the Octave MEX binding): the Ptiff.jl
# module calls the C ABI (libptiff_c) directly via `ccall`. The only thing to
# "build" is the Rust shared library that the module dlopens at runtime.
#
#   Julia -> Ptiff.jl (ccall) -> C ABI (libptiff_c) -> Rust core
#
# Dependencies:
#   * Julia >= 1.6 (the `julia` executable on PATH)
#   * libptiff_c built into the Rust workspace `target/release/`
#     (cargo build -p ptiff-c --release), and `target/ptiff_c.h`
#
# Override PTIFF_C_LIB_DIR to point libptiff_c resolution elsewhere, or set
# the same env var at runtime (Ptiff.libpath honours it).

JULIA ?= julia
JULIAFLAGS ?=

# Paths into the Rust workspace `target/` (resolved from bindings/julia/ to
# the workspace root). Note: the Makefile lives in bindings/julia/ (not a
# deeper subdir), so it needs exactly two `..` to reach the workspace root.
C_ABI_DIR := ../../target
C_LIB_DIR := ../../target/release

.PHONY: libptiff_c
libptiff_c:
	@test -f $(C_LIB_DIR)/libptiff_c.dylib -o -f $(C_LIB_DIR)/libptiff_c.so -o -f $(C_LIB_DIR)/libptiff_c.a || \
		(cd ../.. && cargo build -p ptiff-c --release)

# Rebuild the generated header if the Rust ABI changed (informational; the
# Julia side only talks to the shared lib, but this keeps the source of truth
# fresh for anyone diffing against the header).
.PHONY: header
header:
	cd ../.. && cargo build -p ptiff-c

.PHONY: build
build: libptiff_c
	@echo "Ptiff.jl needs no compilation; relies on $(C_LIB_DIR)/libptiff_c."

.PHONY: test
test: libptiff_c
	$(JULIA) $(JULIAFLAGS) --project=$(CURDIR) $(CURDIR)/test/runtests.jl

.PHONY: clean
clean:
	@echo "Ptiff.jl keeps no build artifacts (C ABI lives in target/release/)."
