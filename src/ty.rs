use crate::{ConfigErr, ConfigResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigType {
    /// Boolean type.
    Bool,
    /// Signed integer type.
    Int,
    /// Unsigned integer type.
    Uint,
    /// String type.
    String,
    /// Array of tuples.
    Tuple(Vec<ConfigType>),
    /// Array type.
    Array(Box<ConfigType>),
    /// Type is unknown.
    Unknown,
}

impl ConfigType {
    pub fn new(ty: &str) -> ConfigResult<Self> {
        #[cfg(test)]
        if ty == "?" {
            return Ok(Self::Unknown);
        }

        let ty = ty.replace(" ", "");
        match ty.as_str() {
            "bool" => Ok(Self::Bool),
            "int" => Ok(Self::Int),
            "uint" => Ok(Self::Uint),
            "str" => Ok(Self::String),
            _ => {
                if ty.starts_with("(") && ty.ends_with(")") {
                    let tuple = &ty[1..ty.len() - 1];
                    if tuple.is_empty() {
                        return Ok(Self::Tuple(Vec::new()));
                    }
                    let items = split_tuple_items(tuple).ok_or(ConfigErr::InvalidType)?;
                    let tuple_types = items
                        .into_iter()
                        .map(Self::new)
                        .collect::<ConfigResult<Vec<_>>>()?;
                    Ok(Self::Tuple(tuple_types))
                } else if ty.starts_with('[') && ty.ends_with("]") {
                    let element = &ty[1..ty.len() - 1];
                    if element.is_empty() {
                        return Err(ConfigErr::InvalidType);
                    }
                    Ok(Self::Array(Box::new(Self::new(element)?)))
                } else {
                    Err(ConfigErr::InvalidType)
                }
            }
        }
    }

    pub fn to_rust_type(&self) -> String {
        match self {
            Self::Bool => "bool".into(),
            Self::Int => "isize".into(),
            Self::Uint => "usize".into(),
            Self::String => "&str".into(),
            Self::Tuple(items) => {
                let items = items
                    .iter()
                    .map(Self::to_rust_type)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", items)
            }
            Self::Array(ty) => format!("Vec<{}>", ty.to_rust_type()),
            _ => panic!("Unknown type"),
        }
    }
}

fn split_tuple_items(s: &str) -> Option<Vec<&str>> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut level = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => level += 1,
            ')' => level -= 1,
            ',' if level == 0 => {
                if start < i {
                    items.push(&s[start..i]);
                } else {
                    return None;
                }
                start = i + 1;
            }
            _ => {}
        }
        if level < 0 {
            return None;
        }
    }
    if level == 0 && start < s.len() {
        items.push(&s[start..]);
        Some(items)
    } else {
        None
    }
}