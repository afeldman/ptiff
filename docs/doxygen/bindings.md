# Bindings and the C ABI {#bindings}

## One core, one stable FFI surface

PTIFF's reference implementation is a pure-Rust workspace. The Rust core and
idiomatic crates expose **no C ABI** (`#![forbid(unsafe_code)]`); the stable C
ABI lives in a dedicated `ptiff-c` crate so that every foreign runtime links
the same, small, hand-audited `extern "C"` surface (`libptiff_c`):

```
Rust workspace                      C/C++ and FFI consumers
─────────────────────              ──────────────────────────
ptiff-core (reference core)         C consumers  (tests, SWIG-generated glue)
   │                                ┌──────────────────────────────┐
   ▼                                │ C ABI libptiff_c              │
ptiff-rust (idiomatic facade)  ──►  │ header: target/ptiff_c.h      │
   │                                │ cbindgen-authorised from      │
   ▼                                │ crates/ptiff-c                │
ptiff-c  (extern "C" veneer)  ──►   └──────────────────────────────┘
                                              │
                        ┌─────────────────────┼──────────────────────┐
                        ▼                     ▼                      ▼
               ptiff-cpp (C++ wrapper)  Ptiff.jl (Julia ccall)   Go/Ruby/Octave
               bindings/cpp             bindings/julia            bindings/*
               header-only RAII                                   (SWIG or native)
```

## Source of truth and generation

The Rust `extern "C"` declarations in `crates/ptiff-c` are the **single source
of truth** for the ABI. cbindgen (`crates/ptiff-c/build.rs` +
`cbindgen.toml`) generates `target/ptiff_c.h` at build time; do not edit that
header by hand. The C++ wrapper's `ptiff::detail::c_abi.hpp` hand-marshals the
slice of the ABI the wrapper uses and pins its struct layouts/enum values to
the generated header.

## The ABI contract

* **Opaque handles** — all resources (`ptiff_source`, `ptiff_sink`,
  `ptiff_image`) are opaque, heap-owned pointers; released with their
  `_close`/`_destroy` counterpart (`NULL` = no-op).
* **Error codes** — every fallible function returns a negative
  `ptiff_error_code` on failure (`0` on success).
* **Ownership** — callee-allocated strings are released with
  `ptiff_free_string`; out-structs are written into caller-owned buffers.
* **No C++ types cross the boundary** — pure `extern "C"` over `stdint.h` /
  `stddef.h` types, so any FFI runtime can link it.

## Which documentation belongs to which side

| Layer        | Code location                | Documented by                        |
|--------------|------------------------------|--------------------------------------|
| Rust core    | `crates/ptiff-core`          | rustdoc (`ptiff` core modules)       |
| Rust facade  | `crates/ptiff-rust`          | rustdoc                              |
| C ABI        | `crates/ptiff-c` → `target/ptiff_c.h` | rustdoc (crate) **and** this Doxygen site (header) |
| C++ wrapper  | `bindings/cpp/include/ptiff/`| this Doxygen site (namespace `ptiff`)|
| Other FFI runtimes (Go, Ruby, Octave, Julia, Python, MCP) | `bindings/` | their per-binding `README.md` (see `bindings/README.md`) |

This site's C ABI pages therefore document the exact contract the C++ wrapper,
`Ptiff.jl`, and the SWIG-generated bindings speak — the C header is the
interop specification foreign runtimes read.
