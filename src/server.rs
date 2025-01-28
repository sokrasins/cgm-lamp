pub mod server {
    use crate::settings::settings::{Store, AppSettings};
    use embedded_svc::{
        http::{Headers, Method},
        io::{Read, Write},
    };
    use esp_idf_svc::http::server::EspHttpServer;
    use log::info;
    use std::sync::mpsc::Sender;
    use std::sync::{Arc, Mutex};

    static INDEX_HTML: &str = include_str!("index.html");

    const STACK_SIZE: usize = 10240;

    // Need lots of stack to parse JSON
    // Max payload length
    const MAX_LEN: usize = 128;

    pub struct Server<'a> {
        server: Option<EspHttpServer<'a>>,
        tx_channel: Sender<AppSettings>,
        settings: Arc<Mutex<AppSettings>>,
    }

    impl<'a> Server<'a> {
        pub fn new(store: &Store) -> Self {
            Server {
                server: None,
                tx_channel: store.tx_channel(),
                settings: store.settings(),
            }
        }

        // Start server listeners
        pub fn start(&mut self) -> anyhow::Result<()> {
            let server_configuration = esp_idf_svc::http::server::Configuration {
                stack_size: STACK_SIZE,
                ..Default::default()
            };

            // Clone tx_channel to give it to the server handler
            let tx = self.tx_channel.clone();
            let settings: Arc<Mutex<AppSettings>> = Arc::clone(&self.settings);

            self.server = Some(EspHttpServer::new(&server_configuration).unwrap());

            // Listener: serve the config page
            self.server
                .as_mut()
                .unwrap()
                .fn_handler("/", Method::Get, |req| {
                    req.into_ok_response()?
                        .write_all(INDEX_HTML.as_bytes())
                        .map(|_| ())
                })?;

            // Listener: Handle new settings from the web app
            self.server
                .as_mut()
                .unwrap()
                .fn_handler::<anyhow::Error, _>("/post", Method::Post, move |mut req| {
                    let len = req.content_len().unwrap_or(0) as usize;

                    if len > MAX_LEN {
                        req.into_status_response(413)?
                            .write_all("Request too big".as_bytes())?;
                        return Ok(());
                    }

                    let mut buf = vec![0; len];
                    req.read_exact(&mut buf)?;
                    let mut resp = req.into_ok_response()?;

                    if let Ok(form) = serde_json::from_slice::<AppSettings>(&buf) {
                        info!("Got new settings: {:?}", form);
                        tx.send(form).unwrap();
                        write!(resp, "New settings applied")?;
                    } else {
                        resp.write_all("JSON error".as_bytes())?;
                    }

                    Ok(())
                })?;

            // Listener: Serve the device's status when on request
            self.server
                .as_mut()
                .unwrap()
                .fn_handler("/state", Method::Get, move |req| {
                    info!("Get request on /state!");

                    // Acquire lock on sapp state
                    let state_guard = settings.lock().unwrap();
                    let state = (*state_guard).clone();
                    std::mem::drop(state_guard);
                    

                    info!("{:?}", state);

                    // Serialize, send back to web app
                    let state_ser = serde_json::to_string(&state).unwrap();
                    req.into_ok_response()?
                        .write_all(state_ser.as_bytes())
                        .map(|_| ())
                })?;

            Ok(())
        }
    }
}
