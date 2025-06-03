# Release TODOs
**This documentation is aimed at developers.** It collects what to complete before a new release.

`$VERSION` does not refer to an environment variable in this context. It simply means "place your new version string here".

## Version Bump
- [ ] Bump version number in:
    - [ ] `Cargo.toml`
    - [ ] `.gitlab-ci.yml`
    - [ ] `package_fdomat.sh`

## Building packages
- [ ] build ARM/AMD64 with
    ```bash
    cargo bulid -r
    cargo bulid -r --target aarch64-unknown-linux-musl
    ```
- [ ] package each into deb and rpm
    ```bash
    ./package_fdomat deb [path to release folder]
    ./package_fdomat rpm [path to release folder]
    ```
    > there should be 4 artifacts now: ARM/DEB, ARM/RPM, AMD64/DEB, AMD64/RPM
- [ ] sign artifacts (in pwd) : `find . -type f -exec ssh-keygen -Y sign -f KEY -n file '{}' ';'`

## Add release(s)
- [ ] GitHub/GitLab
    - [ ] create annotated commit
        ```bash
        git tag -a v$VERSION
        git push origin v$VERSION
        ```
    - [ ] add release
- [ ] crates.io
    - [ ] assert [metadata-](https://doc.rust-lang.org/cargo/reference/manifest.html) and [API guidelines](https://rust-lang.github.io/api-guidelines/) are fulfilled
    - [ ] test and create release
        ```bash
        cargo publish --dry-run
        cargo publish
        ```
