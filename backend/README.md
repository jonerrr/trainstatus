# Backend

## Structure

- `protos` folder contains GTFS protobuf definitions
- `realtime` folder contains SIRI, GTFS-RT, parsing and handling logic
- `static_data` folder contains fetching and parsing static bus and train data logic
- `api` folder contains the API routes

## Config

| Environmental Variable | Usage                                                                  | Required |
| ---------------------- | ---------------------------------------------------------------------- | -------- |
| `DATABASE_URL`         | PostgreSQL database URL                                                | Yes      |
| `ADDRESS`              | Address for axum to bind to. Defaults to `127.0.0.1:3055`              | Yes      |
| `FORCE_UPDATE`         | If set, static data will update on startup                             | No       |
| `DEBUG_GTFS`           | If set, saves all realtime GTFS data as a txt and pb file to `./gtfs/` | No       |
| `READ_ONLY`            | If set, the backend will not update any realtime or static data        | No       |
