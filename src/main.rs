extern crate tokio_core;
extern crate hyper;
extern crate hyper_tls;
extern crate futures;
extern crate clap;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;
extern crate num_cpus;
extern crate serde_json;

use hyper::Client;
use hyper_tls::HttpsConnector;
use futures::future::{Future, join_all};
use futures::sync::mpsc::channel;
use futures::{Stream, Sink};
use clap::{App, Arg};
use serde_json::Value;

use std::env::*;
use std::ops::Deref;
use std::cell::RefCell;
use std::thread;
use std::process::Command;

fn main() {
    let matches = App::new("Gerrit tools")
        .version("2.0")
        .author("sbw <sbw@sbw.so>")
        .about("gerrit merged/abandoned branch cleanner")
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("addr")
                .value_name("ADDR")
                .help("set gerrit address")
                .takes_value(true)
                .default_value("https://cr.deepin.io"),
        )
        .get_matches();

    pretty_env_logger::init().unwrap();

    let addr = matches.value_of("address").unwrap();
    // let path = "/home/Projects/dde-session-ui";
    let path = current_dir().unwrap().to_str().unwrap().to_owned();

    info!("addr: {}", addr);
    info!("path: {}", path);

    let mut core = tokio_core::reactor::Core::new().unwrap();
    let handle = core.handle();
    let client = Client::configure()
        .connector(HttpsConnector::new(4, &handle).unwrap())
        .build(&handle);

    let result = Command::new("git")
        .arg("branch")
        .current_dir(&path)
        .output()
        .expect("fail to get branch info");

    let output = String::from_utf8_lossy(&result.stdout);
    let r: Vec<String> = output
        .split('\n')
        .map(|b| b.trim())
        .filter(|b| !b.is_empty() && !b.starts_with('*'))
        .map(|b| b.to_owned())
        .collect();

    let (mut tx, rx) = channel(num_cpus::get());
    let p = path.clone();
    thread::spawn(move || for b in r {
        let exec = Command::new("git")
            .arg("rev-parse")
            .arg(b.clone())
            .current_dir(&p)
            .output()
            .expect("failed to get commit hash");

        let hash = String::from_utf8_lossy(&exec.stdout).trim().to_owned();

        tx = tx.send((b, hash)).wait().unwrap();
    });

    let p = RefCell::new(path);
    let r: Vec<_> = rx.map(|(b, h)| {
        info!("{} {}", b, h);

        let p = p.borrow();
        let url = format!("{}/changes/?q=commit:{}", addr, h);
        client.get(url.parse().unwrap()).and_then(move |r| {
            r.body().concat2().and_then(move |r| {

                let json: Value = serde_json::from_slice(&r[4..]).unwrap();
                let list = json.as_array().unwrap();
                if list.is_empty() {
                    return Ok(());
                }

                let object = list[0].as_object().unwrap();
                let status = object["status"].as_str().unwrap();

                println!("branch: {}, status: {}", b, status);

                match status {
                    "MERGED" | "ABANDONED" => {}
                    _ => return Ok(()),
                }

                let exec = Command::new("git")
                    .arg("branch")
                    .arg("-D")
                    .arg(b)
                    .current_dir(p.deref())
                    .output()
                    .expect("failed to get commit hash");
                println!("{}", String::from_utf8_lossy(&exec.stdout).trim());

                Ok(())
            })
        })

    }).collect()
        .wait()
        .unwrap();

    let _ = core.run(join_all(r)).unwrap();
}
