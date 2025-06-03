# Release TODOs
**This documentation is aimed at developers.** It collects what to complete before a new release.

`$VERSION` does not refer to an environment variable in this context. It simply means "place your new version string here".

## Version Bump
- [ ] Bump version number in:
    - [ ] `Cargo.toml`

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
