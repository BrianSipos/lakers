# Lakers C
C bindings for the `lakers` crate.

The `./build.sh` script will:
1. build this library wrapper as a static library
2. generate C headers
3. zip all artifacts together

The results will be available in the `target` folder.

# Examples
To build with the PSA backend, to run in the host:

```bash
./build.sh crypto-psa
```

To build with the CC310 backend, to run in an embedded device:

```bash
./build.sh crypto-cryptocell310
```
