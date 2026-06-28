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

The profiles automatically apply only kube-proxy-compatible conntrack capacity
and timeout bounds, plus packet forwarding for the gateway profile. Settings
whose correct value depends on RAM, bandwidth-delay product, application
timeouts, CNI routing, NIC capabilities, or the full network path are shown as
investigation candidates with a `(?)` suffix and are not changed automatically.
This includes socket buffers, listen and device queues, neighbor thresholds,
ARP policy, reverse-path filtering, MTU, ring size, interrupt coalescing, and
offloads.

Generated `cmd`, `conf`, and `script` output includes actionable investigation
candidates as commented-out, syntactically valid settings that must be
explicitly uncommented after validation.

Use `ethctl stats` before changing candidates. It reports conntrack utilization
and hash load, softnet pressure, TCP listen/queue/memory failures, neighbor-table
occupancy and failures, and the detected kube-proxy dataplane/rule count. Use
`ethctl stats --interface <name>` to include standard NIC missed/drop counters.

The `gateway` profile enables IPv4 and IPv6 forwarding and applies only
kube-proxy-compatible conntrack recommendations. Endpoint TCP settings,
firewall policy, MTU, queues, and VPN-specific offloads must be validated for
the deployed routing topology.

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
