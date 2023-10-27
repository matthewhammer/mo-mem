# `mo-mem` Main Memory Snapshots for Motoko.

This repo contains an example protocol for getting snapshots and saving locally.

## ⚠️ WIP
This repo depends on a special version of the Motoko compiler that exposes `prim "regionMainMemorySnapshot"`.  
This feature is currently an open PR. 


## Repo contents:
- `motoko/`
  - `main.mo` gives the IC backend of the example protocol.
- `rust/`
  - `main.rs` gives the CLI frontend of the example protocol.


## Todo:
- Move general stuff for memory-mapped image files into the Motoko VM repo.
- Develop tools in Motoko to use on heap snapshots (based on reading from the file or from the region).
