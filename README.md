# Open Network Management

## libonm

The lib for open network management.

## xpuctl

The command line to manage XPU.

## hcactl

The command line to manage HCA of host.

## smctl

The command line to manage subnet manager.

## ethctl

The command line to manage Ethernet interfaces and network sysctl tuning.

```bash
# List interfaces
ethctl list

# Show interface details and suggested tuning values
ethctl info

# Show link and ethtool settings
ethctl link --name eth0

# Show control-plane tuning for 10k-node cluster
ethctl info --profile control-plane

# Show routing and forwarding checks for a gateway
ethctl info --profile gateway

# Generate sysctl commands
ethctl info --output cmd

# Generate sysctl.conf format
ethctl info --output conf

# Generate tuning script for control-plane
ethctl info --profile control-plane --output script > tune-network.sh

# Show routes or NAT rules
ethctl route
ethctl nat
```

The profiles use kube-proxy-compatible, CPU-derived conntrack capacity. Settings
whose correct value depends on RAM, CNI routing, NIC capabilities, or the full
network path are shown as investigation candidates with a `(?)` suffix and are
not changed automatically. This
includes UDP memory pools, socket defaults, ARP policy, reverse-path filtering,
MTU, queue length, interrupt coalescing, and offloads. Validate those settings
with workload measurements before applying device-specific candidates from
`ethctl link`.

Generated `cmd`, `conf`, and `script` output includes actionable investigation
candidates as commented-out, syntactically valid settings that must be
explicitly uncommented after validation.

The `gateway` profile enables IPv4 and IPv6 forwarding and applies only
forwarding-relevant conntrack and receive-backlog recommendations. Endpoint TCP
settings remain observational, and firewall policy, MTU, and VPN-specific
offloads must be validated for the deployed routing topology.

## onm-shell

```bash
# Build
docker build -t openbce/onm-shell .

# Run as daemon (privileged + host network for device access)
docker run -d --name onm-shell --privileged --network host openbce/onm-shell

# Enter shell
docker exec -it onm-shell bash

# Stop and remove
docker stop onm-shell && docker rm onm-shell
```
