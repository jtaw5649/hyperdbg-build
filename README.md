# HyperDbg Build Helper

Local Rust wrapper for building selected HyperDbg Visual Studio targets from
Linux through Wine/MSBuild.

It keeps Wine/MSBuild workarounds under `hyperdbg-build/out/`, stages build
artifacts, and verifies the staged bundle.

## Commands

```sh
cargo run --manifest-path hyperdbg-build/Cargo.toml -- env
cargo run --manifest-path hyperdbg-build/Cargo.toml -- build target hyperdbg-cli --config debug
cargo run --manifest-path hyperdbg-build/Cargo.toml -- build lane --config debug
cargo run --manifest-path hyperdbg-build/Cargo.toml -- scan stage --config debug
cargo test --manifest-path hyperdbg-build/Cargo.toml
```

Set `HYPERDBG_BUILD_REPO_ROOT` to the HyperDbg checkout and
`HYPERDBG_BUILD_MSVC_ROOT` to the MSVC root. Set `HYPERDBG_BUILD_WINEPREFIX`
only when the Wine wrapper needs a specific prefix.

## Build Flow

`build lane` builds the allowlisted solution targets in dependency order,
preassembles MASM objects, applies local MSBuild overlays, copies script-engine
`.ds` files, and stages artifacts under `hyperdbg-build/out/stage/<config>/`.

HyperDbg's normal solution outputs stay under `hyperdbg/build/`. Helper logs and
generated overlays stay under `hyperdbg-build/out/`.

## Custom Names

Pass all four custom names together:

```sh
cargo run --manifest-path hyperdbg-build/Cargo.toml -- build lane --config debug \
  --sdk-dll-name ExampleSdk.dll \
  --driver-file-name ExampleDriver.sys \
  --driver-service-name ExampleService \
  --device-name ExampleDevice
```

These names are used for staging, manifest generation, and scanner validation.

## Manifest And Scan

`build lane` writes `release-manifest.json` beside the staged artifacts. It
records names, derived device paths, source commit, build config, and artifact
size/SHA-256 values.

`scan stage` validates the manifest, stage directory coverage, artifact hashes,
and expected driver/device strings in staged binaries.

With `--require-custom`, the scanner also rejects default HyperDbg names in the
scanner-scoped bytes and verifies consumers reference the custom SDK DLL.
