
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate hyper;
extern crate hyper_native_tls;
extern crate clap;
extern crate serde_json;

use hyper::client::*;
use hyper::net::HttpsConnector;

use hyper_native_tls::NativeTlsClient;

use clap::{Arg, App};

use serde_json::Value;

use std::io::*;
use std::env::*;
use std::sync::Arc;
use std::process::Command;
use std::thread;

struct Gerrit {
    pub path: String,
    pub addr: String,
}

impl Gerrit {
    pub fn new(path: &str, addr: &str) -> Gerrit {
        Gerrit {
            path: path.to_owned(),
            addr: addr.to_owned(),
        }
    }
}

fn process_branch(gerrit: &Gerrit, branch: &str) {

    let ref path = gerrit.path;
    let ref addr = gerrit.addr;
    let branch = branch.trim();

    // get last commit info
    let result = Command::new("git")
        .arg("rev-parse")
        .arg(branch)
        .current_dir(path)
        .output()
        .expect("fail to get last commit hash");

    let hash = String::from_utf8_lossy(&result.stdout);
    let hash = hash.trim();
    info!("brach: {}, hash: {}", branch, hash);
    let url = format!("{}/changes/?q=commit:{}", addr, hash);

    let ssl = NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    let client = Client::with_connector(connector);
    let mut response = client.get(&url).send().unwrap();
    let mut content = String::new();
    assert!(response.read_to_string(&mut content).is_ok());
    assert!(content.len() > 4);

    // remove )}]' characters
    content.drain(0..4);

    let json = content.parse::<Value>().unwrap();
    let list = json.as_array().unwrap();

    if list.is_empty() {
        return;
    }

    let object = list[0].as_object().unwrap();
    let status = object["status"].as_str().unwrap();

    println!("branch: {}, status: {}", branch, status);

    let delete = match status {
        "MERGED" | "ABANDONED" => true,
        _ => false,
    };

    if !delete {
        return;
    }

    let result = Command::new("git")
        .arg("branch")
        .arg("-D")
        .arg(branch)
        .current_dir(path)
        .output()
        .expect("delete branch failed");

    println!("{}", String::from_utf8_lossy(&result.stdout).trim());
}

fn main() {

    let matches = App::new("Gerrit tools")
        .version("1.0")
        .author("sbw <sbw@sbw.so>")
        .about("gerrit merged/abandoned branch cleanner")
        .arg(Arg::with_name("address")
            .short("a")
            .long("addr")
            .value_name("ADDR")
            .help("set gerrit address")
            .takes_value(true)
            .default_value("https://cr.deepin.io"))
        .get_matches();

    env_logger::init().unwrap();

    let addr = matches.value_of("address").unwrap();
    let path = current_dir().unwrap().to_str().unwrap().to_owned();

    info!("addr: {}", addr);
    info!("path: {}", path);

    let gerrit = Arc::new(Gerrit::new(&path, &addr));

    let result = Command::new("git")
        .arg("branch")
        .current_dir(&path)
        .output()
        .expect("fail to get branch info");

    let output = String::from_utf8_lossy(&result.stdout);

    let threads: Vec<_> = output.split('\n')
        .filter(|branch| {
            let branch = branch.trim();
            !branch.is_empty() && !branch.starts_with('*')
        })
        .map(|branch| {

            info!("process branch: {}", branch);

            let branch = branch.to_owned();
            let gerrit = gerrit.clone();
            thread::spawn(move || process_branch(&gerrit, &branch))
        })
        .collect();

    for t in threads {
        assert!(t.join().is_ok());
    }
}
