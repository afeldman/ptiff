# PTIFF — C/C++ API and concept documentation {#mainpage}

This Doxygen site documents the **C and C++ view of the PTIFF reference
implementation** together with the project's concept/overview pages. It
complements the primary Rust API documentation (rustdoc); the two sites cover
one architecture from the two sides of the same FFI boundary (see
[Bindings & the C ABI](@ref bindings)).

## What you find in this site

| Section                          | Contents                                                                  |
|----------------------------------|---------------------------------------------------------------------------|
| Namespace `ptiff` / Classes      | The `ptiff-cpp` wrapper (`bindings/cpp`): `Image`, `Scene`, `Reader`, `Writer`, `Camera`, `Result<T>`, `Error`, ... |
| Files → `ptiff_c.h`              | The stable C ABI `libptiff_c` — cbindgen-generated from `crates/ptiff-c` (Rust `extern "C"` signatures are the source of truth). |
| Files → `ptiff/*.hpp`            | Header-only C++ wrapper headers (`bindings/cpp/include/ptiff/`). The internal `ptiff::detail` marshalling layer is excluded from the index. |
| Pages                            | This main page, the bindings/FFI overview, plus the repository `README.md` and `ARCHITECTURE.md` as concept pages. |

The Doxygen input is configured in the repository-root `Doxyfile`
(`INPUT = README.md, ARCHITECTURE.md, docs/doxygen/, target/ptiff_c.h,
bindings/cpp/include`). Hand-written overview pages live in `docs/doxygen/`.

## Documented surface

* **C ABI** (`libptiff_c`, header `target/ptiff_c.h`): opaque handles
  (`ptiff_source`, `ptiff_sink`, `ptiff_image`), negative `ptiff_error_code`
  returns, callee-allocated strings freed with `ptiff_free_string`. Pure
  `extern "C"` over `stdint.h`/`stddef.h` — no C++ types cross the boundary.
* **C++ wrapper** (`ptiff-cpp`, `bindings/cpp`): a modern, hand-written,
  header-only RAII facade over that C ABI. `Result<T> = std::expected<T,
  Error>` (never throws), move-only owning handles, STL value types.

The Rust-side documentation for the same architecture — including the
`ptiff-c` crate whose `extern "C"` declarations authorise the header — is
generated with rustdoc (`cargo doc --workspace --all-features --no-deps
--lib`).

## How to generate this site

```bash
cargo build -p ptiff-c   # generate target/ptiff_c.h (cbindgen)
mkdir -p build/docs      # Doxygen >= 1.18 needs the output directory
doxygen Doxyfile         # -> build/docs/html/index.html
```

or, when the project was configured with CMake: `cmake --build build --target
docs`. See `docs/README.md` in the repository for the full documentation
workflow.
