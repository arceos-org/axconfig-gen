use std::io;

use axconfig_gen::Config;
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
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let mut defconfig = Config::new();
    for spec in args.spec {
        let spec_toml = std::fs::read_to_string(spec)?;
        let sub_config = Config::from_toml(&spec_toml).unwrap();
        defconfig.merge(&sub_config).unwrap();
    }

    let output_config = if let Some(oldconfig_path) = args.oldconfig {
        let oldconfig_toml = std::fs::read_to_string(oldconfig_path)?;
        let oldconfig = Config::from_toml(&oldconfig_toml).unwrap();
        defconfig.update(&oldconfig).unwrap();
        defconfig
    } else {
        defconfig
    };

    let output = output_config.gen_toml().unwrap();
    if let Some(path) = args.output {
        std::fs::write(path, output)?;
    } else {
        println!("{}", output);
    }

    Ok(())
}
