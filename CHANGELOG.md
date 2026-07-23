# Changelog

All notable changes to this project will be documented in this file.

## 0.2.9

- Synced the bundled Sentra library main pointer after the CodeBuddy and Cursor fixes.
- Added CodeBuddy CLI provider discovery from the official `models.json` format.
- Marked the active CodeBuddy CLI provider and model from `settings.json`.
- Fixed CodeBuddy CN IDE cron collection for automation databases without `deleted_at`.
- Improved cron terminal output by showing task names and single-line prompt previews.
- Updated Qoder and CodeBuddy product family discovery and Cursor cron support.
- Renamed Kimi App display metadata to Kimi Work.

## 0.2.6

- Added process asset listing for supported agents.
- Added skill asset path output in terminal lists.
- Updated bundled Sentra library process asset specification.
- Improved local Cargo cache and rust-analyzer isolation documentation.

## 0.2.5

- Added installed-agent discovery when an agent home has not been initialized yet.
- Added Codex desktop app detection alongside Codex CLI detection.
- Updated agent list tests to tolerate additional installed agents discovered in CI.

## 0.2.4

- Added Agent installation status display and configurable install/uninstall flows.
- Added uninstall confirmation for configuration data cleanup.
- Synced the bundled Sentra library changes from the rs branch.
- Documented local Cargo cache isolation for repository collaboration.

## 0.2.1

- Added pre-command self-update prompts for outdated Sentra CLI installs.
- Improved model gateway detail display in the TUI.
- Updated bundled Sentra library integration and repository maintenance assets.

## 0.1.0

- Added the initial Sentra CLI release for local AI Agent asset discovery and risk scanning.
- Added bundled YARA, hash, and TI rule support.
- Added JSON output for scan automation.
