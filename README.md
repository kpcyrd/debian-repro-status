# debian-repro-status

A CLI tool for querying the [reproducibility](https://reproducible-builds.org/) status of the Debian packages using data from a [rebuilderd](https://github.com/kpcyrd/rebuilderd) instance such as [reproduce.debian.net](https://reproduce.debian.net/).

This code is heavily inspired and partially yoinked from [arch-repro-status](https://gitlab.archlinux.org/archlinux/arch-repro-status) authored by [Orhun Parmaksız](https://github.com/orhun).

## Installation

### Debian

(Well not yet)

```sh
apt-get install debian-repro-status
```

### crates.io

```sh
cargo install debian-repro-status
```

## Usage

```sh
debian-repro-status
```

## Example output

```
...
[+] logsave amd64 1.47.2~rc1-2 GOOD
[+] mawk amd64 1.3.4.20240905-1 GOOD
[+] mount amd64 2.40.2-12 GOOD
[+] ncurses-base all 6.5-2 GOOD
[?] ncurses-bin amd64 6.5-2+b1 UNKWN
[?] openssl-provider-legacy amd64 3.3.2-2 UNKWN
[?] passwd amd64 1:4.16.0-7 UNKWN
[+] perl-base amd64 5.40.0-8 GOOD
[+] sed amd64 4.9-2 GOOD
[?] sysvinit-utils amd64 3.11-1 UNKWN
[+] tar amd64 1.35+dfsg-3 GOOD
[?] tzdata all 2024b-4 UNKWN
[+] usr-is-merged all 39 GOOD
[+] util-linux amd64 2.40.2-12 GOOD
[?] zlib1g amd64 1:1.3.dfsg+really1.3.1-1+b1 UNKWN
 INFO  debian-repro-status > 44/90 packages are not reproducible.
 INFO  debian-repro-status > Your system is 51.11% reproducible.
```

## License

[The MIT License](https://opensource.org/licenses/MIT)
