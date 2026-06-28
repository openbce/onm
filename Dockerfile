# =============================================================================
# Stage: builder - Build environment for onm tools
# =============================================================================
FROM ubuntu:22.04 AS builder

RUN apt-get update && apt-get -y install \
    build-essential protobuf-compiler libudev-dev pkg-config libclang-dev libibverbs-dev libpci-dev \
    libibumad-dev libibmad-dev git curl ca-certificates

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y -q --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /workspace
COPY . .
RUN cargo build --locked --release --workspace

# =============================================================================
# Stage: onm-shell - Runtime shell with pre-installed tools
# =============================================================================
FROM builder AS onm-shell

RUN apt-get update && apt-get -y install \
    tcpdump iproute2 net-tools bridge-utils ipmitool nftables \
    libcairo2-dev libgirepository1.0-dev python3 python3-pip python3-gi network-manager-dev \
    vim pciutils apt-transport-https jq gnupg

RUN curl -fsSL https://pkgs.k8s.io/core:/stable:/v1.33/deb/Release.key | \
    gpg --dearmor -o /etc/apt/keyrings/kubernetes-apt-keyring.gpg && \
    echo 'deb [signed-by=/etc/apt/keyrings/kubernetes-apt-keyring.gpg] https://pkgs.k8s.io/core:/stable:/v1.33/deb/ /' | \
    tee /etc/apt/sources.list.d/kubernetes.list && \
    apt-get update && apt-get install -y kubectl

RUN install -m 0755 \
    /workspace/target/release/smctl \
    /workspace/target/release/hcactl \
    /workspace/target/release/xpuctl \
    /workspace/target/release/ethctl \
    /usr/local/bin/

ENTRYPOINT ["sh", "-c", "exec tail -f /dev/null"]
