# Sentra CLI

[![CI](https://github.com/flash-dev-ctrl/sentra-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/flash-dev-ctrl/sentra-cli/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/flash-dev-ctrl/sentra-cli?include_prereleases)](https://github.com/flash-dev-ctrl/sentra-cli/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Sentra CLI discovers and scans local AI Agent assets. It supports Codex, Claude, OpenClaw, Hermes, Sentra, and related Agent resources such as skills, providers, memory, and cron entries.

The repository contains two Rust components:

- `sentra-cli`: command-line interface, scan orchestration, bundled rule import, and output rendering.
- `sentra-lib`: reusable Rust library for Agent discovery, asset readers, rule loading, and risk evaluation.

## Requirements

- Rust stable with edition 2024 support. Rust 1.85 or newer is recommended.
- GitHub builds use GitHub Actions on Linux, Windows, and macOS.

## Install

On macOS or Linux, use the standalone installer:

```bash
curl -fsSL https://github.com/flash-dev-ctrl/sentra-cli/releases/latest/download/install.sh | sh
```

On Windows, run:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/flash-dev-ctrl/sentra-cli/releases/latest/download/install.ps1 | iex"
```

Restart the terminal if `sentra` is not found immediately after installation. Use `SENTRA_VERSION` and `SENTRA_INSTALL_DIR` to select a version or install directory.

To build from source:

```bash
git clone --recurse-submodules https://github.com/flash-dev-ctrl/sentra-cli.git
cd sentra-cli
cargo build --locked --release --bin sentra
```

If the repository was cloned without submodules, initialize `sentra-lib` before
building:

```bash
git submodule update --init --recursive
```

Run the binary:

```bash
./target/release/sentra --help
```

On Windows PowerShell:

```powershell
.\target\release\sentra.exe --help
```

## Common Commands

List Agents:

```bash
sentra list agent
```

List skills:

```bash
sentra list skill
sentra list skill --agent codex-cli
sentra list skill --format json
```

Scan skills:

```bash
sentra scan skill
sentra scan skill --agent codex-cli
sentra scan skill /path/to/skills
```

Write JSON output:

```bash
sentra scan skill --format json --output scan-result.json
```

Enable optional checks:

```bash
sentra scan skill --with-llm
sentra scan skill --with-online-ti
sentra scan skill --without-yara
```

Local hash, YARA, and local TI checks are enabled by default. LLM and online TI checks must be enabled explicitly.

## Rules

The root `rules/` directory is bundled into the CLI at build time. On first scan without a custom `scan.rules` configuration, Sentra imports bundled rules into:

```text
~/.sentra/yara
~/.sentra/ti
~/.sentra/hash
```

The default rules reference and reuse rule ideas from:

- [nvidia/skillspector](https://github.com/nvidia/skillspector)
- [cisco-ai-defense/skill-scanner](https://github.com/cisco-ai-defense/skill-scanner)

## Development

```bash
git submodule update --init --recursive
cargo fmt --all -- --check
cargo test --locked --all-targets
```

See [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and [docs/architecture.md](docs/architecture.md) for project guidance.

## License

Sentra CLI is released under the MIT License. See [LICENSE](LICENSE).
