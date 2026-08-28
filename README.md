<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/readme/header-dark.svg">
    <img alt="Kosmos — local-first personal software" src=".github/readme/header-light.svg" width="100%">
  </picture>
</p>

<p align="center"><strong>One data core. Many focused tools. No cloud required.</strong></p>

<p align="center">
  <a href="https://github.com/makekosmos/core"><img alt="GitHub stars" src="https://shieldcn.dev/github/makekosmos/core/stars.svg?variant=branded&size=sm"></a>
  <img alt="Last commit" src="https://shieldcn.dev/github/makekosmos/core/last-commit.svg?variant=branded&size=sm">
  <img alt="Active development" src="https://shieldcn.dev/badge/status-active+development-F59E0B.svg?variant=branded&size=sm">
</p>

<p align="center">
  <a href="https://github.com/makekosmos/docs">Documentation</a> ·
  <a href="https://github.com/makekosmos/docs/blob/main/concepts/architecture.md">Architecture</a> ·
  <a href="./crates/ark-core/README.md">ARK</a> ·
  <a href="#development">Development</a>
</p>

## What is Kosmos?

Kosmos is a **local-first personal software system for Windows**. At its center is **ARK**, a shared Rust + SQLite data runtime for typed objects, links, activity data, and synchronization. Around it are focused tools for notes, tasks, games, reading, focus, and personal analytics.

The UI is replaceable; the data contract is not. Applications do not own isolated databases and do not write directly to SQLite. They communicate with ARK through the same narrow SDK, so a note, task, game, or usage session can remain useful outside the interface that created it.

<p align="center">
  <img alt="Eden typed objects in Kosmos" src=".github/readme/eden.png" width="100%">
</p>

## Principles

- **Local-first** — the complete working dataset lives on the device; the network is optional.
- **One canonical runtime** — ARK owns persistence, object contracts, links, usage data, and sync.
- **Focused clients** — each app solves one workflow instead of becoming another silo.
- **Replaceable interfaces** — desktop apps, future native clients, CLI tools, and projections share stable contracts.
- **Auditable changes** — substantial work follows a spec → evidence → verification proof loop.

## Architecture

```mermaid
flowchart TB
  Apps["Eden · Delphi · Arrancador · Akasha"] --> Shell["Kosmos Desktop Shell<br/>Electron · Vue"]
  Dashboard["Dashboard · Focus · Launcher"] --> Shell
  Shell --> SDK["@kosmos/ark"]
  SDK --> Runtime["Kosmos Runtime<br/>Rust · WebSocket · command bus"]
  Runtime --> ARK["ARK Data Engine<br/>objects · links · usage · sync"]
  ARK --> DB[("Local SQLite")]
  ARK --> Sync["LAN / relay synchronization"]
```

The Electron renderer never opens SQLite. Cortex talks to ARK through
`@kosmos/ark`; ARK remains the canonical owner of data and sync state.

## Surfaces

| Surface          | Role                                                         | State     |
| ---------------- | ------------------------------------------------------------ | --------- |
| **Kosmos Shell** | Launcher, extension host, settings, Dashboard, Focus Session | Active    |
| **Eden**         | Notes, journal, and typed personal objects                   | Active    |
| **Delphi**       | Tasks and inbox workflows                                    | Active    |
| **Arrancador**   | Game library, playtime, backups, and `game_obj` integration  | Incubator |
| **Akasha**       | Continuous EPUB reader                                       | Incubator |
| **ARK**          | Shared typed-data runtime, local storage, usage, and sync    | Core      |

## Stack

<p>
  <img alt="Local-first" src="https://shieldcn.dev/badge/local--first-by+design-8B5CF6.svg?variant=branded&size=sm&logo=sqlite">
  <img alt="Windows desktop" src="https://shieldcn.dev/badge/Windows-desktop-0078D4.svg?variant=branded&size=sm&logo=windows11">
  <img alt="Rust ARK" src="https://shieldcn.dev/badge/Rust-ARK-B7410E.svg?variant=branded&size=sm&logo=rust">
  <img alt="Electron shell" src="https://shieldcn.dev/badge/Electron-shell-47848F.svg?variant=branded&size=sm&logo=electron">
  <img alt="Vue extensions" src="https://shieldcn.dev/badge/Vue-extensions-42B883.svg?variant=branded&size=sm&logo=vuedotjs">
  <img alt="Bun workspace" src="https://shieldcn.dev/badge/Bun-1.3+-000000.svg?variant=branded&size=sm&logo=bun">
</p>

- **Data:** Rust, SQLite, FTS5, Hybrid Logical Clock, LAN/relay sync
- **Desktop:** Electron, Vue, TypeScript, Vite
- **Design:** `@kosmos/visuals`, shared OKLCH tokens and desktop primitives
- **Quality:** Playwright, Vitest, oxlint, oxfmt, proof-loop evidence

## Repository boundary

```text
core/ark/              ARK storage engine, schema, sync and RPC sidecar
```

Desktop, Manager, runtime supervisor and native Windows services live in
[`makekosmos/cortex`](https://github.com/makekosmos/cortex). The TypeScript
SDK and visuals package are maintained in `arca-sdk` and `imago`.

## Development

### Requirements

- Windows 10/11
- [Bun](https://bun.sh/) 1.3.5+
- stable [Rust](https://www.rust-lang.org/tools/install) toolchain
- Node.js 20+
- PowerShell 7+
- `cargo-nextest`, `cargo-shear`, and `cargo-deny` for repository quality gates

```powershell
git clone https://github.com/makekosmos/core.git
cd core
bun install

# Check the ARK storage engine
cargo check -p ark-core
```

Useful repository checks:

```powershell
bun run ark:guard:writes
bun run ark:smoke
cargo test -p ark-core
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo deny check advisories bans sources
```

The local/CI Clippy gate keeps `-D warnings`; eight existing noisy lint
categories are explicitly baselined in `lefthook.yml` and CI until cleaned up.

`bun install` installs Lefthook hooks. Pre-commit runs staged-file checks and
fast Rust validation; pre-push runs the full Rust, dependency, and source-size
checks. CI is the enforceable superset.

Read the [getting-started guide](https://github.com/makekosmos/docs/blob/main/guide/getting-started.md) before
changing ARK or sync. Desktop development happens in
[`makekosmos/cortex`](https://github.com/makekosmos/cortex).
