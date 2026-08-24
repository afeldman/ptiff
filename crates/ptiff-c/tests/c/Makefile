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

CC      ?= cc
CFLAGS  ?= -O2 -Wall -Wextra -std=c11
LDFLAGS ?= -lpthread -ldl -lm

.PHONY: all build run staticlib clean

all: build run

staticlib:
	cd ../../../.. && cargo build -p ptiff-c --release

build: staticlib $(BIN)

$(BIN): $(SRC) Makefile
	@test -f $(HEADERS)/ptiff_c.h || (echo "ptiff_c.h fehlt — läuft cargo build (generiert ihn)"; cd ../../../.. && cargo build -p ptiff-c)
	$(CC) $(CFLAGS) -I$(HEADERS) $(SRC) -L$(LIBDIR) -lptiff_c $(LDFLAGS) -o $@

run: $(BIN)
	./$(BIN)

clean:
	rm -f $(BIN) $(OUT_TIF) $(CAMERA_TIF)
