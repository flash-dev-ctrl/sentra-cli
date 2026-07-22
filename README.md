# Sentra CLI

[![CI](https://github.com/flash-dev-ctrl/sentra-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/flash-dev-ctrl/sentra-cli/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/flash-dev-ctrl/sentra-cli?include_prereleases)](https://github.com/flash-dev-ctrl/sentra-cli/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[English](README.en.md)

Sentra CLI 是一个面向本机 AI Agent 资产的发现与风险扫描工具。它可以发现 Codex、Claude、OpenClaw、Hermes、Sentra 等 Agent 的 Skill、Provider、Memory、Cron 等资产，并使用本地规则、在线威胁情报和可选 LLM 进行安全审计。

项目由两部分组成：

- `sentra-cli`：命令行入口，负责用户交互、扫描编排、规则初始化和输出。
- `sentra-lib`：Rust 库，负责 Agent 发现、资产读取、规则加载和风险检查。

## 安装最新版

Linux 或 macOS 使用独立安装脚本，会自动识别系统和架构，并安装到 `~/.local/bin/sentra`：

```bash
curl -fsSL https://github.com/flash-dev-ctrl/sentra-cli/releases/latest/download/install.sh | sh
```

Windows 使用 PowerShell 安装脚本，安装到 `%USERPROFILE%\.sentra\bin\sentra.exe` 并写入当前用户 `PATH`：

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/flash-dev-ctrl/sentra-cli/releases/latest/download/install.ps1 | iex"
```

安装后如果当前终端找不到 `sentra`，请重新打开终端，或先手动把安装目录加入 `PATH`。可通过 `SENTRA_VERSION` 和 `SENTRA_INSTALL_DIR` 指定安装版本和目录。

## 常用命令

列出 Agent：

```bash
sentra list agent
```

列出 Skill：

```bash
sentra list skill
sentra list skill --agent codex-cli
sentra list skill --format json
```

安装 Skill：

```bash
sentra skill add https://example.test/skill.zip
sentra skill add https://example.test/skill.zip --agent codex-cli --force
```

扫描所有 Agent 的 Skill：

```bash
sentra scan skill
```

扫描指定 Agent：

```bash
sentra scan skill --agent codex-cli
sentra scan skill --agent codex-cli --agent claude
```

扫描指定目录中的 Skill：

```bash
sentra scan skill /path/to/skills
```

输出 JSON：

```bash
sentra scan skill --format json
sentra scan skill --format json --output scan-result.json
```

启用或关闭检查器：

```bash
sentra scan skill --with-llm
sentra scan skill --with-online-ti
sentra scan skill --without-yara
sentra scan skill --with-llm --with-online-ti --without-ti
```

默认启用本地 Hash、YARA、本地 TI 检查。LLM 和在线 TI 需要显式开启并配置相关参数。

## 编译

准备 Rust stable 工具链，克隆源码时拉取 `sentra-lib` submodule：

```bash
git clone --recurse-submodules https://github.com/flash-dev-ctrl/sentra-cli.git
cd sentra-cli
```

本机编译和测试：

```bash
cargo build --locked --release --bin sentra
cargo test --locked --all-targets
```

## 规则来源与致谢

Sentra 的默认规则参考并复用了以下开源项目中的规则，在此致谢：

- [nvidia/skillspector](https://github.com/nvidia/skillspector)
- [cisco-ai-defense/skill-scanner](https://github.com/cisco-ai-defense/skill-scanner)

这些项目推动了 AI Skill / Agent 安全扫描规则的开放实践。Sentra 在此基础上做了本地 Agent 资产发现、规则导入、CLI 输出和异步扫描。

## 开源协议

本项目使用 MIT License 开源，详见 [LICENSE](LICENSE)。

若继续分发或修改本项目，请保留版权声明、许可证文本，以及上游规则项目的来源说明。
