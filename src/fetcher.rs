mod helpers;

use std::env;
use std::thread;
use std::process;
use std::fs::{self, File};
use std::path::PathBuf;
use reqwest::Client;
use serde_json::json;
use chrono::{Duration, Utc, Local, DateTime};
use serde::{Serialize, Deserialize};
use serde_json::Value as JsonValue;
use failure::{Error, ResultExt, format_err};
use self::helpers::{load_toml, load_json, save_json};

const SETTINGS_PATH: &str = "Settings.toml";
const SESSION_PATH: &str = ".session.json";
// Nearly RFC 3339
const DATE_FORMAT: &str = "%FT%T%z";
const LISTENED_SIGNALS: &[libc::c_int] = &[
    signal_hook::SIGINT, signal_hook::SIGHUP,
    signal_hook::SIGQUIT, signal_hook::SIGTERM,
    signal_hook::SIGUSR1, signal_hook::SIGUSR2,
];

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Settings {
    base_url: String,
    username: String,
    password: String,
    client_id: String,
    client_secret: String,
    save_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Session {
    since: i64,
    access_token: Token,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Token {
    data: String,
    valid_until: DateTime<Utc>,
}

impl Default for Token {
    fn default() -> Self {
        Token {
            data: String::default(),
            valid_until: Utc::now(),
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Session {
            since: 0,
            access_token: Token::default(),
        }
    }
}

fn signal_receiver(signals: &[libc::c_int]) -> Result<crossbeam_channel::Receiver<libc::c_int>, Error> {
    let (s, r) = crossbeam_channel::bounded(4);
    let signals = signal_hook::iterator::Signals::new(signals)?;
    thread::spawn(move || {
        for signal in signals.forever() {
            if s.send(signal).is_err() {
                break;
            }
        }
    });
    Ok(r)
}

fn update_token(client: &Client, session: &mut Session, settings: &Settings) -> Result<(), Error> {
    let query = json!({
        "grant_type": "password",
        "client_id": &settings.client_id,
        "client_secret": &settings.client_secret,
        "username": &settings.username,
        "password": &settings.password,
    });

    let url = format!("{}/oauth/v2/token", &settings.base_url);

    let tokens: JsonValue = client.post(&url)
                                  .json(&query)
                                  .send()?
                                  .json()?;
    session.access_token = Token {
        data: tokens.get("access_token")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .ok_or_else(|| format_err!("Missing access token."))?,
        valid_until: tokens.get("expires_in")
                           .and_then(|v| v.as_i64())
                           .map(|d| Utc::now() + Duration::seconds(d))
                           .ok_or_else(|| format_err!("Missing expires in."))?,
    };

    Ok(())
}

fn run() -> Result<(), Error> {
    let mut args = env::args().skip(1);
    let category = args.next()
                       .ok_or_else(|| format_err!("Missing argument: category name."))?;
    let wifi = args.next()
                   .ok_or_else(|| format_err!("Missing argument: wifi status."))
                   .and_then(|v| v.parse::<bool>().map_err(Into::into))?;
    let online = args.next()
                     .ok_or_else(|| format_err!("Missing argument: online status."))
                     .and_then(|v| v.parse::<bool>().map_err(Into::into))?;
    let settings = load_toml::<Settings, _>(SETTINGS_PATH)
                             .context("Can't load settings.")?;
    let mut session = load_json::<Session, _>(SESSION_PATH)
                                .unwrap_or_default();
    let signals = signal_receiver(LISTENED_SIGNALS)?;

    if !online {
        let event = json!({
            "type": "setWifi",
            "enable": true,
        });
        println!("{}", event);
        signals.recv()?;
    }

    if !settings.save_path.exists() {
        fs::create_dir(&settings.save_path)?;
    }

    let client = Client::new();

    if session.access_token.valid_until <= Utc::now() {
        update_token(&client, &mut session, &settings)?;
    }

    let mut page = 1;
    let mut pages_count = 0;
    let mut downloads_count = 0;
    let since = session.since;
    let url = format!("{}/api/entries", &settings.base_url);

    'outer: loop {
        let query = json!({
            "since": since,
            "sort": "updated",
            "order": "asc",
            "archive": 0,
            "page": page,
            "perPage": 8,
        });

        let entries: JsonValue = client.get(&url)
                                       .header(reqwest::header::AUTHORIZATION,
                                               format!("Bearer {}", &session.access_token.data))
                                       .query(&query)
                                       .send()?
                                       .json()?;

        if entries.get("total").is_none() {
            break;
        } else {
            if page == 1 {
                let total = entries.get("total")
                                   .and_then(|v| v.as_u64())
                                   .unwrap();
                let message = if total == 0 {
                    "No new articles.".to_string()
                } else {
                    format!("Found {} new article{}.", total, if total != 1 { "s" } else { "" })
                };
                let event = json!({
                    "type": "notify",
                    "message": &message,
                });
                println!("{}", event);
                if total > 0 {
                    pages_count = entries.get("pages")
                                         .and_then(|v| v.as_u64())
                                         .unwrap();
                }
            }
        }

        if let Some(items) = entries.pointer("/_embedded/items").and_then(|v| v.as_array()) {
            for element in items {
                if let Ok(sig) = signals.try_recv() {
                    if sig != signal_hook::SIGUSR1 {
                        break 'outer;
                    }
                }

                let id = element.get("id")
                                .and_then(|v| v.as_u64())
                                .ok_or_else(|| format_err!("Missing id."))?;

                let title = element.get("title")
                                   .and_then(|v| v.as_str())
                                   .map(String::from)
                                   .unwrap_or_default();

                let published_by = element.get("published_by")
                                          .and_then(|v| v.as_array())
                                          .map(|v| v.iter().filter_map(|x| x.as_str())
                                                           .filter(|x| !x.is_empty())
                                                           .collect::<Vec<&str>>())
                                          .map(|v| v.join(", "))
                                          .filter(|v| !v.is_empty())
                                          .unwrap_or_default();
                let domain_name = element.get("domain_name")
                                         .and_then(|v| v.as_str())
                                         .map(String::from)
                                         .unwrap_or_default();

                let author = match (!published_by.is_empty(), !domain_name.is_empty()) {
                    (true, true) => format!("{} ({})", published_by, domain_name),
                    (true, false) => published_by,
                    _ => domain_name,
                };

                let year = element.get("published_at")
                                  .filter(|v| v.is_string())
                                  .or_else(|| element.get("created_at"))
                                  .and_then(|v| v.as_str())
                                  .and_then(|v| DateTime::parse_from_str(v, DATE_FORMAT).ok())
                                  .map(|v| v.format("%Y").to_string())
                                  .unwrap_or_default();

                let updated_at = element.get("updated_at")
                                        .and_then(|v| v.as_str())
                                        .and_then(|v| DateTime::parse_from_str(v, DATE_FORMAT).ok())
                                        .ok_or_else(|| format_err!("Missing updated at."))?;

                session.since = updated_at.timestamp();

                let epub_path = settings.save_path.join(&format!("{}.epub", id));
                if epub_path.exists() {
                    continue;
                }

                let mut file = File::create(&epub_path)?;
                let url = format!("{}/api/entries/{}/export.epub", settings.base_url, id);

                let response = client.get(&url)
                                     .header(reqwest::header::AUTHORIZATION,
                                             format!("Bearer {}", &session.access_token.data))
                                     .send()
                                     .and_then(|mut body| body.copy_to(&mut file));

                if let Err(err) = response {
                    eprintln!("{}", err);
                    fs::remove_file(epub_path).ok();
                    continue;
                }

                downloads_count += 1;

                let file_info = json!({
                    "path": epub_path.to_str().unwrap_or(""),
                    "kind": "epub",
                    "size": file.metadata().ok()
                                .map_or(0, |m| m.len()),
                });

                let info = json!({
                    "title": title,
                    "author": author,
                    "year": year,
                    "added": updated_at.with_timezone(&Local)
                                       .format("%Y-%m-%d %H:%M:%S")
                                       .to_string(),
                    "categories": [category],
                    "file": file_info,
                });

                let event = json!({
                    "type": "addDocument",
                    "info": &info,
                });

                println!("{}", event);
            }
        }

        page += 1;
        if page > pages_count {
            break;
        }
    }

    if pages_count > 0 {
        let message = if downloads_count > 0 {
            format!("Downloaded {} article{}.", downloads_count, if downloads_count != 1 { "s" } else { "" })
        } else {
            "No articles downloaded.".to_string()
        };
        let event = json!({
            "type": "notify",
            "message": &message,
        });
        println!("{}", event);
    }

    let event = json!({
        "type": "setWifi",
        "enable": wifi,
    });
    println!("{}", event);

    save_json(&session, SESSION_PATH).context("Can't save session.")?;
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        for e in e.iter_chain() {
            eprintln!("article_fetcher: {}", e);
        }
        process::exit(1);
    }
}
