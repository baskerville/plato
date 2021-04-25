mod helpers;

use std::io;
use std::env;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use chrono::{Duration, Utc, Local, DateTime};
use serde::{Serialize, Deserialize};
use serde_json::{json, Value as JsonValue};
use reqwest::blocking::Client;
use anyhow::{Error, Context, format_err};
use self::helpers::{load_toml, load_json, save_json, decode_entities};

const SETTINGS_PATH: &str = "Settings.toml";
const SESSION_PATH: &str = ".session.json";
const BASE_URL: &str = "https://xkcd.com/";

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Settings {
    num_comics_to_download: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Session {
    downloads_count: usize,
    last_opened: String,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            downloads_count: 0,
            last_opened: "0000-00-00 00:00:00".to_string(),
        }
    }
}

fn main() -> Result<(), Error> {
    let mut args = env::args().skip(1);
    let library_path = PathBuf::from(args.next()
        .ok_or_else(|| format_err!("missing argument: library path"))?);
    let save_path = PathBuf::from(args.next()
        .ok_or_else(|| format_err!("missing argument: save path"))?);
    let wifi = args.next()
        .ok_or_else(|| format_err!("missing argument: wifi status"))
        .and_then(|v| v.parse::<bool>().map_err(Into::into))?;
    let online = args.next()
        .ok_or_else(|| format_err!("missing argument: online status"))
        .and_then(|v| v.parse::<bool>().map_err(Into::into))?;
    let settings = load_toml::<Settings, _>(SETTINGS_PATH)
        .with_context(|| format!("can't load settings from {}", SETTINGS_PATH))?;
    let mut session = load_json::<Session, _>(SESSION_PATH)
        .unwrap_or_default();

    if !online {
        if !wifi {
            let event = json!({
                "type": "notify",
                "message": "Establishing a network connection.",
            });
            println!("{}", event);
            let event = json!({
                "type": "setWifi",
                "enable": true,
            });
            println!("{}", event);
        } else {
            let event = json!({
                "type": "notify",
                "message": "Waiting for the network to come up.",
            });
            println!("{}", event);
        }
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
    }

    if !save_path.exists() {
        fs::create_dir(&save_path)?;
    }

    let client = Client::new();

    let sigterm = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&sigterm))?;

    let mut current_num = 1;
    let last_downloads_count = session.downloads_count;

    {
        let url = format!("{}/info.0.json", &BASE_URL);

        let entries: JsonValue = client.get(&url)
            .send()?
            .json()?;

        if entries.get("num").is_none() {
            let message = "Something went wrong getting today's comic.";
            let event = json!({
                    "type": "notify",
                    "message": &message,
                });
            println!("{}", event);
        } else {
            let num = entries.get("num")
                .and_then(|v| v.as_u64())
                .unwrap();
            let message = format!("Today's comic: {}.", num);
            let event = json!({
                    "type": "notify",
                    "message": &message,
                });
            println!("{}", event);

            current_num = num;
        }
    }

    for num in current_num - settings.num_comics_to_download as u64..current_num {
        let url = format!("{}/{}/info.0.json", &BASE_URL, &num);

        let data: JsonValue = client.get(&url)
            .send()?
            .json()?;

        if data.get("num").is_none() {
            continue;
        }

        let num = data.get("num")
            .and_then(|v| v.as_u64())
            .unwrap();

        let title = data.get("title")
            .and_then(JsonValue::as_str)
            .map(decode_entities)
            .map(String::from)
            .unwrap_or_default();

        let safe_title = data.get("safe_title")
            .and_then(JsonValue::as_str)
            .map(decode_entities)
            .map(String::from)
            .unwrap_or_default();

        let comic_url = data.get("img")
            .and_then(JsonValue::as_str)
            .map(decode_entities)
            .map(String::from)
            .unwrap_or_default();

        let comic_path = save_path.join(&format!("{}.png", safe_title));
        if comic_path.exists() {
            continue;
        }

        let mut file = File::create(&comic_path)?;

        let response = client.get(&comic_url)
            .send()
            .and_then(|mut body| body.copy_to(&mut file));

        if let Err(err) = response {
            eprintln!("Can't download {}: {:#}.", &num, err);
            fs::remove_file(comic_path).ok();
            continue;
        }

        session.downloads_count = session.downloads_count.wrapping_add(1);

        if let Ok(path) = comic_path.strip_prefix(&library_path) {
            let file_info = json!({
                        "path": path,
                        "kind": "png",
                        "size": file.metadata().ok()
                                    .map_or(0, |m| m.len()),
                    });

            let info = json!({
                        "title": title,
                        "author": "",
                        "identifier": num.to_string(),
                        "file": file_info,
                    });

            let event = json!({
                        "type": "addDocument",
                        "info": &info,
                    });

            println!("{}", event);
        }
    }

    if !wifi {
        let event = json!({
            "type": "setWifi",
            "enable": false,
        });
        println!("{}", event);
    }

    save_json(&session, SESSION_PATH).context("can't save session")?;
    Ok(())
}
