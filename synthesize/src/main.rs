use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use synthesize::Result;

fn main() -> Result<()> {
    if let Err(e) = self::cli() {
        writeln!(io::stderr(), "{}", e)?;

        std::process::exit(1);
    }

    Ok(())
}

fn cli() -> Result<()> {
    let mut args = std::env::args().skip(1);

    if args.len() != 2 {
        writeln!(
            io::stderr(),
            "Usage: synthesize <generation_dir> <out_path>"
        )?;

        std::process::exit(1);
    }

    let generation_dir = args
        .next()
        .ok_or("Expected path to generation, got none.")?
        .parse::<PathBuf>()?;
    let out_path = args
        .next()
        .ok_or("Expected output path, got none.")?
        .parse::<PathBuf>()?;

    let bootspec = synthesize::synthesize_schema_from_generation(&generation_dir)
        .map_err(|e| format!("Failed to synthesize bootspec for {}:\n{}", generation_dir.display(), e))?;
    let pretty = serde_json::to_string_pretty(&bootspec)
        .map_err(|e| format!("Failed to make pretty JSON from bootspec:\n{}", e))?;

    fs::write(&out_path, pretty)
        .map_err(|e| format!("Failed to write JSON to '{}':\n{}", out_path.display(), e))?;

    Ok(())
}
