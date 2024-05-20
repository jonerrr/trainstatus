-- find stop and its routes
select
    s.*,
    array_agg(rs.route_id)
from
    stops s
    left join route_stops rs on s.id = rs.stop_id
    and rs.stop_type = 1
where
    s.id = '249'
group by
    s.id;

-- get list of route stops by route and the name of each stop
select
    rs.*,
    s."name"
from
    route_stops rs
    left join stops s ON rs.stop_id = s.id
where
    rs.route_id = '1'
order by
    rs.stop_sequence;

-- get arrival and departure times for a route stop (TODO)
select
    t.id,
    t.route_id,
    t.direction,
    --	t.mta_trip_id,
    array_agg(st.arrival) AS arrivals,
    array_agg(st.stop_id) AS stop_ids --	st.arrival,
    --	st.stop_id
from
    trips t
    left join stop_times st on t.id = st.trip_id
where
    t.route_id = 'A'
group by
    t.id;