# archlinux-scan-malloc-usable-size

Takes a path to a local Arch Linux mirror and concurrently scans all packages for ELF binaries mentioning a specific symbol: `malloc_usable_size`

```
cargo run --release -- /srv/archlinux-mirror/
```

The `malloc_usable_size` function is only supposed to be used for diagnostic purposes and crashes the program when used in programs built with `-D_FORTIFY_SOURCE=3`. Programs that make use of this function need to be patched or built with `-D_FORTIFY_SOURCE=2` instead.

Further reading:

- https://gitlab.archlinux.org/archlinux/rfcs/-/blob/master/rfcs/0017-increase-fortification-level.rst
- https://developers.redhat.com/articles/2022/09/17/gccs-new-fortification-level
