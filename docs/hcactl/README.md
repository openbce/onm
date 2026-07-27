# hcactl

`hcactl` lists host channel adapters (HCAs), PCI placement, firmware and board
information, and InfiniBand or Ethernet port state.

## Requirements

`hcactl` runs on Linux and requires libibverbs, libudev, libpci, Clang, and
pkg-config development files. On Ubuntu:

```bash
sudo apt install -y \
  libclang-dev \
  libibverbs-dev \
  libudev-dev \
  libpci-dev \
  pkg-config
```

## Build

```bash
cargo build --release -p hcactl
```

The binary is written to `target/release/hcactl`.

## Usage

```bash
hcactl list
```

## Example

```text
hcactl list
----------------------------------------------
ID             : 15B3:0009
Model          : MT43244 BlueField-3 integrated ConnectX-7 network controller
Vendor         : Mellanox Technologies
FW             : 32.38.1002
Board          : MT_0000000884

    Name           Slot           Node GUID                Port GUID                LID            Subnet                   LinkType       State          PhysState      
    mlx5_2         0000:02:00.0   946d:ae03:0051:9774      -                        0              -                        Eth            Active         LinkUp         
    mlx5_3         0000:02:00.1   946d:ae03:0051:9775      -                        0              -                        Eth            Active         LinkUp         


----------------------------------------------
ID             : 15B3:0054
Model          : MT2910 Family [ConnectX-7]
Vendor         : Mellanox Technologies
FW             : 28.98.2400
Board          : MT_0000000894

    Name           Slot           Node GUID                Port GUID                LID            Subnet                   LinkType       State          PhysState      
    mlx5_0         0000:c1:00.0   e8eb:d303:0098:2ebc      e8eb:d303:0098:2ebc      65535          fe80:0000:0000:0000      IB             Down           Disabled       
    mlx5_1         0000:c1:00.1   e8eb:d303:0098:2ebd      e8eb:d303:0098:2ebd      65535          fe80:0000:0000:0000      IB             Down           Disabled       
```
