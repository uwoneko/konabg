use std::{fs, thread};
use std::fs::File;
use std::path::Path;
use std::process::Command;

use chrono::{Duration, Utc};
use clap::{arg, Parser, Subcommand};
use reqwest::{Proxy, Url};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

const ROOT_DIR: &str = "konabg";
const SAFE_DIR: &str = "safe";
const LEWDS_DIR: &str = "explicit";
const IMAGES_DIR: &str = "images";
const PAGES_DIR: &str = "pages";
const CONFIG_FILE: &str = "config.json";
const CURRENT_FILE: &str = "current.json";

fn main() {
    // setup
    let args = Cli::parse();
    let paths = Paths::create(args.lewds);

    dbg!(&args);
    dbg!(&paths);
    
    fs::create_dir_all(&paths.images_dir).unwrap();
    fs::create_dir_all(&paths.pages_dir).unwrap();

    let mut client_builder = reqwest::blocking::ClientBuilder::new();

    let config = Config::read_or_create(&paths.config_file);
    let mut current = Current::read_or_create(&paths.current_file);

    dbg!(&config);

    if let Some(proxy) = &config.proxy {
        dbg!(&proxy);
        client_builder = client_builder.proxy(Proxy::all(proxy).unwrap());
    }

    let client = client_builder.build().unwrap();

    // update bg
    let bg = if let Some(bg) = current.bg {
        match args.commands {
            Commands::Next => { bg + 1 }
            Commands::Prev => { bg.saturating_sub(1) }
            Commands::Refresh => { bg }
            Commands::Set { new_bg } => { new_bg }
        }
    } else {
        match args.commands {
            Commands::Set { new_bg } => { new_bg },
            _ => { 0 }
        }
    };
    
    dbg!(bg);
    
    current.bg = Some(bg);
    fs::write(&paths.current_file, serde_json::to_string_pretty(&current).unwrap()).unwrap();
    dbg!(&current);
    
    // load & update
    let posts = read_or_query_posts(&client, (bg + 1) / 100, &config.tags, args.lewds, config.time, &paths.pages_dir);

    let post = &posts[(bg % 100) as usize];
    dbg!(post);
    
    let image_path = paths.images_dir.join(format!("{}.jpg", post.id));
    let lock_path = paths.images_dir.join(format!("{}.lock", post.id));

    if lock_path.exists() {
        eprintln!("waiting on lock");
        
        while lock_path.exists() {
            thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    if image_path.exists() {
        change_bg(&image_path);
    } else {
        File::create(&lock_path).unwrap();
        dbg!(&post.jpeg_url);
        post.download_to_file(&client, &image_path);
        change_bg(&image_path);
        fs::remove_file(lock_path).unwrap();
    };

    // preload
    match args.commands {
        Commands::Next | Commands::Refresh | Commands::Set { .. } => {
            let next_posts = read_or_query_posts(&client, (bg + 2) / 100, &config.tags, args.lewds, config.time, &paths.pages_dir);

            let post = &next_posts[((bg + 1) % 100) as usize];
            let image_path = paths.images_dir.join(format!("{}.jpg", post.id));
            let lock_path = paths.images_dir.join(format!("{}.lock", post.id));

            if !image_path.exists() && !lock_path.exists() {
                File::create(&lock_path).unwrap();
                post.download_to_file(&client, &image_path);
                fs::remove_file(lock_path).unwrap();
            }
        }
        Commands::Prev => { }
    }
}

fn read_or_query_posts(client: &Client, page: u32, tags: &str, lewds: bool, duration: u64, pages_dir: &Path) -> Vec<Post> {
    let page_path = pages_dir.join(format!("page_{}.json", page));
    dbg!(page);
    dbg!(&page_path);

    let posts = if !page_path.exists() {
        let posts = query_posts(
            &client,
            page,
            100,
            &format!("{} {}", tags, if lewds { "rating:e" } else { "rating:s"} ),
            duration
        );

        dbg!(&posts);

        fs::write(&page_path, serde_json::to_string(&posts).unwrap()).unwrap();

        posts
    } else {
        serde_json::from_str(&fs::read_to_string(&page_path).unwrap()).unwrap()
    };

    posts
}

fn query_posts(client: &Client, page: u32, limit: u8, tags: &str, duration: u64) -> Vec<Post> {
    let date = Utc::now() - Duration::seconds(duration as i64);
    let formatted_date = date.format("%Y-%m-%d").to_string();

    let url = Url::parse_with_params(
        "https://konachan.com/post.json",
        [
            ("tags", format!("order:score date:>{} {}", formatted_date, tags).as_str()),
            ("page", format!("{}", page + 1).as_str()),
            ("limit", format!("{}", limit).as_str())
        ]
    ).unwrap();

    let response = client.get(url)
        .send()
        .unwrap()
        .text()
        .unwrap();

    dbg!(&response);

    serde_json::from_str(&response).unwrap()
}

fn change_bg(path: &Path) {
    if !Command::new("swww")
        .arg("img")
        .arg(path)
        .arg("--transition-type")
        .arg("wipe")
        .arg("--transition-fps")
        .arg("60")
        .arg("--transition-step")
        .arg("30")
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success() {
        panic!("could not change bg");
    }
}


#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    commands: Commands,
    #[arg(short, long)]
    lewds: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// loads the next image
    Next,
    /// loads the previous image
    Prev,
    /// loads the same image
    Refresh,
    /// sets a new current image
    Set {
        new_bg: u32
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Config {
    tags: String,
    time: u64,
    #[serde(default)]
    proxy: Option<String>
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tags: "".to_string(),
            time: 60 * 60 * 24 * 365,
            proxy: None,
        }
    }
}

macro_rules! read_or_create {
    ($t:ty) => {
        impl $t {
            pub fn read_or_create(path: &std::path::Path) -> $t {
                std::fs::read_to_string(path)
                    .map_or_else(
                        |_| {
                            let default = <$t>::default();
                            std::fs::write(path, serde_json::to_string_pretty(&default).unwrap()).unwrap();
                            default
                        },
                        |file| serde_json::from_str(&file).expect(&format!("{} should be valid", stringify!($t)))
                    )
            }
        }
    };
}

read_or_create!(Config);

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Current {
    bg: Option<u32>
}

read_or_create!(Current);

#[derive(Clone, Debug, )]
struct Paths {
    images_dir: Box<Path>,
    pages_dir: Box<Path>,
    config_file: Box<Path>,
    current_file: Box<Path>,
}

impl Paths {
    pub fn create(lewds: bool) -> Self {
        let root_dir = dirs::data_dir().unwrap().join(ROOT_DIR);
        let rating_root_dir = if lewds {
            root_dir.join(LEWDS_DIR)
        } else {
            root_dir.join(SAFE_DIR)
        };
        Paths {
            images_dir: rating_root_dir.join(IMAGES_DIR).into_boxed_path(),
            pages_dir: rating_root_dir.join(PAGES_DIR).into_boxed_path(),
            current_file: rating_root_dir.join(CURRENT_FILE).into_boxed_path(),
            config_file: root_dir.join(CONFIG_FILE).into_boxed_path(),
        }
    }
}

// boilerplate post def
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Post {
    id: u32,
    // tags: String,
    // created_at: u64,
    // creator_id: u32,
    // author: String,
    // change: u32,
    // source: String,
    // score: u32,
    // md5: String,
    // file_size: u32,
    // file_url: String,
    // is_shown_in_index: bool,
    // preview_url: String,
    // preview_width: u32,
    // preview_height: u32,
    // actual_preview_width: u32,
    // actual_preview_height: u32,
    // sample_url: String,
    // sample_width: u32,
    // sample_height: u32,
    // sample_file_size: u32,
    jpeg_url: String,
    // jpeg_width: u32,
    // jpeg_height: u32,
    // jpeg_file_size: u32,
    // rating: String,
    // has_children: bool,
    // parent_id: Option<u32>,
    // status: String,
    // width: u32,
    // height: u32,
    // is_held: bool,
    // frames_pending_string: String,
    // frames_pending: Vec<String>,
    // frames_string: String,
    // frames: Vec<String>,
}

impl Post {
    fn download_to_file(&self, client: &Client, path: impl AsRef<Path>) {
        let bytes = client.get(&self.jpeg_url)
            .send()
            .unwrap()
            .bytes()
            .unwrap();

        fs::write(path, bytes).unwrap();
    }
}