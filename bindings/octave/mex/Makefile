# Makefile for the Phase-10 ptiff-octave MEX adapter.
#
# Builds the hand-written C++ .oct (bindings/octave/mex/ptiff_octave.cpp) that
# calls the C ABI (libptiff_c) DIRECTLY -- no SWIG. The .m layer in this dir is
# the idiomatic Octave API over the single `ptiff_octave` entry point.
#
#   Octave -> ptiff_octave.oct (native C++ .oct) -> C ABI -> Rust core
#
# Dependencies:
#   * libptiff_c built into the Rust workspace `target/` (cargo build -p ptiff-c)
#   * the generated header `target/ptiff_c.h` (cbindgen, source of truth)
#   * GNU Octave + mkoctfile (== MEX-style C++ shell for the plan's §10.3)
#
# Link model: dynamic against libptiff_c.{dylib,so}. An rpath is embedded so no
# DYLD_LIBRARY_PATH/LD_LIBRARY_PATH is needed at runtime (same as the old SWIG
# .oct). Override PTIFF_C_LIB_DIR to point at a different lib directory.

MKOCTFILE ?= mkoctfile
OCTAVE    ?= octave

# Paths into the Rust workspace `target/` (resolved from bindings/octave/mex/).
C_ABI_DIR := ../../../target
C_LIB_DIR := ../../../target/release

# ---- Resolve libptiff_c dir ----
ifeq ($(PTIFF_C_LIB_DIR),)
	LIB_DIR := $(CURDIR)/$(C_LIB_DIR)
	LIBC_CFLAGS := -I$(CURDIR)/$(C_ABI_DIR)
else
	LIB_DIR := $(if $(filter /%,$(PTIFF_C_LIB_DIR)),$(PTIFF_C_LIB_DIR),$(CURDIR)/$(PTIFF_C_LIB_DIR))
	LIBC_CFLAGS := -I$(C_ABI_DIR)
endif

OC_LDIR := -L$(LIB_DIR)
OC_LIB  := -lptiff_c

# The Rust shared lib must exist before linking.
.PHONY: libptiff_c
libptiff_c:
	@test -f $(C_LIB_DIR)/libptiff_c.dylib -o -f $(C_LIB_DIR)/libptiff_c.so -o -f $(C_LIB_DIR)/libptiff_c.a || \
		(cd ../../.. && cargo build -p ptiff-c --release)

# Rebuild the generated header if the Rust ABI changed.
$(C_ABI_DIR)/ptiff_c.h:
	cd ../../.. && cargo build -p ptiff-c

%.oct: %.cpp $(C_ABI_DIR)/ptiff_c.h libptiff_c
	$(MKOCTFILE) $< $(LIBC_CFLAGS) $(OC_LDIR) $(OC_LIB) -Wl,-rpath,$(LIB_DIR) -o $@

build: ptiff_octave.oct

.PHONY: test
test:
	$(OCTAVE) --quiet --no-gui --eval "addpath('$(CURDIR)'); run('$(CURDIR)/run_ptiff_mex_tests.m');"

.PHONY: clean
clean:
	rm -f ptiff_octave.oct

.PHONY: all
all: build

.PHONY: help
help:
	@echo "make build   - build ptiff_octave.oct against target/release/libptiff_c"
	@echo "make test    - run test_ptiff_mex.m (needs the .oct built)"
	@echo "make clean   - remove ptiff_octave.oct"
