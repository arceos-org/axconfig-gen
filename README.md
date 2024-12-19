# axconfig-gen

A TOML-based configuration generation tool for [ArceOS](https://github.com/arceos-org/arceos).

## Usage

```
axconfig-gen [OPTIONS] --spec <SPEC>

Options:
  -s, --spec <SPEC>            Path to the config specification file
  -c, --oldconfig <OLDCONFIG>  Path to the old config file
  -o, --output <OUTPUT>        Path to the output config file
  -f, --fmt <FMT>              The output format [default: toml] [possible values: toml, rust]
  -h, --help                   Print help
  -V, --version                Print version
```

For example, to generate a config file `.axconfig.toml` from the config specifications distributed in `a.toml` and `b.toml`, you can run:

```sh
axconfig-gen -s a.toml -s b.toml -o .axconfig.toml -f toml
```

See [defconfig.toml](example_configs/defconfig.toml) for an example of a config specification file.
