FROM node:24-bookworm-slim@sha256:3638d9a6fe4030bd716be989438248074489337ba3275657f93595428be4fc03 AS web-builder

WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci --ignore-scripts
COPY web/tsconfig.json web/tsconfig.build.json web/vitest.config.ts ./
COPY web/src ./src
COPY web/tests ./tests
RUN npm run check
RUN npm test
RUN npm run build

FROM mcr.microsoft.com/playwright:v1.62.1-noble@sha256:dcc5531e97840b9b5e794f2814476b21571c5124a3fca2267d73041f56e7580e AS browser-test

WORKDIR /work/web
COPY web/package.json web/package-lock.json ./
RUN npm ci --ignore-scripts
COPY web/playwright.config.ts ./
COPY web/e2e ./e2e
ENTRYPOINT ["npm", "run", "e2e"]

FROM rust:1.97.1-bookworm@sha256:14bc9c5966e7b3a385794b3d5389a8765668342025fbcc7b2e3d2866ac4bd8c3 AS builder

WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps ./apps
COPY crates ./crates
COPY rules ./rules

ARG VDA5050_BUILD_COMMIT=UNVERIFIED-DEMO-CONTAINER
ENV VDA5050_BUILD_COMMIT=${VDA5050_BUILD_COMMIT}
RUN cargo build --release --locked \
    --bin vda5050-demo \
    --bin vda5050-doctor \
    --bin vda5050-fleet-actor \
    --bin vda5050-lsmart-bridge \
    --bin vda5050-lsmart-plan \
    --bin vda5050-lsmart-recorder \
    --bin vda5050-live-plan \
    --bin vda5050-robot-sim \
    --bin vda5050-trace-recorder \
    --bin vda5050-web-gateway

FROM debian:bookworm-slim@sha256:abd67ffcfa541b485a3dff59865ab629aa048a6c613e639d36e7456b0b229241 AS runtime

COPY --from=builder /src/target/release/vda5050-demo /usr/local/bin/vda5050-demo
COPY --from=builder /src/target/release/vda5050-doctor /usr/local/bin/vda5050-doctor
COPY --from=builder /src/target/release/vda5050-fleet-actor /usr/local/bin/vda5050-fleet-actor
COPY --from=builder /src/target/release/vda5050-lsmart-bridge /usr/local/bin/vda5050-lsmart-bridge
COPY --from=builder /src/target/release/vda5050-lsmart-plan /usr/local/bin/vda5050-lsmart-plan
COPY --from=builder /src/target/release/vda5050-lsmart-recorder /usr/local/bin/vda5050-lsmart-recorder
COPY --from=builder /src/target/release/vda5050-live-plan /usr/local/bin/vda5050-live-plan
COPY --from=builder /src/target/release/vda5050-robot-sim /usr/local/bin/vda5050-robot-sim
COPY --from=builder /src/target/release/vda5050-trace-recorder /usr/local/bin/vda5050-trace-recorder
COPY --from=builder /src/target/release/vda5050-web-gateway /usr/local/bin/vda5050-web-gateway
COPY --from=web-builder /web/dist /web

USER 65532:65532
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/vda5050-demo"]
