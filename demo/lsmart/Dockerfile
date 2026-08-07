# syntax=docker/dockerfile:1.7

# LSMART publishes no container image and its pinned source revision has no
# root license file. This image is therefore a local-only build from an
# explicit external checkout. Do not push or redistribute it.
FROM --platform=linux/amd64 ubuntu:22.04 AS build

ARG TARGETARCH
RUN test "$TARGETARCH" = "amd64"

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        cmake \
        freeglut3-dev \
        git \
        libboost-all-dev \
        libeigen3-dev \
        libfreeimage-dev \
        libfreeimageplus-dev \
        libgraphviz-dev \
        liblua5.3-dev \
        libqt5opengl5-dev \
        libspdlog-dev \
        libtbb-dev \
        libtiff-dev \
        libxi-dev \
        libxmu-dev \
        lua5.3 \
        pkg-config \
        python3 \
        python3-pip \
        qtbase5-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=argos_source / /opt/argos3
RUN cmake -S /opt/argos3/src -B /tmp/argos-build \
        -DCMAKE_BUILD_TYPE=Release \
        -DARGOS_DOCUMENTATION=OFF \
        -DARGOS_INSTALL_LDSOCONF=OFF \
    && cmake --build /tmp/argos-build --parallel 2 \
    && cmake --install /tmp/argos-build \
    && printf '%s\n' /usr/local/lib/argos3 > /etc/ld.so.conf.d/argos3.conf \
    && ldconfig

COPY --from=lsmart_source / /usr/project
COPY demo/lsmart/patches/0001-vda5050-causal-bridge.patch /tmp/lsmart-vda5050.patch
COPY demo/lsmart/patches/0002-ubuntu22-spdlog-ostream-compat.patch /tmp/lsmart-spdlog.patch
RUN cd /usr/project \
    && test "$(git rev-parse HEAD)" = "ce0a020d8da10a806b23ffa3ccb56b1affff57d1" \
    && test ! -e LICENSE \
    && test ! -e LICENSE.md \
    && git apply --check /tmp/lsmart-vda5050.patch \
    && git apply --check /tmp/lsmart-spdlog.patch \
    && git apply /tmp/lsmart-vda5050.patch \
    && git apply /tmp/lsmart-spdlog.patch

RUN python3 -m pip install --no-cache-dir -r /usr/project/requirement.txt
RUN cmake -S /usr/project/client/externalDependencies/rpclib \
        -B /usr/project/client/externalDependencies/rpclib/build \
        -DCMAKE_BUILD_TYPE=Release \
    && cmake --build /usr/project/client/externalDependencies/rpclib/build --parallel 2 \
    && cmake --install /usr/project/client/externalDependencies/rpclib/build \
    && ldconfig
RUN cmake -S /usr/project/client -B /usr/project/client/build \
        -DCMAKE_BUILD_TYPE=Release \
    && cmake --build /usr/project/client/build --parallel 2
RUN cmake -S /usr/project/server -B /usr/project/server/build \
        -DCMAKE_BUILD_TYPE=Release \
    && cmake --build /usr/project/server/build --parallel 2
RUN cmake -S /usr/project/planner/RHCR -B /usr/project/planner/RHCR/build \
        -DCMAKE_BUILD_TYPE=Release \
    && cmake --build /usr/project/planner/RHCR/build --parallel 2

COPY demo/lsmart/run-official-lsmart.sh /usr/local/bin/run-official-lsmart
RUN chmod 0555 /usr/local/bin/run-official-lsmart \
    && ldconfig -p | grep -q 'libargos3core_simulator.so' \
    && test -x /usr/project/server/build/ExecutionManager \
    && test -x /usr/project/planner/RHCR/build/lifelong \
    && test -f /usr/project/client/build/controllers/footbot_diffusion/libfootbot_diffusion.so

ENV OPENBLAS_NUM_THREADS=1 \
    PYTHONPATH=/usr/project
WORKDIR /runtime
ENTRYPOINT ["/usr/local/bin/run-official-lsmart"]
