# Train Status

## Notice: Websites are currently offline as I complete a large refactor

The best website to check the status of your train (and bus).

~~If you'd like to try a hosted version of the API, you can find the documentation [here](https://trainstat.us/api/docs)~~. You can also host your own version using the prebuilt container images linked to this repository.

~~A realtime bus map is available at <a href="https://trainstat.us/map" target="_blank">trainstat.us/map</a>.~~

## Features

- Blazingly fast 😼😼😼😼
- Real-time alerts and arrivals for MTA subways and buses
- Works offline
- Installable as a PWA
- Shallow routing between modals so you never lose your place
- Shareable links for your trip
- Works on mobile and desktop
- No ads or tracking (your geolocation data never leaves your device)
- Simple API that supports JSON and GeoJSON responses.

## Self Hosting

- Use the prebuilt container images linked to this repository.
- Required environment variables are listed in `backend/README.md` and `frontend/README.md`.
- Requires PostgreSQL and Valkey/Redis.
- Route snapping for MTA Bus and NJT Bus uses an on-demand `valhalla_service` process managed by the backend (lazy start + idle shutdown).
- Set `VALHALLA_CONFIG` if your Valhalla config file is not at `/data/valhalla.json`.

## Development

### Requirements

- Podman/Docker
- [Mise](https://mise.jdx.dev/)
- [BusTime API Key](https://register.developer.obanyc.com/)

### Setup

1. Clone the repository
2. Mise should automatically install required dependencies
3. Set environment variables as listed in `backend/README.md` inside `backend/.env.toml`
4. Start PostgreSQL and Valkey with `docker compose up -d` or `podman compose up -d`
5. Install SQLX CLI with `cargo install sqlx-cli --no-default-features --features native-tls,postgres`
6. Start backend with `cargo r`
7. Start frontend with `pnpm dev`
