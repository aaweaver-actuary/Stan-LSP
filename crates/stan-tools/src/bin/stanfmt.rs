use std::{
    env, fs,
    io::{self, Read},
    path::PathBuf,
    process::ExitCode,
};

use stan_language::{FormatterConfig, analyze, format};

fn main() -> ExitCode {
    match run() {
        Ok(changed) if changed => ExitCode::from(1),
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stanfmt: {error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<bool, String> {
    let config = fs::read_to_string("stanfmt.toml")
        .ok()
        .map(|input| FormatterConfig::from_toml(&input))
        .transpose()?
        .unwrap_or_default();
    let mut check = false;
    let mut stdin_path = None;
    let mut files = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--check" => check = true,
            "--stdin-file-path" => {
                stdin_path = Some(args.next().ok_or("--stdin-file-path requires a path")?);
            }
            "-h" | "--help" => {
                println!("Usage: stanfmt [--check] [--stdin-file-path PATH] [FILES...]");
                return Ok(false);
            }
            value if value.starts_with('-') => return Err(format!("unknown option {value:?}")),
            value => files.push(PathBuf::from(value)),
        }
    }

    if files.is_empty() {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| error.to_string())?;
        let formatted = format(&analyze(&source), &config)
            .map_err(|error| format!("{}: {error}", stdin_path.as_deref().unwrap_or("stdin")))?;
        if check {
            return Ok(formatted != source);
        }
        print!("{formatted}");
        return Ok(false);
    }

    let mut changed = false;
    for path in files {
        let source =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let formatted = format(&analyze(&source), &config)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if formatted != source {
            changed = true;
            if check {
                eprintln!("would reformat {}", path.display());
            } else {
                fs::write(&path, formatted)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
            }
        }
    }
    Ok(check && changed)
}
