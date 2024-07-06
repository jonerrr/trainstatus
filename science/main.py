# import cudf.pandas
# cudf.pandas.install()
import pandas as pd
import geopandas as gpd
from sqlalchemy import create_engine
from os import environ
from scipy.spatial import cKDTree
from shapely.geometry import Point
import numpy as np

# TODO: import uri from env
uri = environ.get("DATABASE_URI")

alchemyEngine = create_engine(uri)


def longest_trip(df: pd.DataFrame):
    df["trip_length"] = (
        df.groupby("trip_id")
        .apply(lambda group: group["arrival"].iloc[-1] - group["arrival"].iloc[0])
        .reset_index(level=0, drop=True)
    )

    df = df.sort_values(by="trip_length", ascending=True)
    print(df.head(60))


def closest_stops():
    df = pd.read_sql(
        """
        SELECT
            *
        FROM
            stops""",
        alchemyEngine,
    )

    df_bus = pd.read_sql(
        """
        SELECT
            *
        FROM
            bus_stops""",
        alchemyEngine,
    )

    df = gpd.GeoDataFrame(
        df, geometry=gpd.points_from_xy(df["lat"], df["lon"]), crs=4326
    )

    df_bus = gpd.GeoDataFrame(
        df_bus, geometry=gpd.points_from_xy(df_bus["lat"], df_bus["lon"]), crs=4326
    )

    df = df.to_crs(epsg=2263)
    df_bus = df_bus.to_crs(epsg=2263)

    df = ckdnearest(df, df_bus)

    # df_bus_closest: gpd.GeoDataFrame = df_bus.copy()
    # # get nearest 5 bus stops to each train stop
    # df_bus_closest["geometry"] = df_bus_closest["geometry"].apply(
    #     lambda x: x.buffer(1000)
    # )
    # df = gpd.sjoin(df, df_bus_closest, op="intersects", how="inner")
    # df = df.drop(columns=["index_right", "geometry"])
    # df = df.drop_duplicates(subset=["id", "name"])

    print(df)
    df.to_csv("closest_stops.csv", index=False)
    # print(df_bus)


def ckdnearest(gdA, gdB):
    nA = np.array(list(gdA.geometry.apply(lambda x: (x.x, x.y))))
    nB = np.array(list(gdB.geometry.apply(lambda x: (x.x, x.y))))
    btree = cKDTree(nB)
    dist, idx = btree.query(nA, k=1)
    gdB_nearest = gdB.iloc[idx].drop(columns="geometry").reset_index(drop=True)
    gdf = pd.concat(
        [gdA.reset_index(drop=True), gdB_nearest, pd.Series(dist, name="dist")], axis=1
    )

    return gdf


def main():
    closest_stops()
    # get the longest wait between stops
    # df = pd.read_sql(
    #     """
    #     SELECT
    #         st.*, t.route_id, t.direction, t.created_at
    #         FROM
    #             stop_times st
    #         LEFT JOIN trips t ON st.trip_id = t.id
    #         ORDER BY
    #             st.arrival""",
    #     alchemyEngine,
    # )

    # longest_trip()
    quit()

    # print(df.dtypes)
    # df = df.sort_values(by="wait_time", ascending=True)
    df["wait_time"] = (
        df.groupby("trip_id")
        .apply(lambda group: group["arrival"].diff())
        .reset_index(level=0, drop=True)
    )
    df = df.sort_values(by="wait_time", ascending=False)
    # group by trip and sort by arrival, and then get the difference between the next arrival
    # df["wait_time"] = df.groupby("trip_id")["arrival"].diff()
    # df["wait_time"] = df["wait_time"].dt.total_seconds()

    df_t = pd.read_sql(
        """
        SELECT
            *
        FROM
            trips""",
        alchemyEngine,
    )

    df["route"] = df["trip_id"].map(df_t.set_index("id")["route_id"])

    df_s = pd.read_sql(
        """
        SELECT
            *
        FROM
            stops""",
        alchemyEngine,
    )

    df_s = df_s.set_index("id")
    df["stop_name"] = df["stop_id"].map(df_s["name"])
    print(df.head(60))
    # print(df.tail(60))

    # print(df.groupby('trip_id')['wait_time'].max().sort_values(ascending=False).head(10))


if __name__ == "__main__":
    main()


# TODO: calculate longest trip
