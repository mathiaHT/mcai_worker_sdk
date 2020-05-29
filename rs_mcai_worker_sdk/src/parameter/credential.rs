use crate::{
  config::*,
  job::{Session, SessionBody, SessionResponseBody, ValueResponseBody},
};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::Deserialize;

// TODO update version
#[deprecated(
  since = "0.10.3",
  note = "Please use the store field in Parameter instead"
)]
#[derive(Debug, PartialEq, Deserialize)]
pub struct Credential {
  #[serde(flatten)]
  pub key: String,
}

// TODO handle store code
pub fn request_value(credential_key: &str, _store_code: &str) -> Result<String, String> {
  let backend_endpoint = get_backend_hostname();
  let backend_username = get_backend_username();
  let backend_password = get_backend_password();

  let session_url = format!("{}/sessions", backend_endpoint);
  let credential_url = format!("{}/credentials/{}", backend_endpoint, credential_key);

  let client = Client::builder().build().map_err(|e| format!("{:?}", e))?;

  let session_body = SessionBody {
    session: Session {
      email: backend_username,
      password: backend_password,
    },
  };

  let response: SessionResponseBody = client
    .post(&session_url)
    .json(&session_body)
    .send()
    .map_err(|e| format!("{:?}", e))?
    .json()
    .map_err(|e| format!("{:?}", e))?;

  let mut headers = HeaderMap::new();

  headers.insert(
    AUTHORIZATION,
    HeaderValue::from_str(&response.access_token).map_err(|e| format!("{:?}", e))?,
  );

  let client = Client::builder()
    .default_headers(headers)
    .build()
    .map_err(|e| format!("{:?}", e))?;

  let response: ValueResponseBody = client
    .get(&credential_url)
    .send()
    .map_err(|e| format!("{:?}", e))?
    .json()
    .map_err(|e| format!("{:?}", e))?;

  Ok(response.data.value)
}
