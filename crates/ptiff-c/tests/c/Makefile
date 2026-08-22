# ptiff_c_abiltest: C ABI capability test over the PTIFF Rust core.
#
# Builds the ptiff-c staticlib with cargo, then compiles this C program against
# the hand-maintained C headers in bindings/ (source of truth) and links the
# Rust staticlib. Run `make` (default: build + run), `make build` to only
# compile, or `make run` to only run an existing binary.
#
# Requires: cargo (Rust toolchain) + a C compiler (cc / clang).

HEADERS      := $(realpath ../../../../bindings/c)
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
	$(CC) $(CFLAGS) -I$(HEADERS) $(SRC) -L$(LIBDIR) -lptiff_c $(LDFLAGS) -o $@

run: $(BIN)
	./$(BIN)

clean:
	rm -f $(BIN) $(OUT_TIF) $(CAMERA_TIF)
