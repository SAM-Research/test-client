FROM rust:1.86-slim AS builder
ENV SQLX_OFFLINE=true
WORKDIR /test-client
COPY . .

RUN apt update 
RUN apt install -y protobuf-compiler pkg-config libssl-dev

RUN cargo build --release

LABEL org.opencontainers.image.source=https://github.com/SAM-Research/test-client
LABEL org.opencontainers.image.description="SAM/DenIM-on-SAM Test Client image"
LABEL org.opencontainers.image.licenses=MIT


FROM debian:bookworm-slim
RUN apt update && apt install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /test-client/target/release/test-client /test-client


ENTRYPOINT ["/test-client"]