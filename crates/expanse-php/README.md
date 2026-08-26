# Expanse PHP Native Extension (`orieg/expanse-extension`)

Native PHP Zend extension for **Expanse** — the clean-room, pure-Rust implementation of Judy arrays and 256-ary digital tries — built with `ext-php-rs`. It statically embeds the expanse core engine: no separate `libexpanse` shared library is required at runtime.

This is the extension half of the two-package convention: install this for maximum throughput, or use the userland library [`orieg/expanse`](https://packagist.org/packages/orieg/expanse) (`composer require orieg/expanse`), which negotiates between this extension and a portable `\FFI` fallback driver at runtime.

## Installation

```bash
pie install orieg/expanse-extension
```

`pie install` compiles the extension from Rust source on your machine, so a working [Rust toolchain](https://rustup.rs) is required at install time. No prebuilt binaries are shipped.

Alternatively, build from the monorepo:

```bash
cargo build --release -p expanse-php
# Add to php.ini: extension=expanse.so (or libexpanse_php.dylib / expanse_php.dll)
```

## Documentation

This repository is a read-only subtree mirror of [`crates/expanse-php`](https://github.com/orieg/expanse/tree/main/crates/expanse-php) in the [orieg/expanse](https://github.com/orieg/expanse) monorepo — file issues and pull requests there.

Full PHP API documentation: [docs/bindings/php.md](https://github.com/orieg/expanse/blob/main/docs/bindings/php.md).

## License

Dual-licensed under [MIT](https://github.com/orieg/expanse/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/orieg/expanse/blob/main/LICENSE-APACHE), at your option.
