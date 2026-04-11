# Train Status

> [!WARNING]
> This is a work in progress. Some features are still missing/broken.

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
| MTA Subway | ⚠️     |
| MTA Bus    | ⚠️     |
| NJT Bus    | ⚠️     |
| NJT Rail   | 🚧     |
| LIRR       | 🚧     |
| MNR        | 🚧     |

- ✅: Pretty much complete
- ⚠️: Working on it
- 🚧: Not started yet

## Self Hosting

### Requirements

- Podman (Docker can be used, but tasks use podman)
- [BusTime API Key](https://register.developer.obanyc.com/)
- [NJT Developer Account](https://developer.njtransit.com/registration/register)

- Use the prebuilt container images linked to this repository.
- Required environment variables are listed in `backend/README.md` and `frontend/README.md`.
- Requires PostgreSQL with PostGIS and Valkey/Redis.
- Geo styles are generated from `geo/styles/*.json.tmpl` during the geo-assets image build.
  - Set `STYLE_BASE_URLS` when building `geo/Dockerfile.assets` to include your host URL(s) (comma-separated).
- See `demo.pod.yml` for an example deployment with traefik:
  - Copy `demo.configmap.yml.example` to `demo.configmap.yml` and fill in the required values.
  - In `demo.pod.yml`, set `martin.yml -> styles.sources.dark-matter` to the matching generated filename (for example `dark-matter.example-com.json`).
  - Launch it with `podman kube play --replace demo.pod.yml --configmap demo.configmap.yml`.
- **NOTE**: You might need to restart the martin container on the first run so it can pickup the PostGIS tables

## Development

### Setup

1. Install [mise](https://mise.jdx.dev/), which is used for installing various tools and running tasks.
2. Clone the repository
3. Run `mise i` to ensure all tools are installed
4. Set environment variables as listed in `backend/README.md` inside a `backend/mise.local.toml`
5. You can pull the geo data and assets from ghcr or build them locally with `mise //geo:build` (it will take a while).
6. Start PostGIS, Valkey, and Martin from `dev.pod.yml` with `mise start-containers`.
7. Once PostGIS is up, start backend and frontend with `mise dev`.

To stop and clean up the local dev pod, run `podman kube down dev.pod.yml`.
