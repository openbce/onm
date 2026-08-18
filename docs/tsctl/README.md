# tsctl

`tsctl` is a command-line client for inspecting Tailscale tailnets and devices
through the Tailscale REST API v2.

## Build

```bash
cargo build --release -p tsctl
```

The binary is written to `target/release/tsctl`.

## Authentication

Provide either an API access token or an OAuth client.

### API access token

```bash
export TS_API_KEY='tskey-api-...'
```

### OAuth client

Create the OAuth client in the Tailscale admin console under
[Trust credentials](https://login.tailscale.com/admin/settings/trust-credentials)
and grant **Devices → Read** (scope `devices:core:read`, or the legacy
`devices` / `devices:read` scopes). Without a devices scope, `list` and
`view` return HTTP 403. Listing all OAuth clients with `list -c` needs
**`all:read`** (or a user API key). `oauth_keys:read` is enough to resolve the
current `--client-id` only.

```bash
export TS_CLIENT_ID='k...'
export TS_CLIENT_SECRET='tskey-client-...'
```

`tsctl` exchanges the client credentials for a short-lived access token before
calling the API. By default it requests no extra `scope` parameter, so the
token inherits every scope granted to the OAuth client (same as Tailscale's
documented curl example). To narrow the token, set `TS_OAUTH_SCOPE` or
`--oauth-scope` (for example `devices:core:read`).

Keep credentials out of configuration files and source control.

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

List OAuth client IDs in a tailnet:

```bash
tsctl list -c
tsctl list -n example.com --clients
```

`-c/--clients` calls `GET /tailnet/{tailnet}/keys?all=true` and keeps entries
with `keyType=client`. Tailscale only enumerates OAuth clients for credentials
with **`all:read`** / **`all`** (or a user API access token). When you
authenticate with `--client-id` / `TS_CLIENT_ID`, that client is always
included (details are filled in when `oauth_keys:read` or `all:read` allows
`GET /keys/{id}`).

View a tailnet summary and its devices:

```bash
tsctl view -n example.com
```

View one device using its device ID, node ID, MagicDNS name, or hostname:

```bash
tsctl view -d n1234567890CNTRL
tsctl view -d 150-136-69-0.taila6f3f.ts.net
```

`-n/--tailnet` is the tailnet organization ID (or `-`), not a device name.
`-d/--device` selects a single device.

Return the original REST response as JSON or YAML:

```bash
tsctl view -d n1234567890CNTRL --output json
tsctl view -n example.com -o yaml
```

`tsctl view` requires exactly one of `-n/--tailnet` and `-d/--device`.
Omit `-o/--output` for the human-readable view. Use `-o/--output json` or
`-o/--output yaml` for structured output.
Run `tsctl <command> --help` for complete syntax.
