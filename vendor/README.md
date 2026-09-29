# Vendored dependencies

These crates are small, minimally patched copies of crates.io releases. They
are pulled in through the `[patch.crates-io]` section of the root
`Cargo.toml` because their original versions do not build with recent Rust
toolchains, and no fixed release exists.

| Crate         | Version | Reason for the patch                                                                                                                        |
| ------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `devise_core` | 0.2.1   | Used `#![feature(concat_idents)]`, which was removed from Rust. The default mapper names are now passed explicitly to the `mappers!` macro. |
| `traitobject` | 0.1.0   | Implemented the same trait for several equivalent `dyn` types (`dyn Send`, `dyn Sync`, `dyn Send + Sync`), which newer compilers reject.     |
| `rocket_http` | 0.4.11  | Specialised the blanket `impl<T: Display> ToString for T`, which newer compilers reject. The redundant impl was removed (`Display` remains). |

The patches only remove or rewrite the incompatible constructs: no behavior
is changed. A `cargo update` should not be allowed to silently drop them:
keep the entries in `Cargo.toml` in sync with this directory.
