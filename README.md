
<h1 align="center">
	🚧 rbx-configs 🔧
</h1>

<div align="center">

[![Version](https://img.shields.io/crates/v/rbx-configs.svg?style=flat-square)](https://github.com/outofbears/rbx-configs/releases)
[![License](https://img.shields.io/github/license/outofbears/rbx-configs.svg?style=flat-square)](https://github.com/outofbears/rbx-configs/blob/main/LICENSE.md)
[![Stars](https://img.shields.io/github/stars/outofbears/rbx-configs.svg?style=flat-square)](https://github.com/outofbears/rbx-configs/stargazers)
[![Forks](https://img.shields.io/github/forks/outofbears/rbx-configs.svg?style=flat-square)](https://github.com/outofbears/rbx-configs/network/members)
[![Watchers](https://img.shields.io/github/watchers/outofbears/rbx-configs.svg?style=flat-square)](https://github.com/outofbears/rbx-configs/watchers)
[![Issues](https://img.shields.io/github/issues/outofbears/rbx-configs.svg?style=flat-square)](https://github.com/outofbears/rbx-configs/issues)
[![Pull Requests](https://img.shields.io/github/issues-pr/outofbears/rbx-configs.svg?style=flat-square)](https://github.com/outofbears/rbx-configs/pulls)
[![Last Commit](https://img.shields.io/github/last-commit/outofbears/rbx-configs.svg?style=flat-square)](https://github.com/outofbears/rbx-configs/commits/main)


</div>

<br />

`rbx-configs` is a lightweight CLI for downloading, editing, and publishing Roblox Universe Configs (feature flags/experiments) via the official Universe Configs Web API. It helps teams keep configs in version control, batch-update flags safely, and manage drafts.



## ✨ Features

- **Download**: Export all universe configs to a local JSON file.
- **Upload**: Read a local JSON and update only changed flags.
- **Draft control**: Discard or publish staged changes.

## 📦 Installation

If the crate is published to crates.io, you can install with:

```bash
cargo install rbx-configs
```

You need Rust (stable) and Cargo.

- From source (in this repo):

```bash
cargo build --release
# Binary at target/release/rbx-configs
```

## 🔐 Authentication

rbx-configs calls Roblox APIs that require the `.ROBLOSECURITY` cookie.

- Preferred: if `RBX_COOKIE` is not set, rbx-configs will attempt to read your Roblox cookie via the `rbx_cookie` helper.
- Fallback: set the `RBX_COOKIE` environment variable to your cookie value.

Windows PowerShell example:

```powershell
$env:RBX_COOKIE = "<your .ROBLOSECURITY value>"
```

## 🚀 Usage

All commands require a universe id (`-u, --universe-id`). You may also specify a file path (`-f, --file`) which defaults to `config.json`.

Place `-u` and `-f` before the subcommand (e.g., `download`, `upload`, `draft`).

### 📥 Download configs

Export the current universe configs to a local file.

```bash
rbx-configs -u 123456 -f config.json download
```

Output file format:

```json
{
  "FeatureA": {
    "description": "Enables feature A",
    "value": true
  },
  "ExperimentBucket": {
    "description": "Bucket size",
    "value": 10
  }
}
```

### 📤 Upload configs

Read a local JSON and apply only changes (new or updated flags). Existing flags with identical values are ignored.

```bash
rbx-configs -u 123456 -f config.json upload
```

### 🗂️ Manage drafts

Discard or publish staged changes explicitly.

```bash
# Discard staged changes
rbx-configs -u 123456 draft discard

# Publish staged changes
rbx-configs -u 123456 draft publish
```

## 🧩 Configuration file schema

The local JSON uses a simple map keyed by flag name:

- `description`: optional string
- `value`: any valid JSON value (`string`, `number`, `boolean`, or `array`)

Example with nested value:

```json
{
  "CompositeFlag": {
    "description": "A array JSON payload",
    "value": ["1", "2", "3", "4", "..."]
  }
}
```

## 🔧 Logging & environment

- Set `RUST_LOG` to control verbosity (defaults to `rbx_config=debug` in debug builds, `rbx_config=info` in release):

```bash
RUST_LOG=rbx_config=debug rbx-configs -u 123456 download
```

- Optional: a `.env` file is loaded if present for `RBX_COOKIE` or other environment variables.

## 🧰 Troubleshooting

- **403 Forbidden / CSRF errors**: Ensure `RBX_COOKIE` is valid and not expired; try re‑setting it.
- **Rate limit**: The client backs off automatically; you may need to wait.
- **Invalid config JSON**: rbx-configs will log parse errors—verify your file conforms to the schema above.

## 💖 Contribution

rbx-configs was developed by [@Bear](https://github.com/OutOfBears)

## 📄 License

This project is licensed under the MIT License - see the [`LICENSE`](LICENSE.md) file for details.