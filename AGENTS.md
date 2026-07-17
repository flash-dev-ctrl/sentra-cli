# AGENTS.md

本文件约束本仓库中的代理协作方式。子目录存在更近的 `AGENTS.md` 时，以更近文件为准；用户有明确要求时，优先满足用户要求。

## 1. 中文协作

- 与用户的计划、进度、澄清、最终回复、MR/PR 文本、评论和提交信息均使用中文。
- 提交信息格式：`<类型>: <简要描述>`，如 `修复: 处理配置读取失败`。
- 代码标识符、命令、路径、协议字段、第三方 API、英文错误原文可保留原文。
- 用户可见产品文本必须走 i18n，不要因中文协作要求而硬编码中文。

## 2. 工作方式

- 修改、运行命令或调查前，先用 1 到 2 句说明目标、做法和验证方式；复杂任务再拆阶段。
- 不确定或高影响问题先澄清；范围变化时及时更新计划。
- 只做当前请求需要的事，不添加未要求的功能、配置或抽象。
- 发现无关问题只说明，不擅自修复。

## 3. 实现质量

- 遵循现有代码风格、模块边界和工具链，优先复用仓库已有模式与成熟库。
- 结构化数据使用 JSON/YAML/TOML/XML 等对应解析库，避免脆弱字符串处理。
- 保持类型安全，避免魔法字符串、含糊布尔参数和难懂的位置参数。
- 面向外部输入、文件系统、网络、并发和时间的代码要处理可预见失败，并提供带上下文的错误。
- 本次修改引入的无用导入、变量、函数或测试辅助代码应一并移除。

## 4. 国际化

- 新增或变更用户可见文本时，同步更新中文和英文语言资源。
- 日期、数字、货币等格式化内容使用本地化 API。
- 避免拼接自然语言句子；验证语言切换后无遗漏硬编码文本。

## 5. 验证与交付

- 缺陷修复优先补能复现问题的测试；新功能覆盖主路径、边界和失败场景。
- 运行与改动最相关的格式化、lint、测试或构建命令；无法运行时说明原因和剩余风险。
- 长耗时任务持续给出中文进度。
- 最终回复说明完成内容、验证结果、未完成项和风险。

## 6. Cargo 隔离

本仓库所有 `cargo build`、`cargo check`、`cargo test`、`cargo clippy` 都必须使用项目本地 Cargo 缓存，避免 Windows 下共享缓存锁冲突。

PowerShell 中从仓库根目录先设置稳定路径。开发和定向验证默认启用增量编译：

```powershell
$root = (Resolve-Path .).Path
$env:CARGO_HOME = Join-Path $root ".cargo-home"
$env:CARGO_TARGET_DIR = Join-Path $root "target"
$env:CARGO_INCREMENTAL = "1"
```

只有在收尾需要复现 CI 式非增量结果、用户明确要求、或排查增量编译问题时，才临时改为：

```powershell
$env:CARGO_INCREMENTAL = "0"
```

### rust-analyzer 隔离

rust-analyzer 使用独立 `target/rust-analyzer`，手动 Cargo 命令使用根 `target`。两者 target 已分离，运行手动 `cargo build`、`cargo check`、`cargo test`、`cargo clippy` 前不需要先核对 rust-analyzer 或其他 Cargo 进程。VS Code / Devin 建议配置：

```json
{
  "rust-analyzer.cargo.targetDir": "target/rust-analyzer",
  "rust-analyzer.cargo.extraEnv": {
    "CARGO_HOME": "${workspaceFolder}\\.cargo-home",
    "CARGO_INCREMENTAL": "1"
  },
  "rust-analyzer.checkOnSave.enable": true
}
```

如果出现明确的缓存锁、文件占用或异常性能问题，再按需排查具体进程；不要为了常规验证提前核对进程，也不要终止用户的 rust-analyzer 进程。

Cargo 命令执行节奏：

- 实现、阅读代码和普通排查阶段不要反复运行 `cargo build`、`cargo check`、`cargo test` 或 `cargo clippy`；先通过阅读、diff、定向非 Cargo 命令和必要的轻量检查推进。
- 不要把 `cargo test <name>` 误当成“只编译一个测试”；`<name>` 只是运行时过滤。能缩小目标时必须带 `--bin`、`--lib` 或 `--test`。
- 根 `Cargo.toml` 是 `sentra-cli` package；`sentra-lib` 不是 workspace member。验证库代码时使用 `--manifest-path sentra-lib\Cargo.toml`。
- CLI 单元测试优先用：`cargo test --bin sentra <test_name> -j 1`。
- CLI 集成测试优先用：`cargo test --test cli_list <test_name> -j 1`。
- `sentra-lib` 库单元测试优先用：`cargo test --manifest-path sentra-lib\Cargo.toml --lib <test_name> -j 1`。
- `sentra-lib` 指定集成测试优先用：`cargo test --manifest-path sentra-lib\Cargo.toml --test <test_file> <test_name> -j 1`。
- `cargo test --locked --test cli_list sentra_version_flags_print_version -j 1` 仅在收尾验证阶段运行一次，除非用户明确要求立即运行，或当前任务就是诊断该命令本身失败。
- 同一条 Cargo 验证命令成功后，不要在没有相关代码或测试文件变化的情况下重复运行；若需要再次运行，先说明触发原因。

推荐收尾验证按改动面选择，不要默认全量：

```powershell
cargo check -j 1
cargo test --bin sentra <test_name> -j 1
cargo test --test cli_list <test_name> -j 1
cargo test --manifest-path sentra-lib\Cargo.toml --lib <test_name> -j 1
```

不要与其他项目共享 `CARGO_HOME`；`.cargo-home` 仅作本地缓存，不要提交。为避免误判为“每次从头编译”，从仓库根目录设置稳定的 `CARGO_HOME` / `CARGO_TARGET_DIR`；同一会话内不要主动并发启动多条本仓库手动 Cargo 命令。首次完整编译未结束前，中断后再次运行会继续补未完成依赖。

看到其他项目正在运行 `cargo.exe` 或 `rustc.exe` 不代表本仓库未隔离。隔离目标是本项目的 `CARGO_HOME` 和 `CARGO_TARGET_DIR`；`rustc.exe` 来自共享 rustup toolchain，可被多个项目并行调用。只有在排查明确冲突时才查看进程 `CommandLine` 是否指向本仓库路径，不要终止无关项目的编译进程。

## 7. Git 与工作区

- 不回退、覆盖或格式化用户已有改动，除非用户明确要求。
- 工作区有无关改动时保持隔离，只处理本次任务相关文件。
- 禁止使用 `git reset --hard`、`git checkout --` 等破坏性命令，除非用户明确要求并确认目标。
- 提交前确认 diff 只包含本次请求需要的改动。
