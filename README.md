# Open Network Management

Open Network Management (ONM) provides focused command-line tools for host,
fabric, accelerator, and Kubernetes network operations.

## Command-line tools

| Command | Purpose | Start here | Guide |
| --- | --- | --- | --- |
| `ethctl` | Inspect Ethernet interfaces, routes, NAT, pressure, and tuning candidates | `ethctl list` | [ethctl guide](docs/ethctl/README.md) |
| `hcactl` | List host channel adapters and their ports | `hcactl list` | [hcactl guide](docs/hcactl/README.md) |
| `smctl` | Inspect and manage NVIDIA UFM subnet-manager partitions | `smctl list` | [smctl guide](docs/smctl/README.md) |
| `xpuctl` | Discover and inspect XPU/BMC devices through Redfish | `xpuctl list` | [xpuctl guide](docs/xpuctl/README.md) |
| `kprobe` | Check every directed Kubernetes node-to-node pod-network path | `kprobe` | [kprobe guide](docs/kprobe/README.md) |
| `tsctl` | Inspect Tailscale tailnets and devices through the REST API | `tsctl list -n -` | [tsctl guide](docs/tsctl/README.md) |

Run `<command> --help` for CLI syntax. Each linked guide contains the tool's
requirements, configuration, command reference, examples, and operational
notes where applicable.

## Build

```bash
cargo build --release --workspace
```

The command binaries are written to `target/release/`. Some host-management
tools require Linux system libraries; see the relevant guide before building.

## Library

`libonm` contains the shared Rust APIs used by the command-line tools.

## Container shell

ONM also provides a privileged troubleshooting container with the compiled
tools and common network utilities. See the
[onm-shell guide](docs/onm-shell/README.md).
