use crate::errors::ApiError;
use axum::{
    Json,
    extract::{
        FromRequest, FromRequestParts, Path, Query,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    http::request::Parts,
};
use serde::de::DeserializeOwned;

#[derive(Debug)]
pub struct ApiPath<T>(pub T);

#[derive(Debug)]
pub struct ApiQuery<T>(pub T);

#[derive(Debug)]
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(map_path_rejection)?;

        Ok(Self(value))
    }
}

impl<S, T> FromRequestParts<S> for ApiQuery<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(map_query_rejection)?;

        Ok(Self(value))
    }
}

impl<S, T> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(
        request: axum::extract::Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(request, state)
            .await
            .map_err(map_json_rejection)?;

        Ok(Self(value))
    }
}

fn map_path_rejection(_error: PathRejection) -> ApiError {
    ApiError::Validation {
        code: "invalid_path_parameters",
        message: "path parameters are invalid",
    }
}

fn map_query_rejection(_error: QueryRejection) -> ApiError {
    ApiError::Validation {
        code: "invalid_query_parameters",
        message: "query parameters are invalid",
    }
}

fn map_json_rejection(error: JsonRejection) -> ApiError {
    match error {
        JsonRejection::MissingJsonContentType(_) => ApiError::UnsupportedMediaType {
            code: "invalid_content_type",
            message: "request content-type must be application/json",
        },
        JsonRejection::JsonSyntaxError(_) | JsonRejection::BytesRejection(_) => {
            ApiError::Validation {
                code: "invalid_json_body",
                message: "request body is invalid",
            }
        }
        JsonRejection::JsonDataError(_) => ApiError::Validation {
            code: "invalid_request_body",
            message: "request body does not match the expected schema",
        },
        _ => ApiError::Validation {
            code: "invalid_json_body",
            message: "request body is invalid",
        },
    }
}
