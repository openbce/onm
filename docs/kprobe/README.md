# kprobe

`kprobe` validates the pod network between Kubernetes nodes. It creates one
temporary `agnhost` pod on every eligible Linux node and checks every directed
source-to-destination path on TCP port 1199.

## Usage

```bash
kprobe [OPTIONS]
```

Useful options:

```text
-n, --namespace <NAMESPACE>       Namespace (default: onm-system)
-c, --concurrency <CONCURRENCY>   Simultaneous pod exec requests (default: 16)
-t, --timeout <TIMEOUT>           Per-connection timeout (default: 5s)
    --ip-family <IP-FAMILY>       Pod address family: ipv4 or ipv6 (default: ipv4)
    --ready-timeout <TIMEOUT>     DaemonSet readiness timeout (default: 2m)
    --image <IMAGE>               Override the agnhost image
```

The tool uses the active kubeconfig context (or in-cluster configuration). It
does not invoke `kubectl`. Cluster operations and pod exec sessions are made
directly with `kube-rs`. Before creating the DaemonSet, `kprobe` ensures that
the selected namespace exists and creates it when missing. Namespace creation
is idempotent, and the namespace is never deleted during cleanup.

TCP/IPv4 is the default. On dual-stack clusters, `kprobe` explicitly selects
each pod's IPv4 address instead of relying on the primary `podIP`. Use
`--ip-family ipv6` to run the equivalent TCP/IPv6 check. The run fails during
setup if a ready probe pod does not have an address in the selected family.

While running, `kprobe` emits one progress bar rather than one line per path.
The final report contains totals and up to 50 representative failed paths. A
run with any failed path exits non-zero. Pair generation is lazy and successful
results are not retained, so memory use does not grow with the number of paths.

The temporary DaemonSet:

- runs `agnhost netexec --http-port=1199 --udp-port=-1`;
- selects Linux nodes and tolerates all taints;
- requests 5m CPU and 8 MiB memory per node;
- drops Linux capabilities, disallows privilege escalation, and uses the
  runtime-default seccomp profile;
- is uniquely named for each invocation and deleted at the end.

Each source pod runs:

```text
/agnhost connect --timeout <TIMEOUT> <DESTINATION_POD_IP>:1199
```

This is the image's built-in TCP connectivity client and does not require
`curl`, `nc`, a shell, or package installation.

## RBAC

The Kubernetes identity must be able to:

- get and create cluster-scoped `v1` Namespaces;
- create, get, patch, and delete `apps/v1` DaemonSets in the selected namespace;
- list Pods in the selected namespace;
- create requests against the `pods/exec` subresource.

Use `--namespace` to select a different existing diagnostics namespace.
