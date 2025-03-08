pub mod server {
    use embedded_svc::{
        http::{Headers, Method},
        io::{Read, Write},
    };
    use esp_idf_svc::http::server::EspHttpServer;
    use log::info;
    use serde::{Deserialize, Serialize};
    use std::sync::mpsc::Sender;
    use std::sync::{Arc, Mutex};

    static INDEX_HTML: &str = include_str!("index.html");

    const STACK_SIZE: usize = 10240;

    // Need lots of stack to parse JSON
    // Max payload length
    const MAX_LEN: usize = 1024;

    const API_VER: &str = "v1";
    const API_STATE: &str = "state";
    const API_SET: &str = "set";
    const API_RESET: &str = "reset";

    #[derive(Debug, Deserialize, Serialize)]
    struct StateRsp {
        brightness: u8,
        on: bool,
        cred_store: String,
        ap_ssid_stored: bool,
        ap_psk_stored: bool,
        dexcom_user_stored: bool,
        dexcom_pass_stored: bool,
        bat_attached: bool,
        bat_charging: bool,
        bat_capacity: f32,
        uptime: u64,
        temp: i16,
    }

    impl StateRsp {
        pub fn new() -> Self {
            Self {
                brightness: 0,
                on: false,
                cred_store: "".to_string(),
                ap_ssid_stored: false,
                ap_psk_stored: false,
                dexcom_user_stored: false,
                dexcom_pass_stored: false,
                bat_attached: false,
                bat_charging: false,
                bat_capacity: 0f32,
                uptime: 0,
                temp: 0,
            }
        }
    }

    pub struct Server<'a> {
        server: Option<EspHttpServer<'a>>,
    }

    impl<'a> Server<'a> {
        pub fn new() -> Self {
            Server { server: None }
        }

        // Start server listeners
        pub fn start(&mut self) -> anyhow::Result<()> {
            let server_configuration = esp_idf_svc::http::server::Configuration {
                stack_size: STACK_SIZE,
                ..Default::default()
            };

            // Clone tx_channel to give it to the server handler
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
            {
                let tx = self.tx_channel.clone();
                self.server
                    .as_mut()
                    .unwrap()
                    .fn_handler::<anyhow::Error, _>(
                        &format!("/api/{}/{}", API_VER, API_SET),
                        Method::Post,
                        move |mut req| {
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
                                tx.send(SettingsAction::Set(form)).unwrap();
                                write!(resp, "New settings applied")?;
                            } else {
                                resp.write_all("JSON error".as_bytes())?;
                            }

                            Ok(())
                        },
                    )?;
            }

            // Listener: Serve the device's status when on request
            self.server.as_mut().unwrap().fn_handler(
                &format!("/api/{}/{}", API_VER, API_STATE),
                Method::Get,
                move |req| {
                    info!("Get request on /state!");

                    let mut state_rsp = StateRsp::new();

                    // Acquire lock on sapp state
                    let state_guard = settings.lock().unwrap();
                    let state = (*state_guard).clone();
                    std::mem::drop(state_guard);

                    state_rsp.brightness = state.brightness.unwrap();
                    state_rsp.on = true;
                    state_rsp.ap_ssid_stored = state.ap_ssid.is_some();
                    state_rsp.ap_psk_stored = state.ap_psk.is_some();
                    state_rsp.dexcom_user_stored = state.dexcom_user.is_some();
                    state_rsp.dexcom_pass_stored = state.dexcom_pass.is_some();

                    info!("{:?}", state_rsp);

                    // Serialize, send back to web app
                    let state_ser = serde_json::to_string(&state_rsp).unwrap();
                    req.into_ok_response()?
                        .write_all(state_ser.as_bytes())
                        .map(|_| ())
                },
            )?;

            // Listener: Handle new settings from the web app
            {
                let tx = self.tx_channel.clone();
                self.server
                    .as_mut()
                    .unwrap()
                    .fn_handler::<anyhow::Error, _>(
                        &format!("/api/{}/{}", API_VER, API_RESET),
                        Method::Post,
                        move |req| {
                            let mut resp = req.into_ok_response()?;

                            tx.send(SettingsAction::Reset).unwrap();
                            info!("Resetting");
                            write!(resp, "All settings reset")?;

                            Ok(())
                        },
                    )?;
            }

            Ok(())
        }

        pub fn stop(&mut self) {
            self.server = None
        }
    }

    #[derive(Debug)]
    pub enum ServableDataReq<T> {
        Set(T),
        Get,
        Reset,
    }

    pub enum ServableDataRsp<T> {
        Data(T),
        Done,
        Error,
    }

    pub trait ServableData<T> {
        fn set_channel(&mut self) -> Sender<ServableDataReq<T>>;
    }
}
