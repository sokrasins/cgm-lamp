pub mod server {
    use embedded_svc::{
        http::{Headers, Method},
        io::{Read, Write},
    };
    use esp_idf_svc::http::server::EspHttpServer;
    use serde::Deserialize;

    static INDEX_HTML: &str = include_str!("index.html");

    const STACK_SIZE: usize = 10240;

    // Need lots of stack to parse JSON
    // Max payload length
    const MAX_LEN: usize = 128;

    #[derive(Deserialize)]
    struct FormData<'a> {
        first_name: &'a str,
        age: u32,
        birthplace: &'a str,
    }

    pub struct Server<'a> {
        server: EspHttpServer<'a>,
    }

    impl<'a> Server<'a> {
        pub fn new() -> Self {
            let server_configuration = esp_idf_svc::http::server::Configuration {
                stack_size: STACK_SIZE,
                ..Default::default()
            };

            let server = EspHttpServer::new(&server_configuration).unwrap();

            Server { server }
        }

        // TODO: Add a callback parameter here?
        pub fn start(&mut self) -> anyhow::Result<()> {
            self.server.fn_handler("/", Method::Get, |req| {
                req.into_ok_response()?
                    .write_all(INDEX_HTML.as_bytes())
                    .map(|_| ())
            })?;

            self.server
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
                            "Hello, {}-year-old {} from {}!",
                            form.age, form.first_name, form.birthplace
                        )?;
                    } else {
                        resp.write_all("JSON error".as_bytes())?;
                    }

                    Ok(())
                })?;

            Ok(())
        }
    }
}
