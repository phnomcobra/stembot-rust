use actix_web::{web, HttpRequest, HttpResponse, Result};
use reqwest::header::{HeaderName, HeaderValue};
use std::error::Error;

use crate::models::config::Config;


pub async fn test_handler(
    body: web::Bytes,
    _config: web::Data<Config>,
    _request: HttpRequest,
) -> Result<HttpResponse, Box<dyn Error>> {
    let mut response = HttpResponse::Ok().body(format!("test handler received body: {}", String::from_utf8_lossy(&body)));
    let headers = response.headers_mut();
    headers.append(HeaderName::from_static("test-header"), HeaderValue::from_static("test-value"));
    Ok(response)
}