mod config;
mod ty;
mod value;

use toml_edit::TomlError;

pub use self::config::{Config, ConfigItem};
pub use self::ty::ConfigType;
pub use self::value::ConfigValue;

pub enum ConfigErr {
    Parse(TomlError),
    InvalidValue,
    InvalidType,
    ValueTypeMismatch,
    Other(String),
}

impl From<TomlError> for ConfigErr {
    fn from(e: TomlError) -> Self {
        Self::Parse(e)
    }
}

impl core::fmt::Display for ConfigErr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "{}", e),
            Self::InvalidValue => write!(f, "Invalid value type"),
            Self::InvalidType => write!(f, "Invalid value type"),
            Self::ValueTypeMismatch => write!(f, "Value and type mismatch"),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

impl core::fmt::Debug for ConfigErr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

pub type ConfigResult<T> = Result<T, ConfigErr>;
