# `mo-mem` Main Memory Snapshots for Motoko.

This repo contains an example protocol for getting snapshots of Motoko's main memory and saving these binary images as local files.

The example tool also exposes the image to MoVM, permitting Motoko scripts to analyze the image files.

(Long term, the same Motoko code can analyze their own image files stored remotely on the IC.)

## ⚠️ WIP

This repo depends on a special version of the Motoko compiler that exposes `prim "regionMainMemorySnapshot"`.

[This feature is currently an open PR. ](https://github.com/dfinity/motoko/pull/4233)


## Contents
- `motoko/`
  - `main.mo` gives the IC backend of the example protocol.
- `rust/src/`
  - `main.rs` gives the CLI frontend of the example protocol.

## Playing around

### Compile and deploy

- in `motoko/`, do `dfx deploy --network ic`
- in `rust/`, do 
  - `ln -s ../motoko/canister_ids.json .` to symbolically link the `canister_ids.json` file.
  - `cargo run` to run the CLI tool.

### CLI Commands

To start out, try these commands, in this order:

- `info` checks the canister is there, but no snapshots.
- `create` / `update` to create the first snapshot.
- `pull` to download the first snapshot into a local file.
- `eval "image.size()"` will produce the size of the file, in bytes.

More generally, 

 - `update` and `create` will produce new snapshots for each invocation, 
 - `pull` will download the latest one, and 
 - `eval` will analyze the latest one (unless `-f` is supplied to it for a different image file).

### Motoko VM integration

The `eval` command uses MoVM to run Motoko programs that analyze the `image`.

In each program, `image` is a value representing the memory-mapped file for the image (avoiding loading the entire thing into memory, since it could be big, and in the future, we may even want to load a bunch of them to compare them with a Motoko script.


## Next steps
- Merge the compiler PR.
- Move general Rust stuff for Motoko VM to use memory-mapped image files into `motoko.rs` repo.
- Develop tools in Motoko to use on heap snapshots (based on reading from the file or from the region):
  - Reconstruct heap data structures
  - (Permit edits and rewrites?)
  - Summarize the way that the heap is used, with metrics that help devs optimize it
