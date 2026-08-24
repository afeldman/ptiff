# ptiff_c_abiltest: C ABI capability test over the PTIFF Rust core.
#
# Builds the ptiff-c staticlib with cargo, then compiles this C program against
# the cbindgen-GENERATED C header (crates/ptiff-c is the source of truth; the
# header is emitted to target/ptiff_c.h by build.rs) and links the Rust
# staticlib. Run `make` (default: build + run), `make build` to only compile,
# or `make run` to only run an existing binary.
#
# Requires: cargo (Rust toolchain) + a C compiler (cc / clang).

HEADERS      := $(realpath ../../../../target)
LIBDIR       := $(realpath ../../../../target/release)
BIN          := abiltest
SRC          := ptiff_c_abiltest.c

OUT_TIF      := ptiff_c_abiltest_out.tif

CC      ?= cc
CFLAGS  ?= -O2 -Wall -Wextra -std=c11
# libdl doesn't exist on Windows (dlopen-equivalents ship in the CRT/kernel32
# already); MinGW's ld fails outright with "cannot find -ldl" if it's listed.
# `-lptiff_c` resolves to the static `ptiff_c.lib` there (no matching DLL
# import lib is produced), which pulls in Rust std's raw Windows syscalls
# (WSAGetLastError, RtlNtStatusToDosError, ...) -- these live in ws2_32,
# ntdll, bcrypt and userenv, the same set rustc's own windows-gnu linker
# script pulls in for a static build.
UNAME_S := $(shell uname -s 2>/dev/null)
ifneq (,$(findstring MINGW,$(UNAME_S))$(findstring MSYS,$(UNAME_S)))
LDFLAGS ?= -lpthread -lm -lws2_32 -lbcrypt -luserenv -lntdll
else
LDFLAGS ?= -lpthread -ldl -lm
endif

.PHONY: all build run staticlib clean

all: build run

staticlib:
	cd ../../../.. && cargo build -p ptiff-c --release

build: staticlib $(BIN)

$(BIN): $(SRC) Makefile
	@test -f $(HEADERS)/ptiff_c.h || (echo "ptiff_c.h fehlt — läuft cargo build (generiert ihn)"; cd ../../../.. && cargo build -p ptiff-c)
	$(CC) $(CFLAGS) -I$(HEADERS) $(SRC) -L$(LIBDIR) -lptiff_c $(LDFLAGS) -o $@

# The `libptiff_c` shared object/DLL sits in $(LIBDIR), not next to the
# binary, so the dynamic loader needs to be pointed at it explicitly:
# LD_LIBRARY_PATH (Linux), DYLD_LIBRARY_PATH (macOS, harmless if unused),
# PATH (Windows, which resolves DLLs via PATH, not rpath).
run: $(BIN)
	LD_LIBRARY_PATH="$(LIBDIR):$$LD_LIBRARY_PATH" DYLD_LIBRARY_PATH="$(LIBDIR):$$DYLD_LIBRARY_PATH" PATH="$(LIBDIR):$$PATH" ./$(BIN)

clean:
	rm -f $(BIN) $(OUT_TIF) $(CAMERA_TIF)
