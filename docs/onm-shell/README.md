# onm-shell

`onm-shell` is a privileged troubleshooting container containing the ONM
command-line tools and common Linux network and device utilities.

## Build

```bash
docker build -t openbce/onm-shell .
```

Podman can be used instead:

```bash
podman build -t openbce/onm-shell .
```

## Run

Host networking and privileged access are required for complete host-device,
network namespace, firewall, and interface inspection:

```bash
docker run -d \
  --name onm-shell \
  --privileged \
  --network host \
  openbce/onm-shell
```

Enter the shell:

```bash
docker exec -it onm-shell bash
```

The image includes `ethctl`, `hcactl`, `smctl`, `xpuctl`, `kprobe`, `tsctl`,
`kubectl`, `tcpdump`, `iproute2`, `nftables`, `pciutils`, and related tools.

## Stop and remove

```bash
docker stop onm-shell
docker rm onm-shell
```

The container is intended for interactive diagnostics. Do not expose it as a
long-running network service: privileged mode and host networking grant broad
access to the host.
