# Third-party code and tools

- `vendor/avm-contract-sdk`: guest SDK source pinned to GOSH AVM SDK commit
  `3a33ece89e67e291785f07e84f5b98a00ae85752`, MIT, GOSH Technology Ltd. The source
  bytes and `tools/release.py` are checked against `UPSTREAM.json`. Only the
  guest crate's standalone manifest supplies local workspace metadata.
- `vendor/registry`: unmodified Cargo-vendored crates with their upstream
  licenses/checksum manifests. `dlmalloc 0.2.14` (MIT/Apache-2.0, alexcrichton/
  dlmalloc-rs) provides the wasm allocator; `cfg-if 1.0.5` is its shared helper.
  The lock also retains platform dependencies `libc 0.2.191`, `windows-link 0.2.1`
  and `windows-sys 0.61.2`; these are not the selected wasm runtime allocator path.
  See each crate's license files. No allocator internals were rewritten.
- The native canonical-frame helper and `Stand` subprocess wrapper adapt the
  SDK's MIT bounty-escrow fixtures. License retained at
  `licenses/bounty-escrow-LICENSE`.
- `scripts/install_binaryen.sh` adapts the SDK's vesting-streams installer.
  Binaryen 133 is a checksum-pinned external build tool, not redistributed in
  this archive. Original and optimized project WASM are both retained.
- The official SDK binary bundle and Rust toolchain are not redistributed.
- Financial logic, offer protocol, payload CLI, tests, bigint oracle and journeys
  are new project code. They do not include production or audit guarantees.
