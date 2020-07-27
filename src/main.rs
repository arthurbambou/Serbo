#![feature(proc_macro_hygiene, decl_macro)]

use rand::{thread_rng, Rng};
use rocket::request::{Form, FromForm};
use rocket::State;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::Error;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;

#[macro_use]
extern crate rocket;

#[derive(FromForm)]
struct Target {
    target_id: u32,
    caller_id: u32,
}

struct ServerInstance {
    server_process: std::process::Child,
    stdout_join: Option<std::thread::JoinHandle<()>>,
    port: u32,
}

struct Servers {
    refs: Mutex<HashMap<String, ServerInstance>>,
}

fn delete_server(server_id: u32) -> Result<(), Error> {
    fs::remove_dir_all(format!("servers/{}", server_id))?;
    Ok(())
}

fn create_server() -> io::Result<u32> {
    let id = rand::random::<u32>();
    fs::create_dir(format!("servers/{}", id))?;
    fs::copy("server/eula.txt", format!("servers/{}/eula.txt", id))?;
    fs::copy(
        "server/minecraft_server.1.16.1.jar",
        format!("servers/{}/minecraft_server.1.16.1.jar", id),
    )?;
    Ok(id)
}

fn stop_server(server: &mut ServerInstance) -> Result<(), Error> {
    server.server_process.kill()?;
    server.stdout_join.take().unwrap().join();
    Ok(())
}

fn start_server(id: u32) -> Result<ServerInstance, Error> {
    let mut rng = thread_rng();
    let port = rng.gen_range(25565, 65565);
    let mut command = Command::new("java");
    command
        .stdout(Stdio::piped())
        .args(&[
            "-Xmx1024M",
            "-Xms1024M",
            "-jar",
            "minecraft_server.1.16.1.jar",
            "nogui",
            "--port",
            &port.to_string(),
        ])
        .current_dir(format!("servers/{}", id.to_string()));
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let handle = thread::spawn(|| {
        let reader = BufReader::new(stdout);
        reader
            .lines()
            .filter_map(|line| line.ok())
            .for_each(|line| println!("{}", line));
    });
    Ok(ServerInstance {
        server_process: child,
        stdout_join: Some(handle),
        port: port,
    })
}

#[post("/stop", data = "<target>")]
fn stop(target: Form<Target>, servers: State<Servers>) -> String {
    let mut map = servers.refs.lock().expect("locks");
    if !map.contains_key(&target.target_id.to_string()) {
        return String::from("-1");
    }
    let server = map.get_mut(&target.target_id.to_string()).unwrap();
    let port = server.port.to_string();
    stop_server(server);
    map.remove(&target.target_id.to_string());
    String::from("1")
}

#[post("/start", data = "<target>")]
fn start(target: Form<Target>, servers: State<Servers>) -> String {
    let mut map = servers.refs.lock().expect("locks");
    if map.contains_key(&target.target_id.to_string()) {
        return String::from("0");
    }
    let child = start_server(target.target_id);
    match child {
        Ok(x) => {
            let port = x.port.to_string();
            map.insert(target.target_id.to_string(), x);
            return String::from(port);
        },
        Err(e) => return String::from("-1"),
    };
}


#[post("/delete", data = "<target>")]
fn delete(target: Form<Target>, servers:State<Servers>) -> String {
    let mut map = servers.refs.lock().expect("locks");
    if map.contains_key(&target.target_id.to_string()) {
        let server_inst = map.get_mut(&target.target_id.to_string()).unwrap();
        stop_server(server_inst);
    }
    match delete_server(target.target_id){
        Ok(_) => return String::from("1"),
        Err(e) => return String::from("-1")
    };
}

#[post("/create", data = "<target>")]
fn create(target: Form<Target>) -> String {
    match create_server() {
        Ok(id) => return String::from(id.to_string()),
        Err(e) => return String::from("-1")
    }
}

fn main() {
    rocket::ignite()
        .manage(Servers {
            refs: Mutex::new(HashMap::new()),
        })
        .mount("/", routes![create, start, stop, delete])
        .launch();
}
