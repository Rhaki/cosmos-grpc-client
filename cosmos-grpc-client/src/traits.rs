use std::fmt::Display;

use anyhow::anyhow;

use crate::AnyResult;

pub trait IntoSerdeSerialize: serde::Serialize {
    fn json_serialize(&self) -> AnyResult<Vec<u8>> {
        Ok(serde_json_wasm::to_vec(self)?)
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

/// A trait for converting different `Any` types into a [`prost_types::Any`].
pub trait SharedAny: Clone {
    fn into_any(self) -> prost_types::Any;
}

impl SharedAny for osmosis_std::shim::Any {
    fn into_any(self) -> prost_types::Any {
        prost_types::Any {
            type_url: self.type_url,
            value: self.value,
        }
    }
}

impl SharedAny for prost_types::Any {
    fn into_any(self) -> prost_types::Any {
        self
    }
}

/// Enable conversation of [`prost::Message`] type into [`prost_types::Any`].
pub trait AnyBuilder {
    fn build_any(self, type_url: impl Into<String>) -> prost_types::Any;
}

impl<T: prost::Message> AnyBuilder for T {
    fn build_any(self, type_url: impl Into<String>) -> prost_types::Any {
        let type_url = type_url.into();
        let value = self.encode_to_vec();
        prost_types::Any { type_url, value }
    }
}
