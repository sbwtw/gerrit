
extern crate hyper;
extern crate rustc_serialize;

use hyper::client::*;

use rustc_serialize::json::*;

use std::io::*;
use std::env::*;
use std::process::Command;
use std::thread;

static GERRIT: &'static str = "https://cr.deepin.io";

fn process_branch(path: &str, branch: &str) {

    let branch = branch.trim();
    // let client = Client::new();
    //println!("{}", branch);

    // get last commit info
    let result = Command::new("git")
                            .arg("rev-parse")
                            .arg(branch)
                            .current_dir(path)
                            .output()
                            .expect("fail to get last commit hash");

    let hash = String::from_utf8_lossy(&result.stdout);
    //let url = format!("{}/changes/?q=topic:{}+commit:{}", GERRIT, branch, hash);
    let url = format!("{}/changes/?q=commit:{}", GERRIT, hash);
    //println!("{:?}", url);

    let client = Client::new();
    let mut response = client.get(&url).send().unwrap();
    let mut content = String::new();
    assert!(response.read_to_string(&mut content).is_ok());
    assert!(content.len() > 4);

    // remove )}]' characters
    content.drain(0..4);

    let json = content.parse::<Json>().unwrap();
    let list = json.as_array().unwrap();

    if list.is_empty() {return;}

    let object = list[0].as_object().unwrap();
    let status = object["status"].as_string().unwrap();

    println!("branch: {}, status: {}", branch, status);

    let mut delete = false;
    match object["status"].as_string().unwrap() {
        "MERGED" | "ABANDONED"
            => delete = true,
        _ => {},
    }

    if !delete {return;}

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

    let path = current_dir().unwrap().to_str().unwrap().to_owned();

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

        let branch = branch.to_owned();
        let path = path.clone();
        thread::spawn(move || {
            process_branch(&path, &branch)
        })
    }).collect();

    for t in threads {
        assert!(t.join().is_ok());
    }
}
