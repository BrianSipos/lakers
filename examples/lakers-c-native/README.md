# Lakers C Native

Example of `lakers` in C for non-embedded targets.

Compile and run:
```
make && ./lakers_c_native
```

Compile and run, leveraging the [lake-authz draft](https://www.ietf.org/archive/id/draft-ietf-lake-authz-00.html) for zero-touch device enrollment:
```
make LAKERS_EAD=authz && ./lakers_c_native
```

# Requirements

- See the README in the `lakers-c` crate.
- Install [libcoap](https://libcoap.net/install.html):
  - tested with the following configuration: `./configure --disable-doxygen --disable-manpages --disable-dtls --disable-oscore`
