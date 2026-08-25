# Multi-language SWIG build for the libptiff C ABI (bindings/swig).
#
# One ptiff.i drives TWO SWIG targets (Go + Ruby) with per-language
# buffer/out-param typemaps (typemaps.i, guarded by SWIGGO / SWIGRUBY). Output
# goes into per-language subdirs so the generated `ptiff_wrap.c` files don't
# collide. The Python binding is no longer built from SWIG (PyO3, crates/
# ptiff-python), and the Octave binding is now the hand-written MEX adapter
# (bindings/octave/mex/).
#
# Targets:
#   make            build both (go + ruby) -- the Octave binding is now the
#                   hand-written MEX adapter (bindings/octave/mex/), not SWIG
#   make go         scaffold the Go module (go/ptiff.go, ptiff_wrap.c, go.mod,
#                   cgo_flags.go) ready for `go test ./...` in go/
#   make ruby       build the Ruby extension (ruby/ptiff.$(DLEXT))
#   make test       build then run both round-trip (and feature) tests
#   make clean      remove generated outputs
#
# The C ABI now comes from the Rust core: cbindgen (crates/ptiff-c/build.rs)
# emits the single header `target/ptiff_c.h`, and Cargo builds libptiff_c into
# `target/release/`. This SWIG makefile therefore links straight against the
# Rust-built lib by default. Override with PTIFF_C_LIB_DIR to point at a
# different directory containing libptiff_c.{dylib,so,a}.

RUBY      ?= ruby
GO        ?= go
SWIG      ?= swig
CC        ?= cc

# Paths into the Rust workspace `target/` (both resolved from bindings/swig/).
C_ABI_DIR := ../../target
GEN_DIR := real_inc
C_LIB_DIR := ../../target/release

# Per-language output dirs. GO_DIR and RB_DIR are PROMOTED, official
# locations (see the "alles in SWIG" migration notes in README.md). Ruby's
# binding is bindings/ruby/lib/ (Ruby convention: `require "ptiff"` resolves
# the platform-native shared extension (`ptiff.bundle` on macOS, `ptiff.so` on
# Linux) off $LOAD_PATH/RUBYLIB, same as any other lib/ file) -- SWIG -ruby
# emits only ptiff_wrap.c plus that compiled extension, no separate .rb stub,
# so lib/ptiff/version.rb (hand-authored, gem version metadata only) is the
# sole survivor of the old Fiddle wrapper.
GO_DIR  := ../go/ptiff
RB_DIR  := ../ruby/lib
GO_MODULE := github.com/crater/ptiff-go

PKGCONF := $(shell command -v pkg-config)

# -undefined dynamic_lookup is a macOS/clang linker flag that allows shared
# extensions to leave Python/Ruby symbols undefined at link time (resolved
# at load time by the interpreter). GNU ld on Linux does not support it and
# treats it as a forced-undefined symbol reference, causing a link error.
# On Linux, -shared already permits undefined symbols by default.
UNDEF_FLAG := $(if $(filter Darwin,$(shell uname)),-undefined dynamic_lookup,)

# ---- Resolve libptiff_c: prefer pkg-config, else PTIFF_C_LIB_DIR ----
ifeq ($(PTIFF_C_LIB_DIR),)
	# Default: the Rust-built lib in the workspace release dir.
	LIB_DIR := $(CURDIR)/$(C_LIB_DIR)
	LIBC_LDFLAGS := -L$(LIB_DIR) -lptiff_c -Wl,-rpath,$(LIB_DIR)
	LIBC_CFLAGS  := -I$(CURDIR)/$(C_ABI_DIR)
	LIBDIR       := $(LIB_DIR)
else
	# A relative PTIFF_C_LIB_DIR resolves against the build dir.
	LIB_DIR := $(if $(filter /%,$(PTIFF_C_LIB_DIR)),$(PTIFF_C_LIB_DIR),$(CURDIR)/$(PTIFF_C_LIB_DIR))
	LIBC_LDFLAGS := -L$(LIB_DIR) -lptiff_c -Wl,-rpath,$(LIB_DIR)
	LIBC_CFLAGS  := -I$(C_ABI_DIR)
	LIBDIR       := $(LIB_DIR)
endif

RBHDR1 := $(shell $(RUBY) -rrbconfig -e 'print RbConfig::CONFIG["rubyhdrdir"]')
RBHDR2 := $(shell $(RUBY) -rrbconfig -e 'print RbConfig::CONFIG["rubyarchhdrdir"]')
RBDLEXT := $(shell $(RUBY) -rrbconfig -e 'print RbConfig::CONFIG["DLEXT"]')

# ---- Platform strings for OS-portable linker flags ----
# `-undefined dynamic_lookup` lets a Python/Ruby extension resolve its host
# interpreter's symbols at runtime on macOS (Mach-O). GNU ld (Linux) has no
# such switch -- it would be parsed as a stray file name -- and the interpreter
# symbols are already exported by the loaded interpreter there, so the flag is
# simply omitted on every non-Darwin platform.
UNAME_S := $(shell uname -s)
UNDEFINED_SYMBOLS := $(if $(filter Darwin,$(UNAME_S)),-undefined dynamic_lookup,)

# ---- Shim include dir: SWIG-parseable copy of target/ptiff_c.h ----
# SWIG chokes on the PTIFF_C_API block (visibility attribute etc.), so we ship
# SWIG a filtered copy where that block is dropped (PTIFF_C_API => empty). The
# %{...%} C block in ptiff.i is compiled against the REAL target/ptiff_c.h via
# LIBC_CFLAGS above.
.PHONY: staticlib
$(GEN_DIR):
	mkdir -p $(GEN_DIR)
	@test -f $(C_ABI_DIR)/ptiff_c.h || (echo "ptiff_c.h fehlt -- baue Rust-core (cargo build -p ptiff-c --release)"; cargo build -p ptiff-c --release)
	sed -e '/^#if defined(_WIN32)/,/^$$/d' $(C_ABI_DIR)/ptiff_c.h > $(GEN_DIR)/ptiff_c.h

# The Rust static/shared lib must exist before linking. `staticlib` builds it.
staticlib:
	@test -f $(C_LIB_DIR)/libptiff_c.dylib -o -f $(C_LIB_DIR)/libptiff_c.so -o -f $(C_LIB_DIR)/libptiff_c.a || \
		(cd ../.. && cargo build -p ptiff-c --release)

# ============================================================
# Go
# ============================================================
# GO_DIR (bindings/go/ptiff) is the PROMOTED, official Go package dir -- SWIG
# is the primary Go binding layer (see the "alles in SWIG" migration notes in
# bindings/swig/README.md). Its Go MODULE ROOT (bindings/go/, one level up)
# is intentionally separate: go.mod lives there, hand-authored and tracked
# (module github.com/crater/ptiff-go), NOT generated -- GO_MODROOT/go.mod
# below only ever fires if that file is somehow missing.
GO_MODROOT := $(dir $(GO_DIR:%/=%))

$(GO_DIR)/ptiff.go $(GO_DIR)/ptiff_wrap.c: ptiff.i typemaps.i $(GEN_DIR)
	$(SWIG) -go -cgo -outdir $(GO_DIR) -o $(GO_DIR)/ptiff_wrap.c -I$(GEN_DIR) ptiff.i

$(GO_MODROOT)go.mod:
	printf 'module %s\n\ngo 1.21\n' '$(GO_MODULE)' > $@

# cgo_flags.go injects #cgo directives so `go build`/`go test` in GO_DIR can
# find the real header + link the Rust-built libptiff_c. Both -I (header dir)
# and -L/-l/-rpath (lib dir) are baked in (no pkg-config needed).
$(GO_DIR)/cgo_flags.go: $(GO_MODROOT)go.mod
	printf 'package ptiff\n\n/*\n' > $@
	printf '#cgo CFLAGS: -I%s\n' '$(CURDIR)/$(C_ABI_DIR)' >> $@
	printf '#cgo LDFLAGS: -L%s -lptiff_c -Wl,-rpath,%s\n*/\nimport "C"\n' '$(LIBDIR)' '$(LIBDIR)' >> $@

go: $(GO_DIR)/ptiff.go $(GO_DIR)/ptiff_wrap.c $(GO_MODROOT)go.mod $(GO_DIR)/cgo_flags.go staticlib
	cd $(GO_DIR) && $(GO) build ./...

# ============================================================
# Ruby
# ============================================================
$(RB_DIR)/ptiff_wrap.c: ptiff.i typemaps.i $(GEN_DIR)
	$(SWIG) -ruby -outdir $(RB_DIR) -o $(RB_DIR)/ptiff_wrap.c -I$(GEN_DIR) ptiff.i

$(RB_DIR)/ptiff.$(RBDLEXT): $(RB_DIR)/ptiff_wrap.c
	$(CC) -O2 -fPIC -I$(RBHDR1) -I$(RBHDR2) $(LIBC_CFLAGS) -shared $(UNDEFINED_SYMBOLS) \
		$(RB_DIR)/ptiff_wrap.c -o $@ $(LIBC_LDFLAGS)

ruby: $(RB_DIR)/ptiff.$(RBDLEXT)

# ============================================================
# Aggregate
# ============================================================
all: go ruby

# Ruby's promoted tests live in bindings/ruby/test/, separate from the
# promoted package dir (RB_DIR) -- RUBYLIB points the test run at the
# generated module.
RB_TEST_DIR := ../ruby/test

test: all
	cd $(GO_DIR) && $(GO) test ./...
	cd $(RB_TEST_DIR) && RUBYLIB=$(abspath $(RB_DIR)) $(RUBY) test_roundtrip.rb
	cd $(RB_TEST_DIR) && RUBYLIB=$(abspath $(RB_DIR)) $(RUBY) -e 'Dir.glob("test_{version,logger,backend,error,image,sink}.rb").each { |f| require_relative f }'

clean:
	rm -rf $(GEN_DIR)
	rm -rf $(GO_DIR)/ptiff.go $(GO_DIR)/ptiff_wrap.c $(GO_DIR)/cgo_flags.go $(GO_DIR)/roundtrip_go.tif
	rm -rf $(GO_MODROOT)go.sum
	rm -rf $(RB_DIR)/ptiff_wrap.c $(RB_DIR)/ptiff.bundle $(RB_DIR)/ptiff.so $(RB_TEST_DIR)/roundtrip_ruby.tif

.PHONY: all go ruby test clean staticlib
