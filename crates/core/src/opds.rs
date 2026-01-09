use std::fmt::Display;
use std::{fs::File, io::Write, path::PathBuf, str::FromStr};

use anyhow::{format_err, Error};
use attohttpc::Response;
use url::{Position, Url};

use crate::document::html::xml::XmlParser;
use crate::helpers::decode_entities;
use crate::settings::OpdsSettings;

#[derive(PartialEq, Debug, Clone)]
pub enum MimeType {
    Epub,
    Cbz,
    Pdf,
    OpdsCatalog,
    OpdsEntry,
    Other(String),
}

impl Display for MimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match *self {
            MimeType::Epub => "epub".to_string(),
            MimeType::Cbz => "cbz".to_string(),
            MimeType::Pdf => "pdf".to_string(),
            MimeType::OpdsCatalog => "xml".to_string(),
            MimeType::OpdsEntry => "xml".to_string(),
            MimeType::Other(ref s) => s.to_string(),
        };
        write!(f, "{}", str)
    }
}

impl FromStr for MimeType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "application/epub+zip" => Ok(MimeType::Epub),
            "application/x-cbz" => Ok(MimeType::Cbz),
            "application/pdf" => Ok(MimeType::Pdf),
            "application/atom+xml;profile=opds-catalog" => Ok(MimeType::OpdsCatalog),
            "application/atom+xml;type=entry;profile=opds-catalog" => Ok(MimeType::OpdsEntry),
            _ => Ok(MimeType::Other(s.to_string())),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum LinkType {
    Acquisition,
    Cover,
    Thumbnail,
    Sample,
    OpenAccess,
    Borrow,
    Buy,
    Subscribe,
    Subsection,
    Other(String),
}

impl FromStr for LinkType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http://opds-spec.org/acquisition" => Ok(LinkType::Acquisition),
            "http://opds-spec.org/image" => Ok(LinkType::Cover),
            "http://opds-spec.org/image/thumbnail" => Ok(LinkType::Thumbnail),
            "http://opds-spec.org/acquisition/sample" => Ok(LinkType::Sample),
            "http://opds-spec.org/acquisition/preview" => Ok(LinkType::Sample),
            "http://opds-spec.org/acquisition/open-access" => Ok(LinkType::OpenAccess),
            "http://opds-spec.org/acquisition/borrow" => Ok(LinkType::Borrow),
            "http://opds-spec.org/acquisition/buy" => Ok(LinkType::Buy),
            "http://opds-spec.org/acquisition/subscribe" => Ok(LinkType::Subscribe),
            "subsection" => Ok(LinkType::Subsection),
            _ => Ok(LinkType::Other(s.to_string())),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Feed {
    pub title: String,
    pub entries: Vec<Entry>,
    // pub links: Vec<Link>,
}

#[derive(Default, Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub links: Vec<Link>,
}

#[derive(Default, Debug, Clone)]
pub struct Link {
    pub rel: Option<LinkType>,
    pub href: Option<String>,
    pub mime_type: Option<MimeType>,
}

#[derive(Debug, Clone)]
pub struct OpdsFetcher {
    pub settings: OpdsSettings,
    pub root_url: Url,
    pub base_url: Url,
}

impl OpdsFetcher {
    pub fn new(settings: OpdsSettings) -> Result<OpdsFetcher, Error> {
        let root_url = Url::parse(&settings.url)?;
        let base_url = Url::parse(&root_url[..Position::BeforePath])?;

        Ok(OpdsFetcher {
            settings: settings,
            root_url: root_url,
            base_url: base_url,
        })
    }

    pub fn download_relative(&self, path: &str, file_path: &PathBuf) -> Result<File, Error> {
        let full_url = Url::join(&self.base_url, path)?;
        let mut file = File::create(&file_path)?;
        let response: Response = self.request(&full_url)?;
        //TODO check success
        let bytes = response.bytes()?;
        let _ = file.write(&bytes);
        return Ok(file);
    }

    pub fn home(&self) -> Result<Feed, Error> {
        let response = self.request(&self.root_url)?;
        //TODO check success
        return OpdsFetcher::parse_feed(response);
    }

    pub fn pull_relative(&self, path: &str) -> Result<Feed, Error> {
        let full_url = Url::join(&self.base_url, path)?;
        let response = self.request(&full_url)?;
        //TODO check success
        return OpdsFetcher::parse_feed(response);
    }

    fn parse_feed(response: Response) -> Result<Feed, Error> {
        let body = response.text()?;
        let root = XmlParser::new(&body).parse();
        //println!("{:?}", body);
        let feed = root
            .root()
            .find("feed")
            .ok_or_else(|| format_err!("feed is missing"))?;
        let mut entries = Vec::new();
        let mut title = String::new();
        for child in feed.children() {
            if child.tag_name() == Some("title") {
                title = decode_entities(&child.text()).into_owned();
            }
            if child.tag_name() == Some("entry") {
                let mut find_id = None;
                let mut find_title = None;
                let mut author = None;
                let mut links = Vec::new();
                for entry_child in child.children() {
                    if entry_child.tag_name() == Some("id") {
                        find_id = Some(entry_child.text());
                    }
                    if entry_child.tag_name() == Some("title") {
                        find_title = Some(decode_entities(&entry_child.text()).into_owned());
                    }
                    if entry_child.tag_name() == Some("author") {
                        if let Some(name) = entry_child.find("name") {
                            author = Some(decode_entities(&name.text()).into_owned());
                        }
                    }
                    if entry_child.tag_name() == Some("link") {
                        let rel = entry_child
                            .attribute("rel")
                            .map(|s| LinkType::from_str(s).ok())
                            .flatten();
                        let href = entry_child.attribute("href").map(String::from);
                        let mime_type = entry_child
                            .attribute("type")
                            .map(|s| MimeType::from_str(s).ok())
                            .flatten();
                        links.push(Link {
                            rel: rel,
                            href: href,
                            mime_type: mime_type,
                        });
                    }
                }
                //TODO error
                if let (Some(id), Some(title)) = (find_id, find_title) {
                    let entry = Entry {
                        id: id,
                        title: title,
                        author: author,
                        links: links,
                    };
                    //println!("{:#?}", entry);
                    entries.push(entry);
                }
            }
        }
        Ok(Feed {
            title: title,
            entries: entries,
        })
    }

    fn request(&self, url: &Url) -> Result<Response, Error> {
        let mut request_builder = attohttpc::get(url);
        if let Some(username) = self.settings.username.clone() {
            request_builder = request_builder.basic_auth(username, self.settings.password.clone());
        }
        let response = request_builder.send()?;
        return Ok(response);
    }
}
