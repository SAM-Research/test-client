# Denim SAM Test client

# SAM Dependencies

to update sam dependencies just do:

```sh
cargo update -p sam-server
```

you might need to change `sam-server` to either one of the other sam projects

# Docker

Building the `test-client` docker image:

```sh
docker build -t test-client .
```
