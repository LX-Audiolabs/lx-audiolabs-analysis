# LX Audiolabs — plugins

[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Slint](https://img.shields.io/badge/Slint-1.18.1-2379F4.svg)](https://slint.dev)
[![AURA](https://img.shields.io/badge/framework-AURA-6E40C9.svg)](https://github.com/LX-Audiolabs/aura)

Official public product catalog for **LX Audiolabs**, built on **[AURA](https://github.com/LX-Audiolabs/aura)** (Slint UI + CLAP-first). Release CLAPs are built on **GitHub Actions**.

| | |
|--|--|
| **License** | [GPL-3.0-or-later](./LICENSE) |
| **Framework** | [AURA](https://github.com/LX-Audiolabs/aura) via crates.io (`lx-aura-*` 0.14.1) |
| **Ship format** | **CLAP** (per-plugin Actions) · **Windows VST3** · **Linux LV2** |
| **Other formats** | macOS VST3/LV2 not shipped (CLAP + Mac ad-hoc signing left out) |

---

## Status

| Plugin | Version | Role | Ship |
|--------|---------|------|------|
| **Aether** | 1.x | EQ + crossfeed | yes |
| **Meridian** | 1.x | Channel strip | yes |
| **Equilibrium** | 1.x | Pre-master spectral balancer | yes |
| **Lucent** | 1.x | Spectrum analyzer (standalone / hybrid / relay) | yes |
| **Lucent Relay** | 1.x | Relay publisher for Lucent | yes |

These five plugins are the current **shipping set**.

---

## Layout

```
crates/                 # product libraries
  lx-ui-slint/          # design system (Lx* components)
  lx-editor-utils/      # editor helpers (dirty, meters, snap, …)
  lx-vault/             # preset paths / last-preset config
  lx-analysis/          # product *Shared analysis types
plugins/                # products
  aether/ meridian/ equilibrium/
  lucent/ lucent-relay/
```

Runtime GUI: **aura-editor** + **aura-baseview** (default FemtoVG / wgpu).

---

## Requirements

### Framework (AURA)

Ship deps come from **crates.io** (`lx-aura`, `lx-aura-editor`, `lx-aura-dsp`,
`lx-aura-build`, `lx-aura-test`, … — workspace pins **0.14.1**). Lib names stay
`aura::` / `aura_test::` / …

`cargo-aura` / preview / host are **not** on crates.io yet — install from the
AURA git tree when you need the CLI locally:

```bash
cargo install --git https://github.com/LX-Audiolabs/aura --locked cargo-aura
# or path: cargo install --path ../AURA/tools/cargo-aura --locked
```

Docs, ship matrix, and format wrappers: **[AURA README](https://github.com/LX-Audiolabs/aura)**.

### System (UI)

Default editor path is **FemtoVG / wgpu** (Vulkan / DirectX 12 / Metal). A working GPU stack is required to open the UI. **Audio processing does not need GPU** — only the editor window does.

---

## Plugins (detail)

| Crate | What |
|-------|------|
| `aether` | EQ + crossfeed |
| `meridian` | Channel strip |
| `equilibrium` | Pre-master spectral balancer |
| `lucent` | Spectrum analyzer (standalone / hybrid / relay) |
| `lucent-relay` | Relay publisher |

---

## Build (CLAP — supported ship path)

```powershell
# Workspace check
cargo check --workspace

# Install CLAP into the host search path (release)
cargo aura install --clap --release -plug meridian

# All shipping plugins
cargo aura install --clap --release -plug aether meridian equilibrium lucent lucent-relay

# Artifacts are produced under target/release/ (or target/<target-triplet>/release/
# for cross builds). GitHub Actions builds and packages the release CLAPs; local
# packaging scripts are no longer shipped in this repository.
```

### CLAP validate 

use https://github.com/free-audio/clap-validator

---

## VST3 (Windows — Actions)

Workflow **Build VST3 (Windows)** (`workflow_dispatch`) builds all shipping plugins and uploads **one zip per plugin**:

`{crate}-v{version}-win-vst3.zip` → `Name.vst3/Contents/x86_64-win/Name.vst3`

```powershell
# Local equivalent (one plugin)
cargo build --release -p meridian --no-default-features --features vst3
# then copy target/release/meridian.dll into Meridian.vst3/Contents/x86_64-win/Meridian.vst3
```

Or with cargo-aura: `cargo aura install --vst3 --release -plug meridian`.

## LV2 (Linux — Actions)

Workflow **Build LV2 (Linux)** (`workflow_dispatch`) builds all shipping plugins and uploads **one zip per plugin**:

`{crate}-v{version}-lin-lv2.zip` → `Name.lv2/{manifest.ttl,plugin.ttl,lib*.so}`

TTL is generated from real params via `write_lv2_ttl_sidecar` (not cargo-aura’s one-gain fallback):

```bash
cargo build --release -p meridian --no-default-features --features lv2
LV2_TTL_OUT=target/release/meridian-lv2-ttl \
  cargo test -p meridian --no-default-features --features lv2 --release \
  tests::writes_lv2_ttl_sidecar -- --exact
# then pack Name.lv2/ with the .so + both .ttl files
```

`cargo aura install --lv2` still works **after** that sidecar exists under `target/<profile>/{pkg}-lv2-ttl/`.

Details and matrix: **[github.com/LX-Audiolabs/aura](https://github.com/LX-Audiolabs/aura)**.

---

## License

Copyright © 2026 LX Audiolabs  

GPL-3.0-or-later — see [LICENSE](./LICENSE).  
Plugins link **AURA** (also GPL-3.0-or-later); distributing a plugin binary means GPL obligations for that combined work.
