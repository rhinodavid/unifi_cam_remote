#[macro_use]
extern crate serde_derive;
extern crate serde_json;

extern crate reqwest;

use std::io::Read;
use std::env;

#[derive(Serialize, Deserialize, Debug)]
struct CameraResponse {
    data: Vec<UnifiCameraData>,
    meta: CameraResponseMeta,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CameraResponseMeta {
    total_count: u32,
    filtered_count: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct UnifiCameraData {
    #[serde(rename = "_id")]
    id: String,
    name: String,
    managed: bool,
    uuid: String,
    analytics_settings: AnalyticsSettings,
    recording_settings: RecordingSettings,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AnalyticsSettings {
    enable_sound_alert: bool,
    sound_alert_volume: u32,
    minimum_motion_secs: u32,
    end_motion_after_secs: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct RecordingSettings {
    #[serde(default = "default_channel")]
    channel: Option<String>,
    pre_padding_secs: u32,
    post_padding_secs: u32,
    full_time_record_enabled: bool,
    motion_record_enabled: bool,
}

fn default_channel() -> Option<String> {
    Some(String::from("0"))
}

struct Config {
    api_key: String,
    host: String,
}

impl Config {
    fn new() -> Result<Config, &'static str> {
        let api_key = match env::var_os("UNIFI_API_KEY") {
            Some(val) => match val.into_string() {
                Ok(val) => val,
                Err(_) => return Err("Could not convert api key env variable to string"),
            },
            None => return Err("Could not get api key; try setting UNIFI_API_KEY env variable"),
        };

        let host = match env::var_os("UNIFI_VIDEO_HOST") {
            Some(val) => match val.into_string() {
                Ok(val) => val,
                Err(_) => return Err("Could not convert unifi video host env variable to string"),
            },
            None => return Err("Could not get api key; try setting UNIFI_API_KEY env variable"),
        };

        Ok(Config { api_key, host })
    }
}

fn update_record_setting(
    config: &Config,
    camera_id: &str,
    status: bool,
) -> Result<(), &'static str> {
    // get current settings from the camera
    let mut response = reqwest::get(&format!(
        "http://{host}:7080/api/2.0/camera/{id}?apiKey={key}",
        host = config.host,
        key = config.api_key,
        id = camera_id
    )).expect("Failed to send request");
    let mut buf = String::new();
    response.read_to_string(&mut buf).unwrap();
    let mut response_data: CameraResponse = serde_json::from_str(&buf)
        .expect("Failed to parse response to turn recording on initial request");
    assert_eq!(1, response_data.data.len());
    let camera_data = &mut response_data.data[0];

    // if recording is already on, go ahead and exit
    if camera_data.recording_settings.full_time_record_enabled == status {
        return Ok(());
    }

    // update settings to turn recording on
    camera_data.recording_settings.full_time_record_enabled = status;
    camera_data.recording_settings.motion_record_enabled = status;

    // send back the modded settings
    let client = reqwest::Client::new();
    let _ = match client
        .put(&format!(
            "http://{host}:7080/api/2.0/camera/{id}?apiKey={key}",
            host = config.host,
            key = config.api_key,
            id = camera_id
        ))
        .json(camera_data)
        .send()
    {
        Ok(val) => val,
        Err(_) => return Err("There was an error setting the camera to record"),
    };
    Ok(())
}

fn turn_recording_on(config: &Config, camera_id: &str) -> Result<(), &'static str> {
    update_record_setting(config, camera_id, true)
}

fn turn_recording_off(config: &Config, camera_id: &str) -> Result<(), &'static str> {
    update_record_setting(config, camera_id, false)
}

fn get_camera_list(config: &Config) -> Result<Vec<UnifiCameraData>, &'static str> {
    let mut response = reqwest::get(&format!(
        "http://{}:7080/api/2.0/camera?apiKey={}",
        config.host, config.api_key
    )).expect("Failed to send request");
    let mut buf = String::new();
    response
        .read_to_string(&mut buf)
        .expect("Failed to read response");

    let response_data: CameraResponse =
        serde_json::from_str(&buf).expect("Failed to parse response to initial request");
    let cameras: Vec<UnifiCameraData> = response_data
        .data
        .into_iter()
        .filter(move |c| c.managed)
        .collect();
    Ok(cameras)
}

fn main() {
    let config = Config::new().unwrap();
    // hit camera endpoint to get id of first camera found

    let cameras = get_camera_list(&config).unwrap();
    if cameras.len() < 1 {
        panic!("No cameras were found");
    }
    let camera_id = cameras[0].id.clone();

    turn_recording_off(&config, &camera_id).unwrap();
}
