# narrowssh

An SSH forced-command configuration manager

Scripts sometimes need SSH access for  tasks such as deployment or maintenance.
Because configuring  proper limitations can be cumbersome,  narrowssh exists to
streamline and centralize securing automated remote access.

narrowssh manages a section of `authorized_keys` and issues restricted SSH keys
to transparently reduce the attack surface of hosts.

## Usage

_TBD_

## Build

This is a [Rust](https://rust-lang.org) project managed with
[Cargo](https://doc.rust-lang.org/cargo/).  Ensure that `cargo` is available in
`PATH` before proceeding
(see [installation methods](https://www.rust-lang.org/tools/install)).

```sh
git clone https://github.com/OLEGSHA/narrowssh.git

cd narrowssh
cargo build
```

See
[The Cargo Book](https://doc.rust-lang.org/cargo/commands/index.html)
for more useful commands.
This project uses
[rustfmt](https://github.com/rust-lang/rustfmt) (`cargo fmt`)
as a formatter and
[clippy](https://github.com/rust-lang/rust-clippy) (`cargo clippy`)
as a linter.

## License
narrowssh is licensed under GPL-3.0-or-later.
See [LICENSE](LICENSE) for details.
