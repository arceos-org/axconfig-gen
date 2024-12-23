use std::io;
use std::path::PathBuf;

use axconfig_gen::{Config, ConfigValue, OutputFormat};
use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the config specification file
    #[arg(short, long, required = true)]
    spec: Vec<String>,

    /// Path to the old config file
    #[arg(short = 'c', long)]
    oldconfig: Option<String>,

    /// Path to the output config file
    #[arg(short, long)]
    output: Option<String>,

    /// The output format
    #[arg(
        short, long,
        default_value_t = OutputFormat::Toml,
        value_parser = PossibleValuesParser::new(["toml", "rust"])
            .map(|s| s.parse::<OutputFormat>().unwrap()),
    )]
    fmt: OutputFormat,

    /// Setting a config item with format `table.key=value`
    #[arg(short, long, id = "CONFIG")]
    write: Vec<String>,

    /// Verbose mode
    #[arg(short, long)]
    verbose: bool,
}

fn parse_config_write_cmd(cmd: &str) -> Result<(String, String, String), String> {
    let (item, value) = cmd.split_once('=').ok_or_else(|| {
        format!(
            "Invalid config setting command `{}`, expected `table.key=value`",
            cmd
        )
    })?;
    if let Some((table, key)) = item.split_once('.') {
        Ok((table.into(), key.into(), value.into()))
    } else {
        Ok((Config::GLOBAL_TABLE_NAME.into(), item.into(), value.into()))
    }
}

macro_rules! unwrap {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    };
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    macro_rules! debug {
        ($($arg:tt)*) => {
            if args.verbose {
                eprintln!($($arg)*);
            }
        };
    }

    let mut config = Config::new();
    for spec in args.spec {
        debug!("Reading config spec from {:?}", spec);
        let spec_toml = std::fs::read_to_string(spec)?;
        let sub_config = unwrap!(Config::from_toml(&spec_toml));
        unwrap!(config.merge(&sub_config));
    }

    if let Some(oldconfig_path) = args.oldconfig {
        debug!("Loading old config from {:?}", oldconfig_path);
        let oldconfig_toml = std::fs::read_to_string(oldconfig_path)?;
        let oldconfig = unwrap!(Config::from_toml(&oldconfig_toml));

        let (untouched, extra) = unwrap!(config.update(&oldconfig));
        for item in &untouched {
            eprintln!(
                "Warning: config item `{}` not set in the old config, using default value",
                item.item_name(),
            );
        }
        for item in &extra {
            eprintln!(
                "Warning: config item `{}` not found in the specification, ignoring",
                item.item_name(),
            );
        }
    }

    for cmd in args.write {
        let (table, key, value) = unwrap!(parse_config_write_cmd(&cmd));
        if table == Config::GLOBAL_TABLE_NAME {
            debug!("Setting config item `{}` to `{}`", key, value);
        } else {
            debug!("Setting config item `{}.{}` to `{}`", table, key, value);
        }
        let new_value = unwrap!(ConfigValue::new(&value));
        let item = unwrap!(config
            .config_at_mut(&table, &key)
            .ok_or("Config item not found"));
        unwrap!(item.value_mut().update(new_value));
    }

    let output = unwrap!(config.dump(args.fmt));
    if let Some(path) = args.output.map(PathBuf::from) {
        if let Ok(oldconfig) = std::fs::read_to_string(&path) {
            // If the output is the same as the old config, do nothing
            if oldconfig == output {
                return Ok(());
            }
            // Calculate the path to the backup file
            let bak_path = if let Some(ext) = path.extension() {
                path.with_extension(format!("old.{}", ext.to_string_lossy()))
            } else {
                path.with_extension("old")
            };
            // Backup the old config file
            std::fs::write(bak_path, oldconfig)?;
        }
        std::fs::write(path, output)?;
    } else {
        println!("{}", output);
    }

    Ok(())
}
