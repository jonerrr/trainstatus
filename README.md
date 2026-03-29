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

- Use the prebuilt container images linked to this repository.
- Required environment variables are listed in `backend/README.md` and `frontend/README.md`.
- Requires PostgreSQL with PostGIS and Valkey/Redis.
- See `compose.test.yml` for example deployment with traefik. You will need to update the URLs inside of `geo/styles/<style>.json` to match the URL you are hosting it on.

## Development

### Requirements

- Podman (Docker can be used, but tasks use podman)
- [Mise](https://mise.jdx.dev/)
- [BusTime API Key](https://register.developer.obanyc.com/)
- [NJT Developer Account](https://developer.njtransit.com/registration/register)

### Setup

1. Clone the repository
2. Run `mise i` to ensure all tools are installed
3. Set environment variables as listed in `backend/README.md` inside a `backend/mise.local.toml`
4. Run `mise //geo:build` to build valhalla and basemap tiles
5. Run `mise //geo:export` to export tiles to your machine for local development.
6. Start dbs, backend, and frontend with `mise dev`
