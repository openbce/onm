FROM ubuntu:24.04

RUN apt-get update && apt-get -y install \
    tcpdump iproute2 net-tools bridge-utils ipmitool \
    build-essential protobuf-compiler libudev-dev pkg-config libclang-dev libibverbs-dev libpci-dev \
    libcairo2-dev libgirepository1.0-dev python3 python3-pip python3-gi network-manager-dev libibumad-dev libibmad-dev \
    git vim curl pciutils apt-transport-https ca-certificates jq

# Install kubectl (latest stable)
RUN curl -fsSL https://pkgs.k8s.io/core:/stable:/v1.33/deb/Release.key | \
    gpg --dearmor -o /etc/apt/keyrings/kubernetes-apt-keyring.gpg
RUN echo 'deb [signed-by=/etc/apt/keyrings/kubernetes-apt-keyring.gpg] https://pkgs.k8s.io/core:/stable:/v1.33/deb/ /' | \
    tee /etc/apt/sources.list.d/kubernetes.list

RUN apt-get update && apt-get install -y kubectl

# Install latest stable Rust and Cargo
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y -q --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

# Install tools
RUN cargo install --git https://github.com/openbce/onm smctl
RUN cargo install --git https://github.com/openbce/onm hcactl
RUN cargo install --git https://github.com/openbce/onm xpuctl
RUN cargo install --git https://github.com/openbce/onm ethctl

ENTRYPOINT ["sh", "-c", "exec tail -f /dev/null"]