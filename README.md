# Trainstat.us

The best website to check the status of your train (and bus).

## Features

- Blazingly fast
- Real-time information that automatically updates
- Works offline
- Installable as a PWA
- Shallow routing between dialogs so you never lose your place
- Shareable links for your trip
- Works on mobile and desktop
- No ads or tracking (your geolocation data never leaves your device)
- View historical data
- Simple API for developers

## Build Requirements

- Postgres
- Node.js
- Pnpm
- Rust

## Config

### Backend

- Set `FORCE_UPDATE` to force update static bus and train data
- Set `DEBUG_GTFS` to save GTFS data to disk in the `./gtfs` folder
