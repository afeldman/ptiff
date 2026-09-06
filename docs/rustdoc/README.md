# Rust API documentation (rustdoc)

PTIFF's Rust API is documented with **rustdoc**. The generated HTML site is
the primary API documentation for the reference implementation and the
canonical place to read the crate APIs next to the source.

## Workspace crates

| Crate             | Purpose                                                        |
|-------------------|----------------------------------------------------------------|
| `ptiff-core`      | Reference, dependency-light core: domain model, TIFF I/O, backends (`#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`) |
| `ptiff-rust`      | Idiomatic facade crate (package name `ptiff`) over the core: `Tiff`, `RemoteTiff`, high-level reader/writer |
| `ptiff-cli`       | `ptiff` command-line tool                                      |
| `ptiff-c`         | Stable C ABI veneer (`libptiff_c`); the cbindgen-generated header `target/ptiff_c.h` is its documented specification |

The PyO3 Python binding (`crates/ptiff-python`) is deliberately excluded from
the workspace (maturin builds it standalone) and therefore not part of
`cargo doc --workspace`.

## Generate

```bash
# All workspace library crates, all features, without pulling in dependency
# sources (`--lib` avoids the target/doc/ptiff/ collision between the CLI
# binary "ptiff" and the "ptiff" library):
cargo doc --workspace --all-features --no-deps --lib

# Convenience: open the crate you care about in the browser
cargo doc -p ptiff --open
```

Output goes to `target/doc/` (e.g. `target/doc/ptiff/index.html` for the
`ptiff-rust` facade crate, `target/doc/ptiff_c/index.html` for the C ABI
crate). `target/` is ignored by Git, so generated rustdoc output is never
committed.

## Documentation conventions

* `ptiff-core` and `ptiff-rust` enable `#![warn(missing_docs)]`; keep public
  items documented and intra-doc links (`` [`Item`] ``) resolvable.
* `ptiff-c` relaxes `missing_docs`: the crate mirrors the C ABI, and the
  generated header (documented on the Doxygen site) is the specification
  foreign runtimes read. Keep the module/function-level guidance there.
* Private-item documentation is not enabled; examples live in
  `crates/ptiff-rust/examples/` and are exercised by
  `cargo test --workspace --all-features`.

## CI

`cargo doc --workspace --all-features --no-deps --lib` runs on every push/PR
in the CI documentation check (`.github/workflows/docs-check.yml`) with
`RUSTDOCFLAGS="-D warnings"`, so broken intra-doc links and missing-doc
regressions fail CI. See `docs/README.md` for the full documentation
workflow.
