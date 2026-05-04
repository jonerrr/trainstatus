use anyhow::{Context, Result};
use backend::sources::{
    mta_bus::static_data as mta_bus_static, mta_subway::static_data as mta_subway_static,
};
use std::env;
use std::path::{Path, PathBuf};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let source = args
        .next()
        .context("usage: fixtures <mta_subway|mta_bus> [--output <dir>]")?;

    let mut output_dir = PathBuf::from("tests/fixtures/static");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => {
                let value = args.next().context("--output requires a directory path")?;
                output_dir = PathBuf::from(value);
            }
            other => {
                return Err(anyhow::anyhow!("unrecognized argument: {other}"));
            }
        }
    }

    match source.as_str() {
        "mta_subway" => {
            write_source(
                "mta_subway",
                &output_dir,
                mta_subway_static::capture_fixtures().await?,
            )
            .await?
        }
        "mta_bus" => {
            write_source(
                "mta_bus",
                &output_dir,
                mta_bus_static::capture_fixtures().await?,
            )
            .await?
        }
        other => return Err(anyhow::anyhow!("unknown source: {other}")),
    }

    Ok(())
}

async fn write_source(
    source: &str,
    root: &Path,
    fixtures: std::collections::BTreeMap<String, serde_json::Value>,
) -> Result<()> {
    let source_dir = root.join(source);
    tokio::fs::create_dir_all(&source_dir)
        .await
        .with_context(|| format!("failed to create fixture dir {}", source_dir.display()))?;

    for (file_name, payload) in fixtures {
        let file_path = source_dir.join(file_name);
        let content = serde_json::to_string_pretty(&payload)?;
        tokio::fs::write(&file_path, content)
            .await
            .with_context(|| format!("failed to write fixture {}", file_path.display()))?;
    }

    Ok(())
}
