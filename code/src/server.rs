pub mod server {
    use embedded_svc::{
        http::{Headers, Method},
        io::{Read, Write},
    };
    use esp_idf_svc::http::server::EspHttpServer;
    use log::info;
    use serde::{Deserialize, Serialize};
    use std::sync::mpsc;
    use std::sync::mpsc::Sender;
    //use std::sync::{Arc, Mutex};

    static INDEX_HTML: &str = include_str!("index.html");

    const STACK_SIZE: usize = 10240;

    // Need lots of stack to parse JSON
    // Max payload length
    const MAX_LEN: usize = 1024;

    const API_VER: &str = "v1";
    const API_STATE: &str = "state";
    const API_SET: &str = "set";
    const API_RESET: &str = "reset";

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct ServerUpdate {
        pub brightness: Option<u8>,
        pub on: Option<bool>,
        pub ap_ssid: Option<String>,
        pub ap_psk: Option<String>,
        pub dexcom_user: Option<String>,
        pub dexcom_pass: Option<String>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct ServerData {
        pub brightness: Option<u8>,
        pub on: Option<bool>,
        pub ap_ssid_stored: Option<bool>,
        pub ap_psk_stored: Option<bool>,
        pub dexcom_user_stored: Option<bool>,
        pub dexcom_pass_stored: Option<bool>,
        pub bat_attached: Option<bool>,
        pub bat_charging: Option<bool>,
        pub bat_capacity: Option<f32>,
        pub uptime: Option<u64>,
        pub temp: Option<f32>,
    }

    impl ServerData {
        pub fn new() -> Self {
            Self {
                brightness: None,
                on: None,
                //cred_store: None,
                ap_ssid_stored: None,
                ap_psk_stored: None,
                dexcom_user_stored: None,
                dexcom_pass_stored: None,
                bat_attached: None,
                bat_charging: None,
                bat_capacity: None,
                uptime: None,
                temp: None,
            }
        }

        pub fn merge(&mut self, other: &ServerData) {
            self.brightness = self.brightness.or(other.brightness);
            self.on = self.on.or(other.on);
            self.ap_ssid_stored = self.ap_ssid_stored.or(other.ap_ssid_stored);
            self.ap_psk_stored = self.ap_psk_stored.or(other.ap_psk_stored);
            self.dexcom_user_stored = self.dexcom_user_stored.or(other.dexcom_user_stored);
            self.dexcom_pass_stored = self.dexcom_pass_stored.or(other.dexcom_pass_stored);
            self.bat_attached = self.bat_attached.or(other.bat_attached);
            self.bat_charging = self.bat_charging.or(other.bat_charging);
            self.bat_capacity = self.bat_capacity.or(other.bat_capacity);
            self.uptime = self.uptime.or(other.uptime);
            self.temp = self.temp.or(other.temp);
        }
    }

    #[derive(Debug)]
    pub enum ServableDataReq {
        Set(ServerUpdate),
        Get(mpsc::Sender<ServableDataRsp>),
        Reset,
    }

    pub enum ServableDataRsp {
        Data(ServerData),
        Error,
    }

    pub struct Server<'a> {
        server: Option<EspHttpServer<'a>>,
        data_channels: Vec<Sender<ServableDataReq>>,
    }

    impl<'a> Server<'a> {
        pub fn new() -> Self {
            Server {
                server: None,
                data_channels: Vec::new(),
            }
        }

        pub fn add_data_channel(&mut self, obj: &mut impl ServableData) {
            self.data_channels.push(obj.get_channel())
        }

        // Start server listeners
        pub fn start(&mut self) -> anyhow::Result<()> {
            let server_configuration = esp_idf_svc::http::server::Configuration {
                stack_size: STACK_SIZE,
                ..Default::default()
            };

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

            //Listener: Handle new settings from the web app
            {
                let data_channels = self.data_channels.clone();
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

                            let msg = serde_json::from_slice::<ServerUpdate>(&buf);
                            match msg {
                                Ok(form) => {
                                    Server::send_server_update(&data_channels, &form);
                                    write!(resp, "New settings applied")?;
                                }
                                Err(e) => {
                                    info!("Error parsing SET data: {}", e);
                                    resp.write_all("JSON error".as_bytes())?;
                                }
                            }

                            Ok(())
                        },
                    )?;
            }

            // Listener: Serve the device's status when on request
            {
                let data_channels = self.data_channels.clone();
                self.server.as_mut().unwrap().fn_handler(
                    &format!("/api/{}/{}", API_VER, API_STATE),
                    Method::Get,
                    move |req| {
                        info!("Get request on /state!");

                        let app_state = Server::get_server_data(&data_channels);
                        info!("assembled state: {:?}", app_state);

                        // Serialize, send back to web app
                        let state_ser = serde_json::to_string(&app_state).unwrap();
                        req.into_ok_response()?
                            .write_all(state_ser.as_bytes())
                            .map(|_| ())
                    },
                )?;
            }

            // Listener: Handle new settings from the web app
            {
                let data_channels = self.data_channels.clone();
                self.server
                    .as_mut()
                    .unwrap()
                    .fn_handler::<anyhow::Error, _>(
                        &format!("/api/{}/{}", API_VER, API_RESET),
                        Method::Post,
                        move |req| {
                            let mut resp = req.into_ok_response()?;

                            Server::send_reset_signal(&data_channels);
                            info!("Resetting");
                            write!(resp, "All settings reset")?;

                            Ok(())
                        },
                    )?;
            }

            Ok(())
        }

        pub fn send_server_update(channels: &Vec<Sender<ServableDataReq>>, update: &ServerUpdate) {
            for channel in channels.iter() {
                channel
                    .send(ServableDataReq::Set((*update).to_owned()))
                    .unwrap();
            }
        }

        pub fn get_server_data(channels: &Vec<Sender<ServableDataReq>>) -> ServerData {
            let mut num_tx = 0;
            let (tx, rx) = mpsc::channel::<ServableDataRsp>();
            for channel in channels.iter() {
                channel.send(ServableDataReq::Get(tx.clone())).unwrap();
                num_tx += 1;
            }

            let mut server_data = ServerData::new();

            let mut num_rx = 0;
            while num_rx < num_tx {
                if let Ok(rsp) = rx.recv() {
                    if let ServableDataRsp::Data(serve_data) = rsp {
                        server_data.merge(&serve_data);
                    }
                    // Whether the data is present or not, we got a repsonse, so increment our
                    // count
                    num_rx += 1;
                }
                // TODO: Some kind of timeout or check for no response
            }

            server_data
        }

        pub fn send_reset_signal(channels: &Vec<Sender<ServableDataReq>>) {
            for channel in channels.iter() {
                channel.send(ServableDataReq::Reset).unwrap();
            }
        }

        pub fn stop(&mut self) {
            self.server = None
        }
    }

    pub trait ServableData {
        fn get_channel(&mut self) -> Sender<ServableDataReq>;
        fn handle_server_req(&mut self);
    }
}
