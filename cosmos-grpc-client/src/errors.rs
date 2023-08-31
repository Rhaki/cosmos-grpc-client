use cosmwasm_std::{StdError, StdResult};

#[allow(clippy::wrong_self_convention)]
pub trait IntoStdError: std::error::Error {
    fn into_std_error(&self) -> StdError {
        StdError::generic_err(self.to_string())
    }
}

pub trait IntoStdResult<T> {
    fn into_std_result(self) -> StdResult<T>;
}

impl<T, E> IntoStdResult<T> for Result<T, E>
where
    E: std::error::Error,
{
    fn into_std_result(self) -> StdResult<T> {
        self.map_err(|err| StdError::generic_err(err.to_string()))
    }
}
