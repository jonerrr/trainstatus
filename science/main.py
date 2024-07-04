# import cudf.pandas
# cudf.pandas.install()
import pandas as pd
import pandas as pd
from sqlalchemy import create_engine
from os import environ

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


def main():
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
