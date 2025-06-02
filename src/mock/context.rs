use std::collections::HashMap;

use bytes::Bytes;

use super::JsonValue;

#[derive(Default)]
pub struct ExecutionContext {
    ret: Option<JsonValue>,
    variables: HashMap<String, Variable>,
}

impl ExecutionContext {
    pub fn return_value(&mut self, value: impl Into<JsonValue>) {
        if self.ret.is_some() {
            panic!("return_value can only be called once");
        }

        self.ret = Some(value.into());
    }

    pub fn ret(self) -> Option<JsonValue> {
        self.ret
    }

    pub fn set(&mut self, name: &str, value: impl Into<Variable>) {
        self.variables.insert(name.to_owned(), value.into());
    }

    pub fn get<T>(&self, name: &str) -> Option<Result<T, VariableError>>
    where
        T: TryFrom<Variable, Error = VariableError>,
    {
        let value = self.variables.get(name).cloned()?;
        if let Variable::Null = value {
            return None;
        }

        Some(T::try_from(value))
    }

    pub fn get_variable(&self, name: &str) -> Option<&Variable> {
        self.variables.get(name)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VariableError {
    #[error("Invalid variable type")]
    InvalidType,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Variable {
    String(String),
    Integer(isize),
    Number(f64),
    Boolean(bool),
    Bytes(Bytes),
    Null,
}

impl restate_sdk::serde::Serialize for Variable {
    type Error = serde_json::Error;

    fn serialize(&self) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(self).map(Bytes::from)
    }
}

impl restate_sdk::serde::Deserialize for Variable {
    type Error = serde_json::Error;

    fn deserialize(bytes: &mut Bytes) -> Result<Self, Self::Error> {
        serde_json::from_slice(bytes)
    }
}

impl From<String> for Variable {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<isize> for Variable {
    fn from(value: isize) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for Variable {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<bool> for Variable {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<Bytes> for Variable {
    fn from(value: Bytes) -> Self {
        Self::Bytes(value)
    }
}

impl From<Vec<u8>> for Variable {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value.into())
    }
}

impl From<()> for Variable {
    fn from(_: ()) -> Self {
        Self::Null
    }
}

impl TryFrom<Variable> for String {
    type Error = VariableError;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        let Variable::String(s) = value else {
            return Err(VariableError::InvalidType);
        };

        Ok(s)
    }
}

impl TryFrom<Variable> for isize {
    type Error = VariableError;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        let Variable::Integer(i) = value else {
            return Err(VariableError::InvalidType);
        };

        Ok(i)
    }
}

impl TryFrom<Variable> for f64 {
    type Error = VariableError;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        let Variable::Number(n) = value else {
            return Err(VariableError::InvalidType);
        };

        Ok(n)
    }
}

impl TryFrom<Variable> for bool {
    type Error = VariableError;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        let Variable::Boolean(b) = value else {
            return Err(VariableError::InvalidType);
        };

        Ok(b)
    }
}

impl TryFrom<Variable> for Bytes {
    type Error = VariableError;

    fn try_from(value: Variable) -> Result<Self, Self::Error> {
        let Variable::Bytes(b) = value else {
            return Err(VariableError::InvalidType);
        };

        Ok(b)
    }
}
