use std::collections::BTreeMap;
use toml_edit::{Decor, DocumentMut, Item, Table, Value};

use crate::ConfigValue;
use crate::{ConfigErr, ConfigResult, ConfigType};

type ConfigTable = BTreeMap<String, ConfigItem>;

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
                ConfigValue::new_with_value_type(value, ty)?
            } else {
                ConfigValue::new_with_value(value)?
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

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &ConfigValue {
        &self.value
    }

    pub fn comments(&self) -> &str {
        &self.comments
    }
}

#[derive(Default, Debug)]
pub struct Config {
    global: ConfigTable,
    tables: BTreeMap<String, ConfigTable>,
    table_comments: BTreeMap<String, String>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            global: ConfigTable::new(),
            tables: BTreeMap::new(),
            table_comments: BTreeMap::new(),
        }
    }

    fn new_table(&mut self, name: &str, comments: &str) -> &mut ConfigTable {
        self.tables.insert(name.into(), ConfigTable::new());
        self.table_comments.insert(name.into(), comments.into());
        self.tables.get_mut(name).unwrap()
    }

    pub fn global_table(&self) -> &BTreeMap<String, ConfigItem> {
        &self.global
    }

    pub fn table_at(&self, name: &str) -> Option<&BTreeMap<String, ConfigItem>> {
        self.tables.get(name)
    }

    pub fn table_at_mut(&mut self, name: &str) -> Option<&mut BTreeMap<String, ConfigItem>> {
        self.tables.get_mut(name)
    }

    pub fn config_at(&self, table: &str, key: &str) -> Option<&ConfigItem> {
        self.table_at(table).and_then(|t| t.get(key))
    }

    pub fn config_at_mut(&mut self, table: &str, key: &str) -> Option<&mut ConfigItem> {
        self.table_at_mut(table).and_then(|t| t.get_mut(key))
    }

    pub fn table_comments_at(&self, name: &str) -> Option<&str> {
        self.table_comments.get(name).map(|s| s.as_str())
    }

    pub fn table_iter(&self) -> impl Iterator<Item = (&str, &ConfigTable, &str)> {
        self.tables.iter().map(|(name, configs)| {
            (
                name.as_str(),
                configs,
                self.table_comments.get(name).unwrap().as_str(),
            )
        })
    }

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
                    let configs = result.new_table(key, comments.unwrap_or_default());
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

    pub fn gen_toml(&self) -> ConfigResult<String> {
        fn gen_toml_inner(output: &mut String, table: &ConfigTable) {
            for (key, item) in table.iter() {
                output.push_str(&format!(
                    "{}\n{} = {}\n",
                    item.comments.trim(),
                    key,
                    item.value.to_toml()
                ));
            }
        }

        let mut output = String::new();
        gen_toml_inner(&mut output, &self.global);
        for (name, table, comments) in self.table_iter() {
            output.push_str(&format!("{}[{}]\n", comments, name));
            gen_toml_inner(&mut output, table);
        }
        Ok(output)
    }

    pub fn merge(&mut self, other: &Self) -> ConfigResult<()> {
        let insert_config = |table: &mut ConfigTable, key: &str, item| {
            if table.contains_key(key) {
                Err(ConfigErr::Other(format!("Duplicate key `{}`", key)))
            } else {
                table.insert(key.into(), item);
                Ok(())
            }
        };

        for (key, item) in other.global.iter() {
            insert_config(&mut self.global, key, item.clone())?;
        }
        for (name, other_table, table_comments) in other.table_iter() {
            let self_table = if let Some(table) = self.tables.get_mut(name) {
                table
            } else {
                self.new_table(name, table_comments)
            };
            for (key, item) in other_table.iter() {
                insert_config(self_table, key, item.clone())?;
            }
        }
        Ok(())
    }

    pub fn update(&mut self, other: &Self) -> ConfigResult<()> {
        let update_config = |table: &mut ConfigTable, key: &str, item: &ConfigItem| {
            if let Some(self_item) = table.get_mut(key) {
                if let Some(ty) = self_item.value.ty() {
                    if !item.value.type_matches(ty) {
                        eprintln!("Type mismatch for key `{}`: expected `{:?}`", key, ty,);
                        return Err(ConfigErr::ValueTypeMismatch);
                    } else {
                        self_item.value = item.value.clone();
                    }
                }
                Ok(())
            } else {
                // skip if key not found
                Ok(())
            }
        };

        for (table_name, key, item) in other.iter() {
            if table_name == "__GLOBAL__" {
                update_config(&mut self.global, key, item)?;
            } else if let Some(table) = self.tables.get_mut(table_name) {
                update_config(table, key, item)?;
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
