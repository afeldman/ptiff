# Multi-language SWIG build for the libptiff C ABI (bindings/swig).
#
# One ptiff.i drives FOUR SWIG targets with per-language buffer/out-param
# typemaps (typemaps.i, guarded by SWIGPYTHON / SWIGGO / SWIGRUBY /
# SWIGOCTAVE). Output goes into per-language subdirs so the generated
# `ptiff_wrap.c` files don't collide.
#
# Targets:
#   make            build all four (python + go + ruby + octave)
#   make python     build the Python extension (python/_ptiff.so)
#   make go         scaffold the Go module (go/ptiff.go, ptiff_wrap.c, go.mod,
#                   cgo_flags.go) ready for `go test ./...` in go/
#   make ruby       build the Ruby extension (ruby/ptiff.bundle)
#   make octave     build the Octave module (octave/lib/ptiff.oct)
#   make test       build then run all four round-trip (and feature) tests
#   make clean      remove generated outputs
#
# Point PKG_CONFIG_PATH at an installed prefix that ships libptiff_c.pc, e.g.
#   PKG_CONFIG_PATH=/where/ever/lib/pkgconfig make test
# or set PTIFF_C_LIB_DIR to a directory containing libptiff_c.{dylib,so,a}.

PYTHON    ?= python3
RUBY      ?= ruby
GO        ?= go
SWIG      ?= swig
CC        ?= cc
OCTAVE    ?= octave
MKOCTFILE ?= mkoctfile

C_ABI_DIR   := ../c
GEN_DIR     := real_inc
SHIM_EXPORT := swig_include/ptiff_c_export.h

# Per-language output dirs. PY_DIR, GO_DIR and RB_DIR are PROMOTED, official
# locations (see the "alles in SWIG" migration notes in README.md) --
# Python's package root (bindings/python/src/) is flat, matching what
# `swig -python` itself emits (a top-level ptiff.py + _ptiff.so, no package
# subdirectory). Ruby's is bindings/ruby/lib/ (Ruby convention: `require
# "ptiff"` resolves ptiff.bundle off $LOAD_PATH/RUBYLIB, same as any other
# lib/ file) -- SWIG -ruby emits only ptiff_wrap.c/ptiff.bundle, no separate
# .rb stub, so lib/ptiff/version.rb (hand-authored, gem version metadata
# only) is the sole survivor of the old Fiddle wrapper.
PY_DIR  := ../python/src/ptiff
GO_DIR  := ../go/ptiff
RB_DIR  := ../ruby/lib
OC_DIR  := ../octave/lib
GO_MODULE := github.com/crater/ptiff-go

PKGCONF := $(shell command -v pkg-config)

# ---- Resolve libptiff_c: prefer pkg-config, else PTIFF_C_LIB_DIR ----
ifeq ($(PTIFF_C_LIB_DIR),)
	ifneq ($(PKGCONF),)
		LIBC_CFLAGS  := $(shell pkg-config --cflags libptiff_c 2>/dev/null)
		LIBC_LDFLAGS := $(shell pkg-config --libs libptiff_c 2>/dev/null)
		# Embed an rpath so built extensions can load libptiff_c at runtime.
		LIBDIR       := $(shell pkg-config --variable=libdir libptiff_c 2>/dev/null)
		LIBC_LDFLAGS += -Wl,-rpath,$(LIBDIR)
		ifeq ($(LIBC_LDFLAGS),)
			$(error No libptiff_c. Install a prefix and export PKG_CONFIG_PATH=</prefix/lib/pkgconfig>.)
		endif
	endif
else
	# A relative PTIFF_C_LIB_DIR resolves against the build dir.
	LIB_DIR := $(if $(filter /%,$(PTIFF_C_LIB_DIR)),$(PTIFF_C_LIB_DIR),$(CURDIR)/$(PTIFF_C_LIB_DIR))
	LIBC_LDFLAGS := -L$(LIB_DIR) -lptiff_c -Wl,-rpath,$(LIB_DIR)
	LIBC_CFLAGS  := -I$(C_ABI_DIR)
	LIBDIR       := $(LIB_DIR)
endif

PYINC := $(shell $(PYTHON) -c "import sysconfig; print(sysconfig.get_paths()['include'])")
RBHDR1 := $(shell $(RUBY) -rrbconfig -e 'print RbConfig::CONFIG["rubyhdrdir"]')
RBHDR2 := $(shell $(RUBY) -rrbconfig -e 'print RbConfig::CONFIG["rubyarchhdrdir"]')

# ---- Shim include dir: byte-identical copy of ../c with the export macro shimmed.
$(GEN_DIR):
	mkdir -p $(GEN_DIR)
	cp $(C_ABI_DIR)/*.h $(GEN_DIR)/
	cp $(SHIM_EXPORT) $(GEN_DIR)/ptiff_c_export.h

# ============================================================
# Python
# ============================================================
$(PY_DIR)/ptiff.py $(PY_DIR)/ptiff_wrap.c: ptiff.i typemaps.i $(GEN_DIR)
	$(SWIG) -python -outdir $(PY_DIR) -o $(PY_DIR)/ptiff_wrap.c -I$(GEN_DIR) ptiff.i

$(PY_DIR)/_ptiff.so: $(PY_DIR)/ptiff_wrap.c
	$(CC) -O2 -fPIC -I$(PYINC) $(LIBC_CFLAGS) -shared $(PY_DIR)/ptiff_wrap.c -o $@ $(LIBC_LDFLAGS) -undefined dynamic_lookup

python: $(PY_DIR)/_ptiff.so

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
# find the real headers + link libptiff_c (rpath baked in from LIBDIR).
$(GO_DIR)/cgo_flags.go: $(GO_MODROOT)go.mod
	printf 'package ptiff\n\n/*\n#cgo pkg-config: libptiff_c\n' > $@
	printf '#cgo LDFLAGS: -Wl,-rpath,%s\n*/\nimport "C"\n' '$(LIBDIR)' >> $@

go: $(GO_DIR)/ptiff.go $(GO_DIR)/ptiff_wrap.c $(GO_MODROOT)go.mod $(GO_DIR)/cgo_flags.go
	cd $(GO_DIR) && $(GO) build ./...

# ============================================================
# Ruby
# ============================================================
$(RB_DIR)/ptiff_wrap.c: ptiff.i typemaps.i $(GEN_DIR)
	$(SWIG) -ruby -outdir $(RB_DIR) -o $(RB_DIR)/ptiff_wrap.c -I$(GEN_DIR) ptiff.i

$(RB_DIR)/ptiff.bundle: $(RB_DIR)/ptiff_wrap.c
	$(CC) -O2 -fPIC -I$(RBHDR1) -I$(RBHDR2) $(LIBC_CFLAGS) -shared -undefined dynamic_lookup \
		$(RB_DIR)/ptiff_wrap.c -o $@ $(LIBC_LDFLAGS)

ruby: $(RB_DIR)/ptiff.bundle

# ============================================================
# Octave
# ============================================================
# mkoctfile drives its own compiler; re-use the pkg-config/PTIFF_C_LIB_DIR
# resolution but pass the -L / -l flags separately (mkoctfile chokes on a
# combined "-L<path> -l<name>" single argv like pkg-config emits). SWIG
# generates C++ for Octave (the octave_swig_ref runtime and octave_value API
# are C++), so we compile the wrapper as .cpp via mkoctfile.
ifeq ($(PTIFF_C_LIB_DIR),)
OC_LIBDIR := $(if $(PKGCONF),$(shell pkg-config --variable=libdir libptiff_c 2>/dev/null),)
OC_LDIR   := $(if $(PKGCONF),$(shell pkg-config --libs-only-L libptiff_c 2>/dev/null),)
OC_LIB    := $(if $(PKGCONF),$(shell pkg-config --libs-only-l libptiff_c 2>/dev/null),)
else
LIB_DIR := $(if $(filter /%,$(PTIFF_C_LIB_DIR)),$(PTIFF_C_LIB_DIR),$(CURDIR)/$(PTIFF_C_LIB_DIR))
OC_LIBDIR := $(LIB_DIR)
OC_LDIR   := -L$(LIB_DIR)
OC_LIB    := -lptiff_c
endif

$(OC_DIR)/ptiff_wrap.c: ptiff.i typemaps.i $(GEN_DIR)
	$(SWIG) -octave -outdir $(OC_DIR) -o $(OC_DIR)/ptiff_wrap.c -I$(GEN_DIR) ptiff.i

$(OC_DIR)/ptiff.oct: $(OC_DIR)/ptiff_wrap.c
	cp $(OC_DIR)/ptiff_wrap.c $(OC_DIR)/ptiff_wrap.cpp
	$(MKOCTFILE) $(OC_DIR)/ptiff_wrap.cpp $(LIBC_CFLAGS) $(OC_LDIR) $(OC_LIB) -Wl,-rpath,$(OC_LIBDIR) -o $@

octave: $(OC_DIR)/ptiff.oct

# ============================================================
# Aggregate
# ============================================================
all: python go ruby octave

# Python's, Ruby's and Octave's promoted tests live in bindings/{python,ruby,
# octave}/test/, separate from the promoted package dirs (PY_DIR/RB_DIR/
# OC_DIR) -- PYTHONPATH/RUBYLIB/OCTAVE_PATH point the test run at the
# generated module.
PY_TEST_DIR := ../python/test
RB_TEST_DIR := ../ruby/test
OC_TEST_DIR := ../octave/test

test: all
	cd $(PY_TEST_DIR) && PYTHONPATH=$(abspath $(PY_DIR)) $(PYTHON) test_roundtrip.py
	cd $(PY_TEST_DIR) && PYTHONPATH=$(abspath $(PY_DIR)) $(PYTHON) -m unittest test_version test_logger test_backend test_error test_image test_sink -v
	cd $(GO_DIR) && $(GO) test ./...
	cd $(RB_TEST_DIR) && RUBYLIB=$(abspath $(RB_DIR)) $(RUBY) test_roundtrip.rb
	cd $(RB_TEST_DIR) && RUBYLIB=$(abspath $(RB_DIR)) $(RUBY) -e 'Dir.glob("test_{version,logger,backend,error,image,sink}.rb").each { |f| require_relative f }'
	$(OCTAVE) --quiet --no-gui --eval "addpath('$(abspath $(OC_DIR))'); addpath('$(abspath $(OC_TEST_DIR))'); run(fullfile('$(abspath $(OC_TEST_DIR))','run_tests_octave.m'));"

clean:
	rm -rf $(GEN_DIR)
	rm -rf $(PY_DIR)/ptiff.py $(PY_DIR)/ptiff_wrap.c $(PY_DIR)/_ptiff.so $(PY_DIR)/__pycache__ $(PY_TEST_DIR)/roundtrip_python.tif
	rm -rf $(GO_DIR)/ptiff.go $(GO_DIR)/ptiff_wrap.c $(GO_DIR)/cgo_flags.go $(GO_DIR)/roundtrip_go.tif
	rm -rf $(GO_MODROOT)go.sum
	rm -rf $(RB_DIR)/ptiff_wrap.c $(RB_DIR)/ptiff.bundle $(RB_TEST_DIR)/roundtrip_ruby.tif
	rm -rf $(OC_DIR)/ptiff_wrap.c $(OC_DIR)/ptiff_wrap.cpp $(OC_DIR)/ptiff.oct

.PHONY: all python go ruby octave test clean
