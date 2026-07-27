# smctl

`smctl` connects to NVIDIA UFM and inspects or manages subnet-manager
partitions and port membership.

## Build

```bash
cargo build --release -p smctl
```

The binary is written to `target/release/smctl`.

## Connection and authentication

The UFM address is required. Options can be supplied as flags or through their
matching environment variables:

| Option | Environment | Purpose |
| --- | --- | --- |
| `--ufm-address` | `UFM_ADDRESS` | UFM base URL |
| `--ufm-token` | `UFM_TOKEN` | User access token |
| `--ufm-username` | `UFM_USERNAME` | Username authentication |
| `--ufm-password` | `UFM_PASSWORD` | Password authentication |
| `--ufm-ca-crt` | `UFM_CA_CRT` | CA certificate for mutual TLS |
| `--ufm-tls-crt` | `UFM_TLS_CRT` | Client certificate for mutual TLS |
| `--ufm-tls-key` | `UFM_TLS_KEY` | Client key for mutual TLS |

The three mutual-TLS files must be provided together.

Token example:

```bash
export UFM_ADDRESS=https://ufm.example.com
export UFM_TOKEN='<token>'
smctl version
```

Username and password example:

```bash
smctl \
  --ufm-address https://ufm.example.com \
  --ufm-username admin \
  --ufm-password '<password>' \
  version
```

Mutual-TLS example:

```bash
smctl \
  --ufm-address https://ufm.example.com \
  --ufm-ca-crt ca.crt \
  --ufm-tls-crt client.crt \
  --ufm-tls-key client.key \
  version
```

Avoid placing credentials directly in shell history. Prefer protected
environment or secret-injection mechanisms.

## Commands

```text
smctl version
smctl info
smctl list
smctl view   --pkey <PKEY>
smctl create --pkey <PKEY> [--ipoib <BOOL>] [--index0 <BOOL>]
             [--membership <MEMBERSHIP>] [--guids <GUID>...]
smctl update --pkey <PKEY> [--ipoib <BOOL>] [--mtu <MTU>]
             [--service-level <LEVEL>] [--rate-limit <RATE>]
smctl bind   --pkey <PKEY> --guids <GUID>...
smctl unbind --pkey <PKEY> --guids <GUID>...
smctl delete --pkey <PKEY>
```

Use `smctl <command> --help` for complete syntax and default values.

## Examples

List and inspect partitions:

```bash
smctl list
smctl view --pkey 0x5
```

Create a partition and bind additional ports:

```bash
smctl create \
  --pkey 0x5 \
  --membership full \
  --guids 0011223344560200 \
  --guids 1070fd0300176625

smctl bind \
  --pkey 0x5 \
  --guids 0011223344560201
```

Update partition attributes:

```bash
smctl update \
  --pkey 0x5 \
  --mtu 4 \
  --service-level 0 \
  --rate-limit 100
```

Remove a port or partition:

```bash
smctl unbind --pkey 0x5 --guids 0011223344560201
smctl delete --pkey 0x5
```
