let pbfs = ["https://download.geofabrik.de/north-america/us/new-york-latest.osm.pbf", "https://download.geofabrik.de/north-america/us/new-jersey-latest.osm.pbf", "https://download.geofabrik.de/north-america/us/connecticut-latest.osm.pbf"]

for pbf in $pbfs {
    let filename = $pbf | split row '/' | last
    let path = $"data/($filename)"
    let path_exists = $path | path exists

    if $path_exists == false {
        print $"Downloading ($pbf) to ($path)"
        http get $pbf | save $path
    } else {
        print $"File ($path) already exists, skipping download"
    }
}