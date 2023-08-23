use cosmwasm_std::{StdError, StdResult};

pub trait IntoStdError {
    fn into_std_error(self) -> StdError;
}

impl IntoStdError for tonic::transport::Error {
    fn into_std_error(self) -> StdError {
        StdError::generic_err(self.to_string())
    }
}

pub trait IntoStdResult<T> {
    fn into_std_result(self) -> StdResult<T>;
}

impl<T> IntoStdResult<T> for Result<T, tonic::Status> {
    fn into_std_result(self) -> StdResult<T> {
        self.map_err(|err| StdError::generic_err(err.to_string()))
    }
}

impl<T> IntoStdResult<T> for Result<T, tonic::transport::Error> {
    fn into_std_result(self) -> StdResult<T> {
        self.map_err(|err| StdError::generic_err(err.to_string()))
    }
}

impl<T> IntoStdResult<T> for Result<T, bip32::Error> {
    fn into_std_result(self) -> StdResult<T> {
        self.map_err(|err| StdError::generic_err(err.to_string()))
    }
}

impl<T> IntoStdResult<T> for Result<T, bip39::Error> {
    fn into_std_result(self) -> StdResult<T> {
        self.map_err(|err| StdError::generic_err(err.to_string()))
    }
}