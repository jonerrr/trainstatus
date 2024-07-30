-- Train stuff
CREATE INDEX idx_trips_id ON trips(id);

CREATE INDEX idx_positions_trip_id ON positions(trip_id);

CREATE INDEX idx_stop_times_trip_id ON stop_times(trip_id);

CREATE INDEX idx_stop_times_arrival ON stop_times(arrival);

-- Bus stuff
CREATE INDEX idx_bus_trips_id ON bus_trips(id);

CREATE INDEX idx_bus_positions_trip_id ON bus_positions(vehicle_id);

CREATE INDEX idx_bus_stop_times_trip_id ON bus_stop_times(trip_id);

CREATE INDEX idx_bus_stop_times_arrival ON bus_stop_times(arrival);