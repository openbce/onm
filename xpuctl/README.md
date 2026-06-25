# xpuctl

A command line to manage xpu through redfish.

## Configuration

Create a configuration file at `~/.xpuctl`:

```toml
username = "root"
password = "<password>"

# TLS certificate verification (default: true)
# Set to false only for development with self-signed certificates
# tls_verify = false

[[bmc]]
name = "forge02-bf2"
vendor = "bluefield"
address = "https://192.168.0.53"

[[bmc]]
name = "forge02-bf3"
vendor = "bluefield"
address = "https://192.168.0.155"
# Override TLS verification per-BMC if needed
# tls_verify = false
```

**Security Note**: The configuration file contains credentials in plain text. 
Ensure the file has restricted permissions: `chmod 600 ~/.xpuctl`

## Commands

### Discover

```
$ xpuctl discover
Name                BMC                           Status
forge02-bf2         https://192.168.0.53          Ok
forge02-bf3         https://192.168.0.155         Ok
```

### List

```
$ xpuctl list
ID                  Status    Vendor         FW        SN             BMC            Address
forge02-bf2         Ready     bluefield      -         -              Bf-23.09-6     https://192.168.0.53
forge02-bf3         Ready     bluefield      -         -              Bf-23.09-6     https://192.168.0.155
```