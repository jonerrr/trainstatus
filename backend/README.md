# Backend

## Structure

- The `train` and `bus` folders and `alert.rs` file each contain the logic for parsing GTFS data into the database.
- The `routes` folder contains all the API routes

## Config

| Environmental Variable | Usage                                        |
| ---------------------- | -------------------------------------------- |
| `FORCE_UPDATE`         | Force update static bus and train data       |
| `DEBUG_GTFS`           | Save all GTFS data as a txt file to `./gtfs` |
