use actix_web::{error, get, post, web, HttpRequest, HttpResponse, HttpServer};
use qstring::QString;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use super::cache;

pub struct Handler {
    cache: cache::Memstore,
    client: reqwest::Client,
    ttl: Duration,
    filter: Filter,
}

struct Filter {
    exts: HashMap<String, bool>,
    headers: HashMap<String, bool>,
}

impl Handler {
    pub async fn proxy(&self, req: HttpRequest) -> Result<HttpResponse, Err> {
        sanitize_request(&req);
        let get = self.client.get(req);
        let resp = self.client.execute(get.build()?)?.await;
        match resp.error_for_status() {
            Some(_r) => (),
            Err(e) => {
                return Result::Err(error::ErrorBadGateway(format!("downstream error: {}", e)))
            }
        }

        let len = match resp.headers().get("Content-length") {
            None => 0,
            Some(n) => n,
        };

        let body = Vec::with_capacity(len as usize);
        let cacheable = self.filter.is_cacheable(&req);
        if cacheable {
            self.cache
                .store(ds_uri, resp.bytes_stream(), body, len)
                .await?;
        }

        let body = serde_json::to_string(&body)?;

        Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(body))
    }
}

impl Filter {
    // is_cacheable tries to determine if a file is cacheable. It initially tries to get a file extension,
    // and sees if it is in the configured extensions. If it cannot get an extension or the extension is not
    // valid, it checks the Content-type header against the configured header values. Note that a failure to get or
    // handle an extension is a fall-through, but for extensions will cause it to be considered non-cacheable.
    fn is_cacheable(&self, req: &HttpRequest) -> bool {
        // add extension logic
        let file = req.uri().path().split("/").next_back();
        let file_cache = match file {
            Some(s) => {
                let ext = s.split(".").next_back()?;
                self.valid_ext(ext)
            },
            None => false,
        };

        if file_cache {
            return true
        }

        return match req.headers().get("Content-type") {
            Some(s) => {
                self.valid_ctype(s)
            },
            _ => false,
        };
    }

    fn valid_ctype(&self, ctype: &str) -> bool {
        self.headers.contains_key(ctype)
    }

    fn valid_ext(&self, ctype: &str) -> bool {
        self.headers.contains_key(ctype)
    }
}

// sanitize_request encapsulates the logic that is used to ensure a given request is valid for use by
// our client.
fn sanitize_request(req: &HttpRequest) {
    req.headers().remove("Proxy-connection")
}
