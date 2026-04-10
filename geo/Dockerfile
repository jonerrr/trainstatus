FROM debian:bookworm-slim@sha256:4724b8cc51e33e398f0e2e15e18d5ec2851ff0c2280647e1310bc1642182655d AS osm-data

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    wget \
    osmium-tool \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

RUN --mount=type=cache,id=osm-pbf-cache,target=/var/cache/osm-pbfs \
    set -eux; \
    for url in \
    https://download.geofabrik.de/north-america/us/new-york-latest.osm.pbf \
    https://download.geofabrik.de/north-america/us/new-jersey-latest.osm.pbf \
    https://download.geofabrik.de/north-america/us/pennsylvania-latest.osm.pbf \
    https://download.geofabrik.de/north-america/us/connecticut-latest.osm.pbf; do \
    file="$(basename "${url}")"; \
    if [ ! -s "/var/cache/osm-pbfs/${file}" ]; then \
    wget -O "/var/cache/osm-pbfs/${file}" "${url}"; \
    fi; \
    done; \
    osmium merge \
    /var/cache/osm-pbfs/new-york-latest.osm.pbf \
    /var/cache/osm-pbfs/new-jersey-latest.osm.pbf \
    /var/cache/osm-pbfs/pennsylvania-latest.osm.pbf \
    /var/cache/osm-pbfs/connecticut-latest.osm.pbf \
    -o /build/merged.osm.pbf

FROM ghcr.io/valhalla/valhalla:3.6.3@sha256:2b19ea46551a9687b245022551183829d817fdee9b58c5e7b2adb6e422749c43 AS valhalla-builder

WORKDIR /build
COPY --from=osm-data /build/merged.osm.pbf /build/merged.osm.pbf

RUN mkdir -p valhalla_tiles && \
    valhalla_build_config \
    --mjolnir-tile-dir /build/valhalla_tiles \
    --mjolnir-tile-extract /data/valhalla_tiles.tar \
    --mjolnir-timezone /build/valhalla_tiles/timezones.sqlite \
    --mjolnir-admin /build/valhalla_tiles/admins.sqlite \
    > valhalla.json

RUN valhalla_build_timezones > valhalla_tiles/timezones.sqlite
RUN valhalla_build_admins -c valhalla.json merged.osm.pbf
RUN valhalla_build_tiles -c valhalla.json merged.osm.pbf

# Package the tiles into the optimized, mmap-able tar extract
RUN valhalla_build_extract -c valhalla.json -v

# (Optional) Delete the raw tiles directory to save space in the builder stage
# RUN rm -rf /build/valhalla_tiles

FROM ghcr.io/onthegomap/planetiler:latest@sha256:e6f27b62c10a531a42be8168428fa3bf12f6e6c1be66233c476e00a6d89d1139 AS planetiler-builder

WORKDIR /data
COPY --from=osm-data /build/merged.osm.pbf /data/merged.osm.pbf

RUN --mount=type=cache,id=planetiler-sources-cache,target=/data/sources \
    --mount=type=cache,id=planetiler-tmp-cache,target=/tmp/planetiler \
    --mount=type=bind,source=config.properties,target=/data/config.properties,ro \
    java -Xmx4g -cp "/app/resources:/app/classes:/app/libs/*" com.onthegomap.planetiler.Main --config=/data/config.properties

FROM scratch

COPY --from=valhalla-builder /build/valhalla.json /valhalla.json
COPY --from=valhalla-builder /data/valhalla_tiles.tar /valhalla_tiles.tar
COPY --from=planetiler-builder /data/basemap.mbtiles /basemap.mbtiles
