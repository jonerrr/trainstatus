fn main() -> anyhow::Result<()> {
    println!("{}", backend::api::openapi().to_pretty_json()?);
    Ok(())
}
