use std::io;
use std::env;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use plato_core::chrono::{Duration, Utc, Local, DateTime};
use plato_core::serde::{Serialize, Deserialize};
use plato_core::serde_json::{self, json, Value as JsonValue};
use reqwest::blocking::Client;
use plato_core::anyhow::{Error, Context, format_err};
use plato_core::helpers::{load_toml, load_json, save_json, decode_entities};

const SETTINGS_PATH: &str = "Settings.toml";
const SESSION_PATH: &str = ".session.json";
const URLS_PATH: &str = "urls.txt";
// Nearly RFC 3339
const DATE_FORMAT: &str = "%FT%T%z";

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "plato_core::serde")]
#[serde(default, rename_all = "kebab-case")]
struct Settings {
    base_url: String,
    username: String,
    password: String,
    client_id: String,
    client_secret: String,
    sync_finished: bool,
    remove_finished: bool,
    balance_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "plato_core::serde")]
#[serde(default, rename_all = "camelCase")]
struct Session {
    since: i64,
    access_token: Token,
    downloads_count: usize,
    removals_count: usize,
    last_opened: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "plato_core::serde")]
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
            downloads_count: 0,
            removals_count: 0,
            last_opened: "0000-00-00 00:00:00".to_string(),
        }
    }
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

    let response = client.post(&url)
                         .json(&query)
                         .send()?;
    let status = response.status();
    let body: JsonValue = response.json()?;

    if status.is_success() {
        session.access_token = Token {
            data: body.get("access_token")
                      .and_then(|v| v.as_str())
                      .map(String::from)
                      .ok_or_else(|| format_err!("missing access token"))?,
            valid_until: body.get("expires_in")
                             .and_then(|v| v.as_i64())
                             .map(|d| Utc::now() + Duration::seconds(d))
                             .ok_or_else(|| format_err!("missing expires in"))?,
        };
        Ok(())
    } else {
        let err_desc = body.get("error_description")
                           .and_then(JsonValue::as_str)
                           .or_else(|| status.canonical_reason())
                           .unwrap_or_else(|| status.as_str());
        Err(format_err!("failed to authentificate: {}", err_desc))
    }
}

// The *detail* parameter is only available in 2.4.0 and up.
fn is_detail_available(client: &Client, settings: &Settings) -> bool {
    // /api/info is only available in 2.4.0 and up.
    let url = format!("{}/api/info", settings.base_url);
    client.get(&url).send()
          .map_or(false, |response| response.status().is_success())
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

    if session.access_token.valid_until <= Utc::now() {
        update_token(&client, &mut session, &settings)?;
    }

    let sigterm = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&sigterm))?;

    if let Ok(contents) = fs::read_to_string(URLS_PATH) {
        for line in contents.lines() {
            let query = json!({"url": line});
            let url = format!("{}/api/entries", &settings.base_url);
            let response = client.post(&url)
                                 .header(reqwest::header::AUTHORIZATION,
                                         format!("Bearer {}", &session.access_token.data))
                                 .json(&query)
                                 .send();
            let response = response.unwrap();
            if !response.status().is_success() {
                let status = response.status();
                let body: JsonValue = response.json()?;
                let err_desc = body.get("error_description")
                                   .and_then(JsonValue::as_str)
                                   .or_else(|| status.canonical_reason())
                                   .unwrap_or_else(|| status.as_str());
                eprintln!("Can't add {}: {}.", line, err_desc);
            }
        }
    }

    fs::remove_file(URLS_PATH).ok();

    if settings.sync_finished || settings.remove_finished {
        let event = json!({
            "type": "search",
            "path": save_path,
            "query": format!("'F 'O {}", session.last_opened),
            "sortBy": ("opened", false),
        });
        println!("{}", event);

        let mut line = String::new();
        io::stdin().read_line(&mut line)?;

        let last_removals_count = session.removals_count;
        let mut archivals_count = 0;

        if let Ok(event) = serde_json::from_str::<JsonValue>(&line) {
            if let Some(results) = event.get("results").and_then(JsonValue::as_array) {
                let message = if results.is_empty() {
                    "No finished articles.".to_string()
                } else {
                    format!("Found {} finished article{}.", results.len(), if results.len() != 1 { "s" } else { "" })
                };
                let event = json!({
                    "type": "notify",
                    "message": &message,
                });
                println!("{}", event);

                for entry in results {
                    if sigterm.load(Ordering::Relaxed) {
                        break;
                    }

                    if settings.sync_finished {
                        if let Some(id) = entry.get("identifier")
                                               .and_then(JsonValue::as_str)
                                               .and_then(|v| v.parse::<u64>().ok()) {
                            let url = format!("{}/api/entries/{}", &settings.base_url, id);
                            let query = json!({"archive": 1});
                            let response = client.patch(&url)
                                                 .header(reqwest::header::AUTHORIZATION,
                                                         format!("Bearer {}", &session.access_token.data))
                                                 .json(&query)
                                                 .send();
                            let response = response.unwrap();
                            if response.status().is_success() {
                                archivals_count += 1;
                            } else {
                                let status = response.status();
                                let body: JsonValue = response.json()?;
                                let err_desc = body.get("error_description")
                                                   .and_then(JsonValue::as_str)
                                                   .or_else(|| status.canonical_reason())
                                                   .unwrap_or_else(|| status.as_str());
                                eprintln!("Can't mark {} as read: {}.", id, err_desc);
                            }
                        }
                    }

                    if settings.remove_finished {
                        if let Some(path) = entry.pointer("/file/path")
                                                 .and_then(JsonValue::as_str) {
                            let event = json!({
                                "type": "removeDocument",
                                "path": path,
                            });
                            println!("{}", event);
                            session.removals_count = session.removals_count.wrapping_add(1)
                        }
                    }

                    if let Some(opened) = entry.pointer("/reader/opened")
                                               .and_then(JsonValue::as_str) {
                        session.last_opened = opened.to_string();
                    }
                }

                if !results.is_empty() {
                    if settings.sync_finished {
                        let message = if archivals_count > 0 {
                            format!("Marked {} finished article{} as read.", archivals_count, if archivals_count != 1 { "s" } else { "" })
                        } else {
                            "No finished articles marked as read.".to_string()
                        };
                        let event = json!({
                            "type": "notify",
                            "message": &message,
                        });
                        println!("{}", event);
                    }

                    if settings.remove_finished {
                        let removals_count = session.removals_count.saturating_sub(last_removals_count);
                        let message = if removals_count > 0 {
                            format!("Removed {} finished article{}.", removals_count, if removals_count != 1 { "s" } else { "" })
                        } else {
                            "No finished articles removed.".to_string()
                        };
                        let event = json!({
                            "type": "notify",
                            "message": &message,
                        });
                        println!("{}", event);
                    }
                }
            }
        }
    }

    let mut page = 1;
    let mut pages_count = 0;
    let last_downloads_count = session.downloads_count;
    let url = format!("{}/api/entries", &settings.base_url);
    let mut query = json!({
        "since": session.since,
        "sort": "updated",
        "order": "asc",
        "archive": 0,
        "page": page,
        "perPage": 8,
    });

    if is_detail_available(&client, &settings) {
        query["perPage"] = JsonValue::from(100);
        query["detail"] = JsonValue::from("metadata");
    }

    'outer: loop {
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
                if sigterm.load(Ordering::Relaxed) ||
                    (settings.balance_limit > 0 &&
                     session.downloads_count.saturating_sub(session.removals_count) >= settings.balance_limit) {
                    break 'outer;
                }

                let id = element.get("id")
                                .and_then(JsonValue::as_u64)
                                .ok_or_else(|| format_err!("missing id"))?;

                let title = element.get("title")
                                   .and_then(JsonValue::as_str)
                                   .map(decode_entities)
                                   .map(String::from)
                                   .unwrap_or_default();

                let published_by = element.get("published_by")
                                          .and_then(JsonValue::as_array)
                                          .map(|v| v.iter().filter_map(|x| x.as_str())
                                                           .filter(|x| !x.is_empty())
                                                           .collect::<Vec<&str>>())
                                          .map(|v| v.join(", "))
                                          .filter(|v| !v.is_empty())
                                          .unwrap_or_default();
                let domain_name = element.get("domain_name")
                                         .and_then(JsonValue::as_str)
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
                                  .and_then(JsonValue::as_str)
                                  .and_then(|v| DateTime::parse_from_str(v, DATE_FORMAT).ok())
                                  .map(|v| v.format("%Y").to_string())
                                  .unwrap_or_default();

                let updated_at = element.get("updated_at")
                                        .and_then(JsonValue::as_str)
                                        .and_then(|v| DateTime::parse_from_str(v, DATE_FORMAT).ok())
                                        .ok_or_else(|| format_err!("missing updated at"))?;

                session.since = updated_at.timestamp();

                let epub_path = save_path.join(&format!("{}.epub", id));
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
                    eprintln!("Can't download {}: {:#}.", id, err);
                    fs::remove_file(epub_path).ok();
                    continue;
                }

                session.downloads_count = session.downloads_count.wrapping_add(1);

                if let Ok(path) = epub_path.strip_prefix(&library_path) {
                    let file_info = json!({
                        "path": path,
                        "kind": "epub",
                        "size": file.metadata().ok()
                                    .map_or(0, |m| m.len()),
                    });

                    let info = json!({
                        "title": title,
                        "author": author,
                        "year": year,
                        "identifier": id.to_string(),
                        "added": updated_at.with_timezone(&Local)
                                           .format("%Y-%m-%d %H:%M:%S")
                                           .to_string(),
                        "file": file_info,
                    });

                    let event = json!({
                        "type": "addDocument",
                        "info": &info,
                    });

                    println!("{}", event);
                }
            }
        }

        page += 1;

        if page > pages_count {
            break;
        }

        query["page"] = JsonValue::from(page);
    }

    if pages_count > 0 {
        let downloads_count = session.downloads_count
                                     .saturating_sub(last_downloads_count);
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
