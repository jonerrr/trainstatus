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
SELECT
    s.*,
    array_agg(
        distinct jsonb_build_object('id', rs.route_id, 'stop_type', rs.stop_type)
    ) AS routes,
    array_agg(
        distinct jsonb_build_object(
            'id',
            t.id,
            'route_id',
            t.route_id,
            'direction',
            t.direction,
            'assigned',
            t.assigned,
            'created_at',
            t.created_at,
            'stop_times',
            (
                select
                    jsonb_agg(st)
                from
                    (
                        select
                            st.stop_id,
                            st.arrival,
                            st.departure
                        from
                            stop_times st
                        where
                            st.trip_id = t.id
                            and st.arrival > now()
                        order by
                            st.arrival
                    ) as st
            )
        )
    ) as trips
FROM
    stops s
    LEFT JOIN route_stops rs ON s.id = rs.stop_id
    LEFT JOIN stop_times st ON s.id = st.stop_id
    LEFT JOIN trips t ON st.trip_id = t.id
where
    s.id = '250'
GROUP BY
    s.id;

-- get arrivals for stop id
select
    t.id,
    t.route_id,
    t.direction,
    array_agg(
        st
        order by
            st.arrival
    ) as stop_times
from
    trips t
    left join stop_times st on t.id = st.trip_id
where
    t.id = any(
        select
            t.id
        from
            trips t
            left join stop_times st on st.trip_id = t.id
        where
            st.stop_id = '250'
            and st.arrival > now()
    )
group by
    t.id;

--- get alerts by route_id
select
    ae.route_id,
    array_agg(
        distinct jsonb_build_object(
            'id',
            a.id,
            'header',
            a.header_html,
            'description',
            a.description_html,
            'alert_type',
            a.alert_type,
            'active_periods',
            ap
        )
    ) as alerts
from
    alerts a
    left join active_periods ap on a.id = ap.alert_id
    left join affected_entities ae on a.id = ae.alert_id
where
    ae.route_id = '4'
    and ap.start_time < now()
    and (
        ap.end_time > now()
        or ap.end_time is null
    )
group by
    ae.route_id;