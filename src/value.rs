use std::fmt;

use toml_edit::Value;

use crate::{ConfigErr, ConfigResult, ConfigType};

#[derive(Clone)]
pub struct ConfigValue {
    value: Value,
    ty: Option<ConfigType>,
}

impl ConfigValue {
    pub fn new(s: &str) -> ConfigResult<Self> {
        let value = s.parse::<Value>()?;
        Self::new_with_value(&value)
    }

    pub fn new_with_type(s: &str, ty: &str) -> ConfigResult<Self> {
        let value = s.parse::<Value>()?;
        let ty = ConfigType::new(ty)?;
        Self::new_with_value_type(&value, ty)
    }

    pub(crate) fn new_with_value(value: &Value) -> ConfigResult<Self> {
        if !value_is_valid(value) {
            return Err(ConfigErr::InvalidValue);
        }
        Ok(Self {
            value: value.clone(),
            ty: None,
        })
    }

    pub(crate) fn new_with_value_type(value: &Value, ty: ConfigType) -> ConfigResult<Self> {
        if !value_is_valid(value) {
            return Err(ConfigErr::InvalidValue);
        }
        if value_type_matches(value, &ty) {
            Ok(Self {
                value: value.clone(),
                ty: Some(ty),
            })
        } else {
            Err(ConfigErr::ValueTypeMismatch)
        }
    }

    pub fn ty(&self) -> Option<&ConfigType> {
        self.ty.as_ref()
    }

    pub fn inferred_type(self) -> ConfigResult<ConfigType> {
        inferred_type(&self.value)
    }

    pub fn type_matches(&self, ty: &ConfigType) -> bool {
        value_type_matches(&self.value, ty)
    }

    pub fn to_toml(&self) -> String {
        to_toml(&self.value)
    }
}

impl fmt::Debug for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigValue")
            .field("value", &self.to_toml())
            .field("type", &self.ty)
            .finish()
    }
}

fn is_num(s: &str) -> bool {
    let s = s.to_lowercase().replace('_', "");
    if let Some(s) = s.strip_prefix("0x") {
        usize::from_str_radix(s, 16).is_ok()
    } else if let Some(s) = s.strip_prefix("0b") {
        usize::from_str_radix(s, 2).is_ok()
    } else if let Some(s) = s.strip_prefix("0o") {
        usize::from_str_radix(s, 8).is_ok()
    } else {
        s.parse::<usize>().is_ok()
    }
}

fn value_is_valid(value: &Value) -> bool {
    match value {
        Value::Boolean(_) | Value::Integer(_) | Value::String(_) => true,
        Value::Array(arr) => {
            for e in arr {
                if !value_is_valid(e) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

fn value_type_matches(value: &Value, ty: &ConfigType) -> bool {
    match (value, ty) {
        (Value::Boolean(_), ConfigType::Bool) => true,
        (Value::Integer(_), ConfigType::Int | ConfigType::Uint) => true,
        (Value::String(s), _) => {
            let s = s.value();
            if is_num(s) {
                matches!(ty, ConfigType::Int | ConfigType::Uint | ConfigType::String)
            } else {
                matches!(ty, ConfigType::String)
            }
        }
        (Value::Array(arr), ConfigType::Tuple(ty)) => {
            if arr.len() != ty.len() {
                return false;
            }
            for (e, t) in arr.iter().zip(ty.iter()) {
                if !value_type_matches(e, t) {
                    return false;
                }
            }
            true
        }
        (Value::Array(arr), ConfigType::Array(ty)) => {
            for e in arr {
                if !value_type_matches(e, ty) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

fn inferred_type(value: &Value) -> ConfigResult<ConfigType> {
    match value {
        Value::Boolean(_) => Ok(ConfigType::Bool),
        Value::Integer(i) => {
            let val = *i.value();
            if val < 0 {
                Ok(ConfigType::Int)
            } else {
                Ok(ConfigType::Uint)
            }
        }
        Value::String(s) => {
            let s = s.value();
            if is_num(s) {
                Ok(ConfigType::Uint)
            } else {
                Ok(ConfigType::String)
            }
        }
        Value::Array(arr) => {
            let types = arr
                .iter()
                .map(inferred_type)
                .collect::<ConfigResult<Vec<_>>>()?;
            if types.is_empty() {
                return Ok(ConfigType::Unknown);
            }

            let mut all_same = true;
            for t in types.iter() {
                if matches!(t, ConfigType::Unknown) {
                    return Ok(ConfigType::Unknown);
                }
                if t != &types[0] {
                    all_same = false;
                    break;
                }
            }

            if all_same {
                Ok(ConfigType::Array(Box::new(types[0].clone())))
            } else {
                Ok(ConfigType::Tuple(types))
            }
        }
        _ => Err(ConfigErr::InvalidValue),
    }
}

pub fn to_toml(value: &Value) -> String {
    match &value {
        Value::Boolean(b) => b.display_repr().to_string(),
        Value::Integer(i) => i.display_repr().to_string(),
        Value::String(s) => s.display_repr().to_string(),
        Value::Array(arr) => {
            let elements = arr.iter().map(to_toml).collect::<Vec<_>>();
            if arr.iter().any(|e| e.is_array()) {
                format!("[\n    {}\n]", elements.join(",\n").replace("\n", "\n    "))
            } else {
                format!("[{}]", elements.join(", "))
            }
        }
        _ => "".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigErr, ConfigResult, ConfigType, ConfigValue};

    fn check_type_infer(value: &str, expect_ty: &str) -> ConfigResult<()> {
        let value = ConfigValue::new(value)?;
        let expect = ConfigType::new(expect_ty)?;
        let inferred = value.inferred_type()?;
        if inferred != expect {
            println!("inferred: {:?}, expect: {:?}", inferred, expect);
            return Err(super::ConfigErr::ValueTypeMismatch);
        }
        Ok(())
    }

    macro_rules! assert_err {
        ($res:expr, $err:ident) => {
            match $res {
                Err(ConfigErr::$err) => {}
                _ => panic!("expected `Err({:?})`, got `{:?}`", ConfigErr::$err, $res),
            }
        };
    }

    #[test]
    fn test_type_infer() {
        macro_rules! check_infer {
            ($value:expr, $ty:expr) => {
                check_type_infer($value, $ty).unwrap();
            };
        }

        check_infer!("true", "bool");
        check_infer!("false", "bool");

        check_infer!("0", "uint");
        check_infer!("2333", "uint");
        check_infer!("-2333", "int");
        check_infer!("0b1010", "uint");
        check_infer!("0xdead_beef", "uint");

        check_infer!("\"0xffff_ffff_ffff_ffff\"", "uint");
        check_infer!("\"hello, world!\"", "str");
        check_infer!("\"0o777\"", "uint");
        check_infer!("\"0xx233\"", "str");
        check_infer!("\"\"", "str");

        check_infer!("[1, 2, 3]", "[uint]");
        check_infer!("[\"1\", \"2\", \"3\"]", "[uint]");
        check_infer!("[\"a\", \"b\", \"c\"]", "[str]");
        check_infer!("[true, false, true]", "[bool]");
        check_infer!("[\"0\", \"a\", true, -2]", "(uint, str, bool, int)");
        check_infer!("[]", "?");
        check_infer!("[[]]", "?");
        check_infer!("[[2, 3, 3, 3], [4, 5, 6, 7]]", "[[uint]]");
        check_infer!("[[1], [2, 3], [4, 5, 6]]", "[[uint]]");
        check_infer!(
            "[[2, 3, 3], [4, 5, \"abc\", 7]]",
            "([uint], (uint, uint, str, uint))"
        );
    }

    #[test]
    fn test_type_match() {
        macro_rules! check_match {
            ($value:expr, $ty:expr) => {
                ConfigValue::new_with_type($value, $ty).unwrap();
            };
        }
        macro_rules! check_mismatch {
            ($value:expr, $ty:expr) => {
                assert_err!(ConfigValue::new_with_type($value, $ty), ValueTypeMismatch);
            };
        }

        check_match!("true", "bool");
        check_match!("false", "bool");
        check_mismatch!("true", "int");

        check_match!("0", "uint");
        check_match!("0", "int");
        check_match!("2333", "int");
        check_match!("-2333", "uint");
        check_match!("0b1010", "int");
        check_match!("0xdead_beef", "int");

        check_mismatch!("\"abc\"", "uint");
        check_match!("\"0xffff_ffff_ffff_ffff\"", "uint");
        check_match!("\"0xffff_ffff_ffff_ffff\"", "str");
        check_match!("\"hello, world!\"", "str");
        check_match!("\"0o777\"", "uint");
        check_match!("\"0xx233\"", "str");
        check_match!("\"\"", "str");

        check_match!("[1, 2, 3]", "[uint]");
        check_match!("[\"1\", \"2\", \"3\"]", "[uint]");
        check_match!("[\"1\", \"2\", \"3\"]", "[str]");
        check_match!("[true, false, true]", "[bool]");
        check_match!("[\"0\", \"a\", true, -2]", "(uint, str, bool, int)");
        check_mismatch!("[\"0\", \"a\", true, -2]", "[uint]");
        check_match!("[]", "[int]");
        check_match!("[[]]", "[()]");
        check_match!("[[2, 3, 3, 3], [4, 5, 6, 7]]", "[[uint]]");
        check_match!("[[2, 3, 3, 3], [4, 5, 6, 7]]", "[(int, int, int, int)]");
        check_match!("[[1], [2, 3], [4, 5, 6]]", "[[uint]]");
        check_match!("[[1], [2, 3], [4, 5, 6]]", "([uint],[uint],[uint])");
        check_match!("[[1], [2, 3], [4, 5, 6]]", "((uint),(uint, uint),[uint])");
        check_match!(
            "[[2, 3, 3], [4, 5, \"abc\", 7]]",
            "((int, int, int), (uint, uint, str, uint))"
        );
        check_match!("[[1,2], [3,4], [5,6,7]]", "[[uint]]");
        check_match!("[[1,2], [3,4], [5,6,7]]", "([uint], [uint], [uint])");
        check_match!("[[1,2], [3,4], [5,6,7]]", "((uint,uint), [uint], [uint])");
        check_mismatch!("[[1,2], [3,4], [5,6,7]]", "[(uint, uint)]");
        check_match!("[[[[],[]],[[]]],[]]", "[[[[uint]]]]");
    }

    #[test]
    fn test_err() {
        assert_err!(ConfigType::new("Bool"), InvalidType);
        assert_err!(ConfigType::new("usize"), InvalidType);
        assert_err!(ConfigType::new(""), InvalidType);
        assert_err!(ConfigType::new("&str"), InvalidType);
        assert_err!(ConfigType::new("[]"), InvalidType);
        assert_err!(ConfigType::new("(("), InvalidType);
        assert_err!(ConfigType::new("(int,"), InvalidType);
        assert_err!(ConfigType::new("(,)"), InvalidType);
        assert_err!(ConfigType::new("(uint,)"), InvalidType);
        assert_err!(ConfigType::new("[uint, uint]"), InvalidType);
        assert_err!(ConfigType::new("()()"), InvalidType);
        assert_err!(ConfigType::new("(()())"), InvalidType);
        assert!(ConfigType::new("((),())").is_ok());
        assert!(ConfigType::new("(  )").is_ok());
        assert_err!(ConfigValue::new("233.0"), InvalidValue);
    }
}
