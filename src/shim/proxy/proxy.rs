use actix_web::{get, post, web, error, HttpResponse, HttpServer, HttpRequest};
use qstring::QString;
use std::collections::HashMap;
use reqwest::Client;
use std::time::Duration;
use serde::{Serialize, Deserialize}

use super::cache;

#[derive(Serialize, Deserialize)]
pub struct Response {
    cached: bool,
    ttl: Duration,
}

pub struct Handler {
    exts: HashMap<String, bool>,
    cache: cache::Cache,
    client: reqwest::Client,
    ttl: Duration,
}

impl Handler {
    #[get("/proxy")]
    pub async fn proxy(&self, req: HttpRequest) -> Result<impl Responder> {
        // get downstream URL from parameters
        let query = Qstring::from(req.query_string());
        let downstream: String = query.get("ds").ok_or(error::ErrorBadRequest("no ds query parameter"))?;

        // get extension from downstream URL and check if cacheable
        let ds_uri = match downstream.parse::<actix_web::http::Uri>() {
            Err(e) => return error::ErrorBadRequest(format!("unparseable query parameter: {}", e)),
            Some(u) => u,
        }
        let cacheable = exts.get(ds_uri.path().split("/").collect().last()).unwrap_or_default();

        let get = self.client.get(ds_uri);
        let resp = self.client.execute(get.build()).await?;
        match resp.error_for_status() {
            Some(_r) => (),
            Err(e) => return error::ErrorBadGateway(format!("downstream error: {}", e)),
        }

        let len = match resp.headers.get("Content-length")  {
            None => 0,
            Some(n) => n,
        }

        let resp = Response{ttl: -1, cache: false};
        if cacheable {
            self.cache.store(ds_uri, resp.body, len).await?;
            resp.cached = true;
            resp.ttl = self.ttl;
        }

        let body = serde_json::to_string(&resp)?;
        HttpResponse::Ok()
        .content_type("application/json")
        .body(body)
    }
}