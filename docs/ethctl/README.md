# ethctl

`ethctl` inspects Linux Ethernet interfaces, routes, NAT rules, network-pressure
counters, and selected sysctl and ethtool settings. It also generates conservative
tuning output for Kubernetes control-plane, worker, and gateway nodes.

The tool separates settings into two groups:

- Automatically emitted settings have broadly applicable bounds, such as
  kube-proxy-compatible conntrack capacity and gateway forwarding.
- Workload-, topology-, or device-dependent settings are marked with `(?)` in
  reports and emitted only as commented commands. They are never applied by
  `ethctl` without an operator explicitly uncommenting them.

## Requirements

`ethctl` runs on Linux and reads data from procfs, sysfs, rtnetlink, ethtool
netlink, nftables, and iptables. Some information requires root privileges or
the corresponding command-line utility to be installed:

- `iproute2` for neighbor information
- `ethtool`-capable kernel and driver for device settings
- `nftables` and/or `iptables` for NAT and kube-proxy inspection
- root privileges for complete firewall information and for applying generated
  commands

Virtual interfaces may not implement hardware ethtool operations. Unsupported
ring, coalescing, or offload fields are shown as `-` instead of failing the
entire command.

## Build

Build natively on Linux:

```bash
cargo build --release -p ethctl
```

The binary is written to `target/release/ethctl`.

Build Ubuntu 22.04-compatible release archives for both amd64 and arm64:

```bash
make CONTAINER_ENGINE=docker container-release
```

An amd64 Docker host needs ARM64 binfmt/QEMU support before it can execute the
ARM64 builder image:

```bash
docker run --privileged --rm tonistiigi/binfmt --install arm64
docker run --rm --platform linux/arm64 ubuntu:22.04 uname -m
```

The verification command must print `aarch64`.

## Commands

```text
ethctl list
ethctl info  [-p|--profile <profile>] [-o|--output <format>] [-b|--backup <format>]
ethctl link  -n|--name <interface> [-p|--profile <profile>] [-g|--generate [<format>]]
ethctl route [-4|--ipv4 | -6|--ipv6]
ethctl nat   [-c|--chain <filter>]
ethctl stats [-i|--interface <interface>]
```

Use `ethctl <command> --help` for the complete CLI help.

## List interfaces

```bash
ethctl list
```

The interface report includes addresses, MTU, state, link kind, master, driver,
and the physical/virtual interface hierarchy. The interface summary in
`ethctl info` additionally shows MAC address, TX queue length, speed, duplex,
NUMA node, and classified type. Recognized virtual types include bridge, bond,
VLAN, veth, TUN/TAP, VXLAN, WireGuard, and common CNI interfaces.

## Inspect tuning values

```bash
# Worker profile is the default
ethctl info

ethctl info --profile control-plane
ethctl info --profile worker
ethctl info --profile gateway
```

Profile aliases include `cp` and `master` for `control-plane`, and `router` for
`gateway`.

All profiles derive `nf_conntrack_max` from logical CPU count using the
kube-proxy-compatible formula `max(cores × 32768, 131072)`. The gateway profile
also checks IPv4 and IPv6 forwarding.

The following categories remain investigation candidates because their correct
values depend on measured traffic and topology:

- socket buffers and TCP memory bounds
- listen and network-device backlogs
- TCP keepalive and close behavior
- neighbor-table thresholds and ARP behavior
- reverse-path filtering
- MTU and TX queue length
- NIC rings, interrupt coalescing, and offloads

Per-interface `rp_filter` candidates are generated only for physical
interfaces. Global/default `rp_filter` is not changed because the kernel uses
the maximum of the global and per-interface values, which can interfere with
asymmetric CNI or gateway routing.

## Generate tuning output

Supported formats are `cmd`, `conf`, and `script`:

```bash
ethctl info --profile gateway --output cmd
ethctl info --profile worker --output conf
ethctl info --profile control-plane --output script > tune-network.sh
```

The output contains:

1. Un-commented sysctl commands only when an automatically managed value does
   not meet its required bound.
2. Commented sysctl investigation candidates.
3. Commented, capability-aware commands for every physical interface whose
   current setting differs from the profile's starting candidate.

Example device section:

```bash
# Physical-interface candidates (not applied; uncomment after validation):
# ens7:
# ethtool -G ens7 rx 2048 tx 1024
# ip link set dev ens7 txqueuelen 2000
# ethtool -C ens7 rx-usecs 50 tx-usecs 50
```

Combined ring commands include both RX and TX values even when only one side
changes. Unsupported operations and already-matching values are omitted.

Review commented commands using pressure counters, RAM and bandwidth-delay
product, application timeouts, CNI behavior, and the complete network path
before enabling them. In particular, cluster size alone is not sufficient to
select MTU, TX queue length, ring size, or interrupt coalescing.

### Back up current sysctls

```bash
ethctl info --backup cmd
ethctl info --backup conf > network-sysctl-backup.conf
```

Backup supports `cmd` and `conf`; it does not support `script`.

## Inspect one link

```bash
ethctl link --name ens7 --profile gateway
```

The report combines rtnetlink attributes with supported ethtool ring,
coalescing, and offload values.

Generate commented candidates for only that interface:

```bash
ethctl link --name ens7 --profile gateway --generate cmd

# --generate with no value defaults to cmd
ethctl link --name ens7 --profile gateway --generate
```

The generated device commands are always commented because device tuning is
topology- and workload-specific.

## Observe network pressure

```bash
ethctl stats
ethctl stats --interface ens7
```

The global report includes:

- conntrack usage, hash buckets, entries per bucket, insert failures, and drops
- per-CPU softnet processed, dropped, time-squeeze, collision, RPS, and
  flow-limit counters
- TCP/UDP socket allocation and memory counters
- TCP listen overflow, request-queue, SYN-cookie, memory-abort, and time-wait
  overflow counters
- IPv4/IPv6 neighbor occupancy and failed/incomplete entries
- detected kube-proxy nftables, iptables, or IPVS mode and rule counts

With `--interface`, the report also includes RX/TX packets, bytes, errors,
drops, FIFO errors, RX missed/no-handler counters, carrier errors, and
collisions.

Most error counters are cumulative since boot. Compare samples over the same
workload interval before changing a candidate value.

## Inspect routes

```bash
ethctl route
ethctl route --ipv4
ethctl route --ipv6
```

The route table includes destination, gateway, interface hierarchy, metric,
protocol, scope, and route type. `--ipv4` and `--ipv6` are mutually exclusive.

## Inspect NAT

```bash
ethctl nat
ethctl nat --chain POSTROUTING
ethctl nat --chain ts-postrouting
```

`ethctl nat` merges supported nftables and iptables views, follows jumps through
custom chains, deduplicates equivalent rules, and reports SNAT, DNAT,
MASQUERADE, packet, and byte information. The chain filter is case-insensitive
and matches partial names.

## Recommended workflow

```bash
# 1. Capture current pressure and interface state
ethctl stats --interface ens7
ethctl link --name ens7 --profile gateway

# 2. Review safe changes and commented candidates
ethctl info --profile gateway --output cmd

# 3. Apply only the un-commented sysctl lines first
# 4. Load-test, sample stats again, and enable individual candidates only when
#    the measurements justify them
```

Generated output does not configure firewall policy. A gateway operator must
separately ensure that forwarding rules permit only intended traffic.
