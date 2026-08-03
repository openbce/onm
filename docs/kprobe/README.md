# kprobe

`kprobe` validates the pod network between Kubernetes nodes. It creates one
temporary probe pod on every eligible Linux node and checks every directed
source-to-destination path. Each path first verifies TCP connectivity with
`agnhost`, then measures bandwidth with `iperf3`. Cordoned / unschedulable
nodes are skipped.

## Build

```bash
cargo build --release -p kprobe
```

The binary is written to `target/release/kprobe`.

## Usage

```bash
kprobe [OPTIONS]
```

Useful options:

```text
-n, --namespace <NAMESPACE>         Namespace (default: onm-system)
-l, --selector <SELECTOR>           Extra node labels: key=value[,key=value...]
-c, --concurrency <CONCURRENCY>     Simultaneous pod exec requests (default: 16)
-t, --timeout <TIMEOUT>             Per-connect timeout (default: 5s)
    --bandwidth-time <DURATION>     iperf3 test duration per path (default: 3s, min 1s)
    --bandwidth-image <IMAGE>       iperf3 image (default: networkstatic/iperf3:multiarch)
    --ip-family <IP-FAMILY>         Pod address family: ipv4 or ipv6 (default: ipv4)
    --ready-timeout <TIMEOUT>       DaemonSet readiness timeout (default: 2m)
    --image <IMAGE>                 Agnhost image for connectivity checks
```

Use `--selector` to limit which nodes receive probe pods. Labels are merged into
the DaemonSet `nodeSelector` alongside the built-in `kubernetes.io/os=linux`
requirement. For example:

```bash
kprobe --selector kubernetes.io/hostname=node-a
kprobe -l topology.kubernetes.io/zone=zone-1,node-role.kubernetes.io/worker=
```

The tool uses only the active context from the local kubeconfig. It does not
invoke `kubectl`, select cloud-provider profiles, or contain provider-specific
authentication logic. Authentication is delegated to whatever mechanism the
current context defines. Cluster operations and pod exec sessions are made
directly with `kube-rs`.

Before creating the DaemonSet, `kprobe` ensures that the selected namespace
exists and creates it when missing. Namespace creation is idempotent, and the
namespace is never deleted during cleanup.

TCP/IPv4 is the default. On dual-stack clusters, `kprobe` explicitly selects
each pod's IPv4 address instead of relying on the primary `podIP`. Use
`--ip-family ipv6` to run the equivalent TCP/IPv6 check. The run fails during
setup if a ready probe pod does not have an address in the selected family.

While running, `kprobe` emits one progress bar. It first tests connectivity for
all paths, then measures bandwidth only for paths that connected successfully.
The final report splits connect vs bandwidth results, reports aggregate
throughput, the 3 slowest paths, and up to 50 representative failed paths. A
run with any failed path exits non-zero. Pair generation is lazy and only the
slowest successful paths are retained, so memory use does not grow with the
number of paths.

The temporary DaemonSet runs two containers in each probe pod:

- `connect`: `agnhost netexec --http-port=1199 --udp-port=-1` for TCP connectivity;
- `bandwidth`: `iperf3 -s -p 5201` for throughput measurement (writable `/tmp`
  emptyDir for iperf temp files);
- selects Linux nodes (plus any `--selector` labels) and tolerates all taints;
- skips cordoned/unschedulable nodes during readiness (their probe pods stay
  Pending because the default scheduler will not bind them);
- has no resource requests or limits, giving it Kubernetes `BestEffort` QoS;
- drops Linux capabilities, disallows privilege escalation, and uses the
  runtime-default seccomp profile;
- is uniquely named for each invocation and deleted at the end.

While waiting for readiness, fatal container states such as `ErrImagePull`,
`ImagePullBackOff`, configuration errors, and crash loops are detected and
reported with the pod and node instead of being reduced to a readiness timeout.
Pods that remain Pending only because their target node is unschedulable are
ignored so the probe can continue on the remaining nodes.

Each source pod first checks connectivity:

```text
/agnhost connect --timeout <TIMEOUT> <DESTINATION_POD_IP>:1199
```

On success it measures bandwidth:

```text
iperf3 -c <DESTINATION_POD_IP> -p 5201 -t <BANDWIDTH_TIME> -J
```

Bandwidth is taken from iperf3 JSON `end.sum_received.bits_per_second`.
Because each iperf3 server handles only one client at a time, bandwidth tests
are serialized per destination after the connectivity phase completes. Brief
retries cover residual "server is busy" races. Connectivity failures preserve
agnhost error classes such as `TIMEOUT` and `REFUSED`.

## RBAC

The Kubernetes identity must be able to:

- get and create cluster-scoped `v1` Namespaces;
- create, get, patch, and delete `apps/v1` DaemonSets in the selected namespace;
- list Pods in the selected namespace;
- create requests against the `pods/exec` subresource.

Use `--namespace` to select a different existing diagnostics namespace.
