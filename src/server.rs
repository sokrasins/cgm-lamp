pub mod server {
    use embedded_svc::{
        http::{Headers, Method},
        io::{Read, Write},
    };
    use log::info;
    use esp_idf_svc::http::server::EspHttpServer;
    use serde::Deserialize;

    static INDEX_HTML: &str = include_str!("index.html");

    const STACK_SIZE: usize = 10240;

    // Need lots of stack to parse JSON
    // Max payload length
    const MAX_LEN: usize = 128;

    #[derive(Deserialize)]
    struct FormData<'a> {
        wifi_name: &'a str,
        wifi_pass: &'a str,
    }

    pub struct Server<'a> {
        server: Option<EspHttpServer<'a>>,
    }

    impl<'a> Server<'a> {
        pub fn new() -> Self {
            Server { server: None }
        }

        // TODO: Add a callback parameter here?
        pub fn start(&mut self) -> anyhow::Result<()> {

            let server_configuration = esp_idf_svc::http::server::Configuration {
                stack_size: STACK_SIZE,
                ..Default::default()
            };

            self.server = Some(EspHttpServer::new(&server_configuration).unwrap());

            self.server.as_mut().unwrap().fn_handler("/", Method::Get, |req| {
                req.into_ok_response()?
                    .write_all(INDEX_HTML.as_bytes())
                    .map(|_| ())
            })?;

            self.server.as_mut().unwrap()
                .fn_handler::<anyhow::Error, _>("/post", Method::Post, |mut req| {
                    let len = req.content_len().unwrap_or(0) as usize;

                    if len > MAX_LEN {
                        req.into_status_response(413)?
                            .write_all("Request too big".as_bytes())?;
                        return Ok(());
                    }

                    let mut buf = vec![0; len];
                    req.read_exact(&mut buf)?;
                    let mut resp = req.into_ok_response()?;

                    if let Ok(form) = serde_json::from_slice::<FormData>(&buf) {
                        write!(
                            resp,
                            "New settings applied"
                        )?; 
                        info!("Got new wifi creds - SSID: {} pass: {}", form.wifi_name, form.wifi_pass);
                    } else {
                        resp.write_all("JSON error".as_bytes())?;
                    }

                    Ok(())
                })?;

            Ok(())
        }
    }
}
