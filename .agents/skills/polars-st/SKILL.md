---
name: polars-st
description: "Use when working with geospatial data in Polars, polars-st, Shapely, GeoPandas, geometry columns, CRS transformations, spatial joins, or GIS pipelines. Keeps work Polars-native and prevents unnecessary conversion to GeoPandas or use of built-in Polars when polars-st provides the operation."
---

# Polars ST

Use `polars-st` as the default geospatial layer for this project. Keep geometry in a Polars `DataFrame` or `LazyFrame`, use Polars expressions for tabular work, and use the `polars_st` spatial namespace for geometry work. Do not convert to GeoPandas merely because a geometry operation is needed.

## First Steps

1. Inspect the existing frame schema and identify the geometry column.
2. Check the installed version before relying on a version-sensitive method:

   ```bash
   mise exec -- uv run python -c "import importlib.metadata as md; print(md.version('polars-st'))"
   ```

3. Search this skill's API map and the official reference before inventing a helper:
   <https://oreilles.github.io/polars-st/api-reference/>
4. Preserve the current execution mode. Prefer `.lazy()` and expressions for pipelines; call `.collect()` only at an intentional boundary.
5. Keep CRS/SRID explicit. A geometry's coordinate numbers are not enough to establish its CRS.

## Decision Rules

- Use `import polars as pl` for filtering, joining, grouping, sorting, casting, null handling, and string or numeric expressions.
- Use `import polars_st as st` for geometry construction and spatial expressions.
- Use `st.geom("geometry")` when selecting a named geometry column, and use `pl.col("geometry").st` when the Polars expression namespace is available and clearer.
- Use `st.sjoin(left, right, ...)` for spatial joins rather than converting both frames to GeoPandas.
- Use `st.from_wkb`, `st.from_wkt`, `st.from_ewkt`, or `st.from_geojson` at ingestion; serialize with `st.to_wkb`, `st.to_wkt`, `st.to_ewkt`, or `st.to_geojson` at an output boundary.
- Use `st.from_shapely` only when an upstream library already returns Shapely objects. Do not create Shapely objects row by row as an intermediate representation.
- Use `st.from_geopandas` or `.st.to_geopandas()` only at a genuine interoperability boundary: a required third-party API, a user-facing GeoPandas-only operation, or a deliberate export. State the reason in the surrounding code or task summary.
- Never use GeoPandas just to calculate length, area, bounds, centroids, predicates, buffers, overlays, CRS transforms, or spatial joins when the corresponding `polars-st` method exists.
- Never fall back to built-in Polars for geometry encoded as Python objects or strings if it can first be parsed into a native `polars-st` geometry column.

## Core Patterns

Construct geometry in expressions:

```python
import polars as pl
import polars_st as st

frame = frame.with_columns(
    geometry=st.point(
        pl.concat_list(
            pl.col("x").cast(pl.Float64),
            pl.col("y").cast(pl.Float64),
        ),
        srid=4326,
    )
)
```

Apply geometry operations in a lazy pipeline:

```python
result = (
    frame.lazy()
    .with_columns(
        length=st.geom("geometry").st.length(),
        geometry_4326=st.geom("geometry").st.to_srid(4326),
    )
    .filter(st.geom("geometry").st.is_valid())
    .collect()
)
```

Use another geometry expression for binary operations:

```python
matched = frame.filter(
    st.geom("geometry").st.intersects(st.geom("other_geometry"))
)
```

For values that are already normalized to a fractional line position, use the geometry methods directly rather than converting to GeoPandas:

```python
midpoint = st.geom("geometry").st.interpolate(pl.lit(0.5), normalized=True)
position = st.geom("geometry").st.project(st.geom("point"))
```

## API Map

The names below are the principal public methods. Confirm signatures and availability against the installed version and the official reference before use.

### Creation and I/O

- Constructors: `point`, `multipoint`, `linestring`, `circularstring`, `multilinestring`, `polygon`, `rectangle`
- Parsers: `from_wkb`, `from_wkt`, `from_ewkt`, `from_geojson`, `from_shapely`, `from_geopandas`
- File and output helpers: `read_file`, `write_file`, `write_geojson`, `write_ndgeojson`

### Serialization and inspection

- Serialization: `to_wkb`, `to_wkt`, `to_ewkt`, `to_geojson`, `to_dict`, `to_shapely`, `to_geopandas`, `to_dicts`
- General inspection: `geometry_type`, `dimension`, `coordinate_dimension`, `area`, `bounds`, `length`, `minimum_clearance`, `x`, `y`, `z`, `m`, `count_coordinates`, `coordinates`, `count_geometries`, `get_geometry`, `count_points`, `get_point`, `count_interior_rings`, `get_interior_ring`, `exterior_ring`, `interior_rings`, `parts`, `precision`, `set_precision`

### CRS and predicates

- CRS/SRID: `srid`, `set_srid`, `to_srid`
- Unary predicates: `has_z`, `has_m`, `is_ccw`, `is_closed`, `is_empty`, `is_ring`, `is_simple`, `is_valid`, `is_valid_reason`
- Binary predicates: `crosses`, `contains`, `contains_properly`, `covered_by`, `covers`, `disjoint`, `dwithin`, `intersects`, `overlaps`, `touches`, `within`, `equals`, `equals_exact`, `equals_identical`, `relate`, `relate_pattern`

### Geometry operations

- Set operations: `union`, `unary_union`, `coverage_union`, `intersection`, `difference`, `symmetric_difference`
- Constructive operations: `cast`, `multi`, `boundary`, `buffer`, `offset_curve`, `centroid`, `center`, `clip_by_rect`, `convex_hull`, `concave_hull`, `segmentize`, `envelope`, `extract_unique_points`, `build_area`, `make_valid`, `normalize`, `node`, `point_on_surface`, `remove_repeated_points`, `reverse`, `simplify`, `force_2d`, `force_3d`, `flip_coordinates`, `minimum_rotated_rectangle`, `maximum_inscribed_circle`, `snap`, `shortest_line`
- Affine transforms: `affine_transform`, `translate`, `rotate`, `scale`, `skew`
- Line operations: `interpolate`, `project`, `substring`, `line_merge`, `shared_paths`
- Aggregation: `total_bounds`, `collect`, `union_all`, `coverage_union_all`, `intersection_all`, `difference_all`, `symmetric_difference_all`, `polygonize`, `voronoi_polygons`, `delaunay_triangles`
- Spatial operations: `sjoin`
- Visualization: `plot`

Methods are available as top-level helpers where documented and, for many operations, through the geometry namespace. In this codebase, the established style is `st.geom("geometry").st.length()` and related chained expressions.

## Geometry and CRS Checks

Before a spatial calculation, verify:

- the geometry column exists and has a geometry dtype rather than raw strings or arbitrary Python objects;
- both operands use compatible CRS/SRID values;
- distance, buffer, and length units are understood in that CRS;
- `LineString` assumptions are true before using line-only operations such as `project`, `interpolate`, or `substring`;
- null and empty geometries are handled intentionally;
- invalid geometries are detected with `is_valid` and repaired with `make_valid` only when that is the desired behavior.

For geographic output, transform only at the boundary needed by the consumer. Do not repeatedly transform a frame between CRS values inside a pipeline.

## GeoPandas Boundary Checklist

Before converting to GeoPandas, answer all three questions:

1. Which exact operation or API requires GeoPandas?
2. Is there a `polars-st` equivalent or a simpler WKB/GeoJSON output boundary?
3. Where will the conversion happen, and will the result be converted back immediately?

If the answer to the first question is vague, stay in `polars-st`. If conversion is required, convert once near the boundary, avoid row-wise conversions, and preserve the original Polars frame for the rest of the pipeline.

## Completion Checks

- The implementation uses `polars_st as st` for available geometry operations.
- No unnecessary `to_geopandas()` or `from_geopandas()` call was introduced.
- Lazy pipelines remain lazy until their existing collection boundary.
- CRS/SRID and measurement units are explicit and compatible.
- Geometry operations are tested on empty, null, invalid, and representative geometry types where relevant.
- The installed `polars-st` version supports every method used.
