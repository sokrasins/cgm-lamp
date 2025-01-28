pub mod server {
    use crate::settings::settings::AppSettings;
    use embedded_svc::{
        http::{Headers, Method},
        io::{Read, Write},
    };
    use esp_idf_svc::http::server::EspHttpServer;
    use log::info;
    use crate::settings::settings::Store;

    static INDEX_HTML: &str = include_str!("index.html");

    const STACK_SIZE: usize = 10240;

    // Need lots of stack to parse JSON
    // Max payload length
    const MAX_LEN: usize = 128;

    /*#[derive(Debug, Deserialize)]
    pub struct AppDelta<'a> {
        ap_ssid: Option<&'a str>,
        ap_pass: Option<&'a str>,
        dexcom_user: Option<&'a str>,
        dexcom_pass: Option<&'a str>,
        lamp_brightness: Option<usize>,
    }*/

    pub struct Server<'a> {
        server: Option<EspHttpServer<'a>>,
        store: &'a mut Store<'a>
    }

    impl<'a> Server<'a> {
        pub fn new(store: &'a mut Store<'a>) -> Self {
            Server { 
                server: None,
                store,
            }
        }

        // TODO: Add a callback parameter here?
        pub fn start(&mut self) -> anyhow::Result<()> {
            let server_configuration = esp_idf_svc::http::server::Configuration {
                stack_size: STACK_SIZE,
                ..Default::default()
            };

            self.server = Some(EspHttpServer::new(&server_configuration).unwrap());

            self.server
                .as_mut()
                .unwrap()
                .fn_handler("/", Method::Get, |req| {
                    req.into_ok_response()?
                        .write_all(INDEX_HTML.as_bytes())
                        .map(|_| ())
                })?;

            self.server
                .as_mut()
                .unwrap()
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

                    //info!("buf: {:?}", buf);

                    //let settings = serde_json::from_slice::<AppDelta>(&buf).unwrap();
                    if let Ok(form) = serde_json::from_slice::<AppSettings>(&buf) {
                        //self.store.modify(&form);
                        write!(resp, "New settings applied")?;
                        info!("Got new settings: {:?}", form);
                    } else {
                        resp.write_all("JSON error".as_bytes())?;
                    }

                    Ok(())
                })?;

            self.server
                .as_mut()
                .unwrap()
                .fn_handler("/state", Method::Get, |req| {
                    let state = AppSettings {
                        ap_ssid: Some("ResearchSmoko".to_string()),
                        ap_pass: None,
                        dexcom_user: Some("cvitat".to_string()),
                        dexcom_pass: None,
                        lamp_brightness: Some(64),
                    };

                    info!("Get request on /state!");

                    let state_ser = serde_json::to_string(&state).unwrap();

                    req.into_ok_response()?
                        .write_all(state_ser.as_bytes())
                        .map(|_| ())
                })?;

            Ok(())
        }
    }
}
