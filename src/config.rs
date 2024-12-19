use std::collections::BTreeMap;
use toml_edit::{Decor, DocumentMut, Item, Table, Value};

use crate::output::{Output, OutputFormat};
use crate::{ConfigErr, ConfigResult, ConfigType, ConfigValue};

type ConfigTable = BTreeMap<String, ConfigItem>;

/// A structure representing a config item.
///
/// It contains the config key, value and comments.
#[derive(Debug, Clone)]
pub struct ConfigItem {
    key: String,
    value: ConfigValue,
    comments: String,
}

impl ConfigItem {
    fn new(table: &Table, key: &str, value: &Value) -> ConfigResult<Self> {
        let inner = || {
            let item = table.key(key).unwrap();
            let comments = prefix_comments(item.leaf_decor())
                .unwrap_or_default()
                .to_string();
            let suffix = suffix_comments(value.decor()).unwrap_or_default().trim();
            let value = if !suffix.is_empty() {
                let ty_str = suffix.trim_start_matches('#');
                let ty = ConfigType::new(ty_str)?;
                ConfigValue::from_raw_value_type(value, ty)?
            } else {
                ConfigValue::from_raw_value(value)?
            };
            Ok(Self {
                key: key.into(),
                value,
                comments,
            })
        };
        let res = inner();
        if let Err(e) = &res {
            eprintln!("Parsing error at key `{}`: {:?}", key, e);
        }
        res
    }

    /// Returns the key of the config item.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the value of the config item.
    pub fn value(&self) -> &ConfigValue {
        &self.value
    }

    /// Returns the comments of the config item.
    pub fn comments(&self) -> &str {
        &self.comments
    }
}

/// A structure storing all config items.
///
/// It contains a global table and multiple named tables, each table is a map
/// from key to value, the key is a string and the value is a [`ConfigItem`].
#[derive(Default, Debug)]
pub struct Config {
    global: ConfigTable,
    tables: BTreeMap<String, ConfigTable>,
    table_comments: BTreeMap<String, String>,
}

impl Config {
    /// Create a new empty config object.
    pub fn new() -> Self {
        Self {
            global: ConfigTable::new(),
            tables: BTreeMap::new(),
            table_comments: BTreeMap::new(),
        }
    }

    fn new_table(&mut self, name: &str, comments: &str) -> ConfigResult<&mut ConfigTable> {
        if name == "__GLOBAL__" {
            return Err(ConfigErr::Other(
                "Table name `__GLOBAL__` is reserved".into(),
            ));
        }
        if self.tables.contains_key(name) {
            return Err(ConfigErr::Other(format!("Duplicate table name `{}`", name)));
        }
        self.tables.insert(name.into(), ConfigTable::new());
        self.table_comments.insert(name.into(), comments.into());
        Ok(self.tables.get_mut(name).unwrap())
    }

    /// Returns the global table of the config.
    pub fn global_table(&self) -> &BTreeMap<String, ConfigItem> {
        &self.global
    }

    /// Returns the reference to the table with the specified name.
    pub fn table_at(&self, name: &str) -> Option<&BTreeMap<String, ConfigItem>> {
        self.tables.get(name)
    }

    /// Returns the mutable reference to the table with the specified name.
    pub fn table_at_mut(&mut self, name: &str) -> Option<&mut BTreeMap<String, ConfigItem>> {
        self.tables.get_mut(name)
    }

    /// Returns the reference to the config item with the specified table name and key.
    pub fn config_at(&self, table: &str, key: &str) -> Option<&ConfigItem> {
        self.table_at(table).and_then(|t| t.get(key))
    }

    /// Returns the mutable reference to the config item with the specified
    /// table name and key.
    pub fn config_at_mut(&mut self, table: &str, key: &str) -> Option<&mut ConfigItem> {
        self.table_at_mut(table).and_then(|t| t.get_mut(key))
    }

    /// Returns the comments of the table with the specified name.
    pub fn table_comments_at(&self, name: &str) -> Option<&str> {
        self.table_comments.get(name).map(|s| s.as_str())
    }

    /// Returns the iterator of all tables.
    ///
    /// The iterator returns a tuple of table name, table and comments. The
    /// global table is named `__GLOBAL__`.
    pub fn table_iter(&self) -> impl Iterator<Item = (&str, &ConfigTable, &str)> {
        let global_iter = [("__GLOBAL__", &self.global, "")].into_iter();
        let other_iter = self.tables.iter().map(|(name, configs)| {
            (
                name.as_str(),
                configs,
                self.table_comments.get(name).unwrap().as_str(),
            )
        });
        global_iter.chain(other_iter)
    }

    /// Returns the iterator of all config items.
    ///
    /// The iterator returns a tuple of table name, key and config item. The
    /// global table is named `__GLOBAL__`.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str, &ConfigItem)> {
        let global_iter = self
            .global
            .iter()
            .map(|(k, v)| ("__GLOBAL__", k.as_str(), v));
        let other_iter = self
            .table_iter()
            .flat_map(|(t, c, _)| c.iter().map(move |(k, v)| (t, k.as_str(), v)));
        global_iter.chain(other_iter)
    }
}

impl Config {
    /// Parse a toml string into a config object.
    pub fn from_toml(toml: &str) -> ConfigResult<Self> {
        let doc = toml.parse::<DocumentMut>()?;
        let table = doc.as_table();

        let mut result = Self::new();
        for (key, item) in table.iter() {
            match item {
                Item::Value(val) => {
                    result
                        .global
                        .insert(key.into(), ConfigItem::new(table, key, val)?);
                }
                Item::Table(table) => {
                    let comments = prefix_comments(table.decor());
                    let configs = result.new_table(key, comments.unwrap_or_default())?;
                    for (key, item) in table.iter() {
                        if let Item::Value(val) = item {
                            configs.insert(key.into(), ConfigItem::new(table, key, val)?);
                        } else {
                            return Err(ConfigErr::InvalidValue);
                        }
                    }
                }
                Item::None => {}
                _ => {
                    return Err(ConfigErr::Other(format!(
                        "Object array `[[{}]]` is not supported",
                        key
                    )))
                }
            }
        }
        Ok(result)
    }

    /// Dump the config into a string with the specified format.
    pub fn dump(&self, fmt: OutputFormat) -> ConfigResult<String> {
        let mut output = Output::new(fmt);
        for (name, table, comments) in self.table_iter() {
            if name != "__GLOBAL__" {
                output.table_begin(name, comments);
            }
            for (key, item) in table.iter() {
                if let Err(e) = output.write_item(item) {
                    eprintln!("Dump config `{}` failed: {:?}", key, e);
                }
            }
            if name != "__GLOBAL__" {
                output.table_end();
            }
        }
        Ok(output.result().into())
    }

    /// Merge the other config into self, if there is a duplicate key, return an error.
    pub fn merge(&mut self, other: &Self) -> ConfigResult<()> {
        for (name, other_table, table_comments) in other.table_iter() {
            let self_table = if name == "__GLOBAL__" {
                &mut self.global
            } else if let Some(table) = self.tables.get_mut(name) {
                table
            } else {
                self.new_table(name, table_comments)?
            };
            for (key, item) in other_table.iter() {
                if self_table.contains_key(key) {
                    return Err(ConfigErr::Other(format!("Duplicate key `{}`", key)));
                } else {
                    self_table.insert(key.into(), item.clone());
                }
            }
        }
        Ok(())
    }

    /// Update the values of self with the other config, if there is a key not found in self, skip it.
    pub fn update(&mut self, other: &Self) -> ConfigResult<()> {
        for (table_name, key, item) in other.iter() {
            let table = if table_name == "__GLOBAL__" {
                &mut self.global
            } else if let Some(table) = self.tables.get_mut(table_name) {
                table
            } else {
                continue;
            };

            if let Some(self_item) = table.get_mut(key) {
                if let Some(ty) = self_item.value.ty() {
                    if let Ok(new_value) =
                        ConfigValue::from_raw_value_type(item.value.value(), ty.clone())
                    {
                        self_item.value = new_value;
                    } else {
                        eprintln!("Type mismatch for key `{}`: expected `{:?}`", key, ty);
                        return Err(ConfigErr::ValueTypeMismatch);
                    }
                }
            }
        }
        Ok(())
    }
}

fn prefix_comments(decor: &Decor) -> Option<&str> {
    decor.prefix().and_then(|s| s.as_str())
}

fn suffix_comments(decor: &Decor) -> Option<&str> {
    decor.suffix().and_then(|s| s.as_str())
}
