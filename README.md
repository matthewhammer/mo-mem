# `mo-mem` Main Memory Snapshots for Motoko.

This repo contains an example protocol for getting snapshots of Motoko's main memory and saving these binary images as local files.

The example tool also exposes the image to MoVM, permitting Motoko scripts to analyze the image files.

(Long term, the same Motoko code can analyze their own image files stored remotely on the IC.)

## ⚠️ WIP

This repo depends on a special version of the Motoko compiler that exposes `prim "regionMainMemorySnapshot"`.

[This feature is currently an open PR. ](https://github.com/dfinity/motoko/pull/4233)


## Contets
- `motoko/`
  - `main.mo` gives the IC backend of the example protocol.
- `rust/`
  - `main.rs` gives the CLI frontend of the example protocol.


## Next steps
- Merge the compiler PR.
- Move general Rust stuff for Motoko VM to use memory-mapped image files into `motoko.rs` repo.
- Develop tools in Motoko to use on heap snapshots (based on reading from the file or from the region):
  - Reconstruct heap data structures
  - (Permit edits and rewrites?)
  - Summarize the way that the heap is used, with metrics that help devs optimize it
