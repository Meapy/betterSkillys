//! Account server requests: form POSTs answered with XML (or `<Error>...</Error>`), as
//! `AppEngineRetryLoader.as` sends them.

use crate::Pending;

pub type Response = Result<String, String>;

/// POST `params` as a form to `url`. The body arrives later in the returned `Pending`.
pub fn post_form(url: &str, params: &[(&str, &str)]) -> Pending<Response> {
    let (pending, slot) = Pending::new();
    let body = encode_form(params);
    imp::post(url.to_owned(), body, move |r| *slot.lock().unwrap() = Some(r));
    pending
}

/// GET `url`'s bytes (sounds, music). The body arrives later in the returned `Pending`.
pub fn get_bytes(url: &str) -> Pending<Result<Vec<u8>, String>> {
    let (pending, slot) = Pending::new();
    imp::get(url.to_owned(), move |r| *slot.lock().unwrap() = Some(r));
    pending
}

pub fn encode_form(params: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for (i, (k, v)) in params.iter().enumerate() {
        if i > 0 {
            out.push('&');
        }
        escape(k, &mut out);
        out.push('=');
        escape(v, &mut out);
    }
    out
}

fn escape(s: &str, out: &mut String) {
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::sync::OnceLock;

    fn agent() -> &'static ureq::Agent {
        static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
        AGENT.get_or_init(|| crate::tls::agent_builder().build())
    }

    pub fn get(url: String, done: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static) {
        std::thread::spawn(move || {
            let result = match agent().get(&url).timeout(std::time::Duration::from_secs(30)).call() {
                Ok(r) => {
                    let mut bytes = Vec::new();
                    std::io::Read::read_to_end(&mut r.into_reader(), &mut bytes).map(|_| bytes).map_err(|e| e.to_string())
                }
                Err(e) => Err(e.to_string()),
            };
            done(result);
        });
    }

    pub fn post(url: String, body: String, done: impl FnOnce(super::Response) + Send + 'static) {
        std::thread::spawn(move || {
            let result = match agent().post(&url)
                .set("Content-Type", "application/x-www-form-urlencoded")
                .timeout(std::time::Duration::from_secs(20))
                .send_string(&body)
            {
                Ok(r) => r.into_string().map_err(|e| e.to_string()),
                // Some answers come with an error status but an <Error> body that says why
                // (purchaseCharSlot answers 404 "Insufficient funds"): keep the body.
                Err(ureq::Error::Status(code, r)) => match r.into_string() {
                    Ok(text) if text.trim_start().starts_with('<') => Ok(text),
                    _ => Err(format!("{url}: status code {code}")),
                },
                Err(e) => Err(e.to_string()),
            };
            done(result);
        });
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    pub fn post(url: String, body: String, done: impl FnOnce(super::Response) + 'static) {
        wasm_bindgen_futures::spawn_local(async move {
            done(fetch(&url, &body).await);
        });
    }

    pub fn get(url: String, done: impl FnOnce(Result<Vec<u8>, String>) + 'static) {
        wasm_bindgen_futures::spawn_local(async move {
            done(fetch_bytes(&url).await);
        });
    }

    async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
        let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
        let window = web_sys::window().ok_or("no window")?;
        let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url)).await.map_err(err)?.dyn_into().map_err(err)?;
        if !response.ok() {
            return Err(format!("HTTP {}", response.status()));
        }
        let buffer = JsFuture::from(response.array_buffer().map_err(err)?).await.map_err(err)?;
        Ok(js_sys::Uint8Array::new(&buffer).to_vec())
    }

    async fn fetch(url: &str, body: &str) -> super::Response {
        let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
        let init = web_sys::RequestInit::new();
        init.set_method("POST");
        init.set_body(&wasm_bindgen::JsValue::from_str(body));
        let request = web_sys::Request::new_with_str_and_init(url, &init).map_err(err)?;
        request.headers().set("Content-Type", "application/x-www-form-urlencoded").map_err(err)?;
        let window = web_sys::window().ok_or("no window")?;
        let response: web_sys::Response =
            JsFuture::from(window.fetch_with_request(&request)).await.map_err(err)?.dyn_into().map_err(err)?;
        let status = response.status();
        let text = JsFuture::from(response.text().map_err(err)?).await.map_err(err)?;
        let text = text.as_string().ok_or("response isn't text")?;
        // An error status with an <Error> body: the body says why (see the native side).
        if !response.ok() && !text.trim_start().starts_with('<') {
            return Err(format!("HTTP {status}"));
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn form_encoding() {
        assert_eq!(super::encode_form(&[("guid", "a b@c.de"), ("password", "p&w=1")]), "guid=a+b%40c.de&password=p%26w%3D1");
    }
}
