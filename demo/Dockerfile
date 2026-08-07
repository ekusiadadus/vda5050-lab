FROM rust:1.97.1-bookworm@sha256:14bc9c5966e7b3a385794b3d5389a8765668342025fbcc7b2e3d2866ac4bd8c3 AS builder

WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps ./apps
COPY crates ./crates
COPY rules ./rules

ARG VDA5050_BUILD_COMMIT=UNVERIFIED-DEMO-CONTAINER
ENV VDA5050_BUILD_COMMIT=${VDA5050_BUILD_COMMIT}
RUN cargo build --release --locked --bin vda5050-demo --bin vda5050-doctor

FROM debian:bookworm-slim@sha256:abd67ffcfa541b485a3dff59865ab629aa048a6c613e639d36e7456b0b229241 AS runtime

COPY --from=builder /src/target/release/vda5050-demo /usr/local/bin/vda5050-demo
COPY --from=builder /src/target/release/vda5050-doctor /usr/local/bin/vda5050-doctor

USER 65532:65532
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/vda5050-demo"]
