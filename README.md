# fift &emsp; [![crates-io-batch]][crates-io-link] [![docs-badge]][docs-url] [![rust-version-badge]][rust-version-link] [![workflow-badge]][workflow-link]

[crates-io-batch]: https://img.shields.io/crates/v/fift.svg

[crates-io-link]: https://crates.io/crates/fift

[docs-badge]: https://docs.rs/fift/badge.svg

[docs-url]: https://docs.rs/fift

[rust-version-badge]: https://img.shields.io/badge/rustc-1.65+-lightgray.svg

[rust-version-link]: https://blog.rust-lang.org/2022/11/03/Rust-1.65.0.html

[workflow-badge]: https://img.shields.io/github/actions/workflow/status/broxus/fift/master.yml?branch=master

[workflow-link]: https://github.com/broxus/fift/actions?query=workflow%3Amaster

> Status: WIP

## About

Rust implementation of the Fift esoteric language.

## Installation

```bash
curl https://sh.rustup.rs -sSf | sh
cargo install --locked fift-cli
```

## Usage

```
Usage: fift [<source_files...>] [-n] [-i] [-I <include>] [-L <lib>]

A simple Fift interpreter. Type `bye` to quie, or `words` to get a list of all commands

Positional Arguments:
  source_files      a list of source files to execute (stdin will be used if
                    empty)

Options:
  -n, --bare        do not preload standard preamble file `Fift.fif`
  -i, --interactive force interactive mode even if explicit source file names
                    are indicated
  -I, --include     sets color-separated library source include path. If not
                    indicated, $FIFTPATH is used instead
  -L, --lib         sets an explicit path to the library source file. If not
                    indicated, a default one will be used
  --help            display usage information
  -v, --version     print version information and exit
  -s                script mode: use first argument as a fift source file and
                    import remaining arguments as $n
```

## Contributing

We welcome contributions to the project! If you notice any issues or errors, feel free to open an issue or submit a pull request.

## License

* The `fift` and `fift-proc` library crates are licensed under either of
  * Apache License, Version 2.0 ([/LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
  * MIT license ([/LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

  at your option.

* The `fift-cli` binary crate is licensed under
  * GNU Lesser General Public License v2.1 ([/cli/LICENSE](./cli/LICENSE) or <https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html>)
