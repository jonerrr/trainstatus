import pandas as pd
import geopandas as gpd
from shapely import wkt

df = pd.read_csv(
    "https://data.ny.gov/api/views/i9wp-a4ja/rows.csv?accessType=DOWNLOAD&sorting=true"
)

df["geometry"] = df.entrance_georeference.apply(wkt.loads)
print(df)

gdf = gpd.GeoDataFrame(df, geometry="geometry", crs="EPSG:4326")

gdf.to_file("entrances.geojson", driver="GeoJSON")
