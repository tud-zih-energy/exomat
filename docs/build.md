# Build from source
## Build for x86_64 (default)
```bash
# 1. compile the exomat
$ cargo bulid -r
# 2. create deb package (use absolute path)
$ ./package_exomat deb $HOME/experiment-o-mat/target/release
# 3. install deb package
$ apt install ./exomat-0.1.0-1_amd64.deb
```

## Build for aarch64, Rocky Linux
```bash
# 1. edit cargo config, TODO: find a way to get rid of this part
$ cat ~/.cargo/config
[target.aarch64-unknown-linux-musl]
linker  = "aarch64-linux-gnu-gcc"
rustflags = ["-L", "/usr/lib/gcc-cross/aarch64-linux-gnu/12/"]
# 2. compile the exomat
$ cargo bulid -r --target aarch64-unknown-linux-musl
# 3. create rpm package (use absolute path)
$ ./package_exomat rpm $HOME/experiment-o-mat/target/aarch64-unknown-linux-musl/release

# 4. install rpm package (on target system)
$ dnf install [path to rpm]/exomat-0.1.0-1_aarch64.rpm
```
Dependencies:
- rpm


### TODOs
- automatic version parsing for fpm
