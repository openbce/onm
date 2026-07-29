# tsctl

`tsctl` is a command-line client for inspecting Tailscale tailnets and devices
through the Tailscale REST API v2.

## Build

```bash
cargo build --release -p tsctl
```

The binary is written to `target/release/tsctl`.

## Authentication

Create a Tailscale API access token and expose it through the environment:

```bash
export TS_API_KEY='tskey-api-...'
```

Keep the token out of configuration files and source control. For scoped
credentials, read-only device operations require the `devices:core:read`
scope.

The API URL defaults to `https://api.tailscale.com/api/v2/`. It can be
overridden with `TS_API_URL` or `--api-url`.

## Commands

List devices in the tailnet associated with the token:

```bash
tsctl list -n -
```

List devices in a named tailnet:

```bash
tsctl list -n example.com
```

View a tailnet summary and its devices:

```bash
tsctl view -n example.com
```

View one device using its device ID or node ID:

```bash
tsctl view -d n1234567890CNTRL
```

Return the original REST response as JSON or YAML:

```bash
tsctl view -d n1234567890CNTRL --output json
tsctl view -n example.com -o yaml
```

`tsctl view` requires exactly one of `-n/--tailnet` and `-d/--device`.
Omit `-o/--output` for the human-readable view. Use `-o/--output json` or
`-o/--output yaml` for structured output.
Run `tsctl <command> --help` for complete syntax.
