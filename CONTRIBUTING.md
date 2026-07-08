# Contributing

Thank you for helping improve Sentra CLI.

## Development Setup

Install the stable Rust toolchain:

```bash
rustup default stable
```

Build and test the project:

```bash
cargo build --locked --all-targets
cargo test --locked --all-targets
```

Run formatting before opening a pull request:

```bash
cargo fmt --all -- --check
```

## Pull Requests

- Keep changes focused on one problem.
- Add or update tests when behavior changes.
- Update `README.md` and `README.en.md` when user-facing commands or behavior change.
- Keep user-facing text localizable and avoid hard-coded UI strings in new code.

## Commit Messages

Use concise Chinese commit messages when this repository generates commits automatically:

```text
修复: 解决扫描输出路径错误
功能: 添加 GitHub 自动编译流程
```
