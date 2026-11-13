# Train Status

> [!WARNING]
> This is a work in progress. Some features are still missing/broken and breaking changes will occur.

The best website to check the status of your train (and bus).

If you'd like to try a hosted version of the API, you can read the [API Documentation](https://trainstat.us/api/docs). You can also host your own version using the prebuilt container images linked to this repository.

A realtime transit map is available at [trainstat.us/map](https://trainstat.us/map).

## Features

- Blazingly fast 😼😼😼😼
- Real-time alerts and arrivals for various transit agencies
- Works offline
- Installable as a PWA
- Shallow routing between modals so you never lose your place
- Shareable links for your trip
- Works on mobile and desktop
- No ads or tracking (your geolocation data never leaves your device)
- View transit data at a specific point in time
- Simple JSON API that you can use to develop your own applications.
- Automatic vector tile layers for spatial data using Martin.

## Agencies Supported

| Agency     | Status |
| ---------- | ------ |
| MTA Subway | ✅     |
| MTA Bus    | ✅     |
| NJT Bus    | ✅     |
| NJT Rail   | 🚧     |
| LIRR       | 🚧     |
| MNR        | 🚧     |

- ✅: Complete
- ⚠️: Working on it
- 🚧: Not started yet

## Self Hosting

> [!NOTE]
> You might need to restart the martin container on the first run so it picks up the tile functions installed by the baseline migrations.

### Requirements

- Podman (Docker can be used, but mise tasks use podman)
- [mise](https://mise.jdx.dev/) (Required for local development)
- [BusTime API Key](https://register.developer.obanyc.com/)
- [NJT Developer Account](https://developer.njtransit.com/registration/register)

### Production Setup

1. Review and set the required environment variables listed in `backend/README.md` and `frontend/README.md`.
2. Geo styles in `geo/styles/*.json` use relative URLs. Martin reads the `X-Forwarded-Host`/`X-Forwarded-Proto` headers to resolve the public URL, so make sure your reverse proxy forwards them (see the `traefik` service in `demo.pod.yml`).
3. See `demo.pod.yml` for an example deployment with traefik.
4. Copy `demo.configmap.yml.example` to `demo.configmap.yml` and fill in the required values.
5. Launch it with `mise demo`. This starts the `trainstatus-demo` pod when it is not already running.

### Local Development Setup

1. Clone the repository
2. Run `mise bootstrap --only tools,task` to install the tools and Git hooks.
3. Copy `dev.pod.yml.example` to `dev.pod.yml`. It includes a Datadog Agent (commented out by default) for [optional telemetry](#optional-telemetry).
4. Create `fnox.local.toml` in the repository root with your provider secrets as described in [Local secrets](#local-secrets).
   - Non-sensitive variables like the DB URLs are already set in the root `mise.toml`. Put additional non-sensitive overrides from [backend configuration](backend/README.md#config) in `backend/mise.local.toml`.
5. Within the `backend/` directory, run `mise bootstrap packages` to install the required build dependencies for Valhalla and protobufs.
6. You can pull the geo data and assets from ghcr or build them locally with `mise //geo:build` (it will take a while).
7. Run `mise //geo:export` to export the Valhalla tile extract.
8. Start Postgres and Martin with `mise containers:up`.
9. Start the backend and frontend with `mise dev`. This starts the dev pod first when it is not already running.

`mise containers:down` stops the dev pod and keeps its volumes. `mise reset` deletes that pod and its volumes, then creates them again. Both commands take an optional suffix, `dev` by default: `mise reset demo` targets the `trainstatus-demo` pod from `demo.pod.yml`.

#### Local secrets

Local mise tasks use [fnox](https://fnox.jdx.dev/) through [mise's secrets integration](https://mise.jdx.dev/environments/secrets/fnox.html). fnox is included in the project's mise tools. Create a gitignored `fnox.local.toml` in the repository root with your own values:

```toml
[secrets]
MTA_API_URL = { default = "https://your-mta-api-host" }
MTA_OBA_API_KEY = { default = "your-bustime-api-key" }
NJT_USERNAME = { default = "your-njt-username" }
NJT_PASSWORD = { default = "your-njt-password" }
```

These `default` values are stored in plaintext. You can instead configure an [fnox provider](https://fnox.jdx.dev/) in the same local file for encrypted values or password-manager references.

Each task explicitly lists the secrets it needs. The backend dev tasks and fixture capture receive the provider credentials; tests, SQLx preparation/checks, OpenAPI export, and the container build need `MTA_API_URL` because Rust embeds it at compile time. Dispatcher tasks do not pass secrets to other tasks, and secrets do not appear in your activated shell. Run `mise secrets ls` to inspect the available keys and task grants without printing their values.

Podman kube deployments continue to use `demo.configmap.yml` and, when needed, `dev.configmap.yml`. Fill those in separately using their example files; the mise fnox integration does not populate the configmaps or inject secrets into containers automatically.

#### Optional telemetry

The backend exports telemetry over OTLP. Configure a collector in your local `dev.pod.yml` and run:

1. Run `mise containers:up` (`mise containers:down` first if the pod exists; volumes are kept).
2. From `backend/`, run `mise run dev:telemetry` instead of the ordinary dev task.

The `dev.pod.yml.example` file includes a commented Datadog Agent configuration as an example. If you use it, copy `dev.configmap.yml.example` to the gitignored `dev.configmap.yml` and set `DD_API_KEY` (stored in plaintext; keep it private). Filter Datadog by `service:trainstatus-backend env:development`. See the [backend telemetry architecture](docs/architecture/backend.md#optional-telemetry) for details.
