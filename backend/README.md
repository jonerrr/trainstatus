# Backend

## Structure

- `protos` folder contains GTFS protobuf definitions
- `realtime` folder contains SIRI, GTFS-RT, parsing and handling logic
- `static_data` folder contains fetching and parsing static bus and train data logic
- `api` folder contains the API routes

## Config

| Environmental Variable | Usage                                                     |
| ---------------------- | --------------------------------------------------------- |
| `FORCE_UPDATE`         | Force update static bus and train data                    |
| `DEBUG_GTFS`           | Save all GTFS data as a txt file to `./gtfs/`             |
| `DATABASE_URL`         | PostgreSQL database URL                                   |
| `ADDRESS`              | Address for axum to bind to. Defaults to `127.0.0.1:3055` |
