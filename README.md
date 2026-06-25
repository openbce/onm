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

# Show interface details and sysctl settings
ethctl info -n eth0

# Show sysctl tuning (default: worker profile)
ethctl sysctl

# Show control-plane tuning for 10k-node cluster
ethctl sysctl --profile control-plane

# Generate sysctl commands
ethctl sysctl -g

# Generate sysctl.conf format
ethctl sysctl -g conf

# Generate tuning script for control-plane
ethctl sysctl -p control-plane -g script > tune-sysctl.sh
```

## onm-shell

```bash
# Build
docker build -t openbce/onm-shell .

# Run (privileged + host network for device access)
docker run -it --rm --privileged --network host openbce/onm-shell bash
```