use anyhow::Result;
use serde::Serialize;

pub fn format_json<T: Serialize>(data: &T) -> Result<String> {
    let json = serde_json::to_string_pretty(data)?;
    Ok(json)
}

pub fn print_json<T: Serialize>(data: &T) -> Result<()> {
    let json = format_json(data)?;
    println!("{}", json);
    Ok(())
}
