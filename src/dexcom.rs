pub mod dexcom {
    use embedded_svc::{http::client::Client, io::Write, utils::io};
    use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};
    use log::{error, info};
    use serde::{Deserialize, Serialize};
    use serde_json;

    pub const APPLICATION_ID: &'static str = "d89443d2-327c-4a6f-89e5-496bbb0317db";

    pub const BASE_URL: &'static str = "https://share1.dexcom.com/ShareWebServices/Services";

    pub const LOGIN_ID_ENDPOINT: &'static str = "General/LoginPublisherAccountById";
    pub const AUTHENTICATE_ENDPOINT: &'static str = "General/AuthenticatePublisherAccount";
    pub const GLUCOSE_READINGS_ENDPOINT: &'static str =
        "Publisher/ReadPublisherLatestGlucoseValues";

    pub const MAX_MAX_COUNT: isize = 288;

    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct DexcomLogin {
        account_name: String,
        password: String,
        application_id: String,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct DexcomSession {
        account_id: String,
        password: String,
        application_id: String,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct DexcomGlucose {
        session_id: String,
        minutes: isize,
        max_count: isize,
    }

    #[derive(Deserialize, Serialize, Debug)]
    #[serde(rename_all = "lowercase")]
    struct DexcomGlucoseReading {
        pub wt: String,
        pub st: String,
        pub dt: String,
        pub value: isize,
        pub trend: String,
    }

    #[derive(Debug, Copy, Clone)]
    pub enum GlucoseTrend {
        NoTrend,
        DoubleUp,
        SingleUp,
        FortyFiveUp,
        Flat,
        FortyFiveDown,
        SingleDown,
        DoubleDown,
        NotComputable,
        RateOutOfRange,
    }

    impl GlucoseTrend {
        pub fn from_str(trend: &str) -> Self {
            match trend {
                "DoubleUp" => Self::DoubleUp,
                "SingleUp" => Self::SingleUp,
                "FortyFiveUp" => Self::FortyFiveUp,
                "Flat" => Self::Flat,
                "FortyFiveDown" => Self::FortyFiveDown,
                "SingleDown" => Self::SingleDown,
                "DoubleDown" => Self::DoubleDown,
                "NotComputable" => Self::NotComputable,
                "RateOutOfRange" => Self::RateOutOfRange,
                _ => Self::NoTrend,
            }
        }
    }

    #[derive(Debug, Copy, Clone)]
    pub struct GlucoseReading {
        pub time: i64,
        pub value: isize,
        pub trend: GlucoseTrend,
    }

    impl GlucoseReading {
        pub fn new() -> Self {
            Self {
                time: 0,
                value: 0,
                trend: GlucoseTrend::NoTrend,
            }
        }
    }

    pub struct Dexcom {
        client: Client<EspHttpConnection>,
    }

    impl Dexcom {
        pub fn new() -> Self {
            let connection = EspHttpConnection::new(&HttpConfiguration {
                use_global_ca_store: true,
                crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
                ..Default::default()
            })
            .unwrap();

            let client = Client::wrap(connection);

            Dexcom { client }
        }

        pub fn get_user_id(&mut self, acct_name: &str, pass: &str) -> anyhow::Result<String> {
            let login_ctx = DexcomLogin {
                account_name: acct_name.to_string(),
                password: pass.to_string(),
                application_id: APPLICATION_ID.into(),
            };

            let auth_url = format!("{}/{}", BASE_URL, AUTHENTICATE_ENDPOINT);

            let user_id_json = Dexcom::post(
                &mut self.client,
                &auth_url,
                &(serde_json::to_string(&login_ctx).unwrap()),
            )
            .unwrap();

            Ok(serde_json::from_str(&user_id_json).unwrap())
        }

        pub fn get_session(&mut self, user_id: &str, pass: &str) -> anyhow::Result<String> {
            let session_ctx = DexcomSession {
                account_id: user_id.to_string(),
                password: pass.to_string(),
                application_id: APPLICATION_ID.into(),
            };

            let login_url = format!("{}/{}", BASE_URL, LOGIN_ID_ENDPOINT);

            let session_json = Dexcom::post(
                &mut self.client,
                &login_url,
                &(serde_json::to_string(&session_ctx).unwrap()),
            )
            .unwrap();

            Ok(serde_json::from_str(&session_json).unwrap())
        }

        pub fn get_latest_glucose(&mut self, session: &str) -> anyhow::Result<GlucoseReading> {
            match self.get_glucose(session, 5, 1) {
                Ok(vec) => {
                    if vec.len() > 0 {
                        return Ok(vec[0]);
                    } else {
                        return Err(anyhow::anyhow!("No measurement"));
                    }
                }
                Err(error) => Err(error),
            }
        }

        pub fn get_glucose(
            &mut self,
            session: &str,
            minutes: isize,
            max_count: isize,
        ) -> anyhow::Result<Vec<GlucoseReading>> {
            let glucose_ctx = DexcomGlucose {
                session_id: session.to_string(),
                minutes,
                max_count,
            };

            let glucose_url = format!("{}/{}", BASE_URL, GLUCOSE_READINGS_ENDPOINT);

            let mut glucose_json = Dexcom::post(
                &mut self.client,
                &glucose_url,
                &(serde_json::to_string(&glucose_ctx).unwrap()),
            )
            .unwrap();

            // Fix the json field names
            glucose_json = glucose_json.replace("WT", "wt");
            glucose_json = glucose_json.replace("ST", "st");
            glucose_json = glucose_json.replace("DT", "dt");
            glucose_json = glucose_json.replace("Value", "value");
            glucose_json = glucose_json.replace("Trend", "trend");

            let glucose_readings: Vec<DexcomGlucoseReading> =
                serde_json::from_str(&glucose_json).unwrap();

            Ok(glucose_readings
                .into_iter()
                .map(|reading| {
                    let start_bytes = reading.wt.find("(").unwrap_or(0) + 1;
                    let end_bytes = reading.wt.find(")").unwrap_or(reading.wt.len());
                    let time: i64 = reading.wt[start_bytes..end_bytes]
                        .to_string()
                        .parse()
                        .unwrap(); //parse::<isize>().unwrap();
                    GlucoseReading {
                        time, // TODO: wt, st, or dt?
                        value: reading.value,
                        trend: GlucoseTrend::from_str(&reading.trend),
                    }
                })
                .collect())
        }

        fn post(
            client: &mut Client<EspHttpConnection>,
            url: &str,
            payload: &str,
        ) -> anyhow::Result<String> {
            let content_length_header = format!("{}", payload.len());
            let headers = [
                ("accept-encoding", "application/json"),
                ("content-type", "application/json"),
                ("content-length", &*content_length_header),
            ];

            let mut request = client.post(url, &headers)?;
            request.write_all(payload.as_bytes())?;
            request.flush()?;
            info!("-> POST {}", url);
            let mut response = request.submit()?;

            let status = response.status();
            info!("<- {}", status);
            let mut buf = [0u8; 4096];
            let bytes_read = io::try_read_full(&mut response, &mut buf).map_err(|e| e.0)?;
            info!("Read {} bytes", bytes_read);
            match std::str::from_utf8(&buf[0..bytes_read]) {
                Ok(body_string) => {
                    info!(
                        "Response body (truncated to {} bytes): {:?}",
                        buf.len(),
                        body_string
                    );
                    return Ok(body_string.to_owned());
                }
                Err(e) => error!("Error decoding response body: {}", e),
            };

            Ok("".to_owned())
        }
    }
}
