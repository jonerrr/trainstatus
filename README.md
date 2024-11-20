# TrainStat.us

The best website to check the status of your train (and bus).

If you'd like to try a hosted version of the API, you can find the documentation [here](https://trainstat.us/api/docs). You can also host your own version using the prebuilt container images linked to this repository.

## Features

- Blazingly fast 😼😼😼😼
- Real-time information that automatically updates
- Works offline
- Installable as a PWA
- Shallow routing between modals so you never lose your place
- Shareable links for your trip
- Works on mobile and desktop
- No ads or tracking (your geolocation data never leaves your device)
- Simple API for developers for static and real-time data (in JSON and GeoJSON formats).

## Development

### Requirements

- Podman/Docker
- Pnpm
- Rust

### Setup

1. Clone the repository
2. Get an MTA BusTime API Key from [here](https://bustime.mta.info/wiki/Developers/Index).
3. Set environment variables as listed in `backend/README.md`
4. Start databases with `docker compose up`
5. Start backend with `cargo run --release`
6. Start frontend with `pnpm dev`
