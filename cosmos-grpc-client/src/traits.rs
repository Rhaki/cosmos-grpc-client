use std::fmt::Display;

use anyhow::anyhow;

use crate::AnyResult;

pub trait IntoSerdeSerialize: serde::Serialize {
    fn json_serialize(&self) -> AnyResult<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }
}

pub trait IntoAnyhowError {
    fn into_anyerror(self) -> anyhow::Error;
}

impl<T> IntoAnyhowError for T
where
    T: Display,
{
    fn into_anyerror(self) -> anyhow::Error {
        anyhow!("{}", self)
    }
}

pub trait IntoAnyhowResult {
    type Output;
    fn into_anyresult(self) -> AnyResult<Self::Output>;
}

impl<T, E> IntoAnyhowResult for Result<T, E>
where
    E: Display,
{
    type Output = T;

    fn into_anyresult(self) -> AnyResult<T> {
        self.map_err(|err| anyhow!("{}", err))
    }
}

pub trait OkOrAny {
    type Output;
    fn ok_or_any(self, error: &str) -> AnyResult<Self::Output>;
}

impl<T> OkOrAny for Option<T> {
    type Output = T;
    fn ok_or_any(self, error: &str) -> AnyResult<T> {
        self.ok_or(anyhow!("{}", error))
    }
}
