# Release TODOs
**This documentation is aimed at developers.** It collects what to complete before a new release.

`$VERSION` does not refer to an environment variable in this context. It simply means "place your new version string here".

## Version Bump
- [ ] Bump version number in:
    - [ ] `Cargo.toml`
- [ ] Update cargo: `cargo update`
- [ ] Run update cargo **a second time** to see breaking updates: `cargo update --verbose`
    - apply updates by hand (perhaps incrementally)
- [ ] Actually build: `cargo build`
    - note: This updates `Cargo.lock`, so do it!
- [ ] Update nix: `nix flake update` (optional, but recommended when building binaries)
- [ ] Commit

## Building Binaries (optional)
```bash
nix flake update
nix build .#exomat_all_archs -L
```

> Takes a while, may build a musl cross-compiler.

## Add release(s)
- [ ] GitHub
    - [ ] create annotated commit
        ```bash
        git tag -a v$VERSION
        git push origin v$VERSION
        ```
    - [ ] add release in web frontend
      - [ ] upload built packages
- [ ] publish to crates.io: `cargo publish`
    - [intro in rust book](https://doc.rust-lang.org/book/ch14-02-publishing-to-crates-io.html)
