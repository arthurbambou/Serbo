#![feature(proc_macro_hygiene, decl_macro)]

use rand::{thread_rng, Rng};
use rocket::request::{Form, FromForm};
use rocket::State;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::{BufRead, BufReader, Error};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::thread::sleep;
use std::time;

#[macro_use]
extern crate rocket;

const valid_versions: [&'static str; 2] = ["1.15.2", "1.16.1"];

#[derive(FromForm)]
struct Target {
    target_id: u32,
    caller_id: u32,
}

#[derive(FromForm)]
struct VersionTarget {
    target_id: u32,
    caller_id: u32,
    target_version: String,
}

#[derive(FromForm)]
struct CreateTarget {
    caller_id: u32,
}

struct ServerInstance {
    server_process: Child,
    stdout_join: Option<thread::JoinHandle<()>>,
    port: u32,
}

struct Servers {
    refs: Mutex<HashMap<String, ServerInstance>>,
}

fn change_version(server_id: u32, target: String) -> Result<(), Error> {
    fs::remove_file(format!("servers/{}/server.jar", server_id))?;
    fs::copy(
        format!("server/{}/server.jar", target),
        format!("servers/{}/server.jar", server_id),
    )?;
    Ok(())
}

fn delete_server(server_id: u32) -> Result<(), Error> {
    fs::remove_dir_all(format!("servers/{}", server_id))?;
    Ok(())
}

fn create_server() -> io::Result<u32> {
    let id = rand::random::<u32>();
    fs::create_dir(format!("servers/{}", id))?;
    fs::copy("server/1.16.1/eula.txt", format!("servers/{}/eula.txt", id))?;
    fs::copy(
        "server/1.16.1/server.jar",
        format!("servers/{}/server.jar", id),
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
    let port = rng.gen_range(25565, 35565);
    let mut command = Command::new("java");
    command
        .stdout(Stdio::piped())
        .args(&[
            "-Xmx1024M",
            "-Xms1024M",
            "-jar",
            "server.jar",
            "nogui",
            "--port",
            &port.to_string(),
        ])
        .current_dir(format!("servers/{}", id.to_string()));
    let mut child = command.spawn()?;
    Ok(ServerInstance {
        server_process: child,
        stdout_join: None,
        port: port,
    })
}

#[post("/version", data = "<target>")]
fn version(target: Form<VersionTarget>, servers: State<Servers>) -> String {
    let valid_version = valid_versions.iter().any(|x| *x == target.target_version);
    if !valid_version {
        return String::from("-1");
    }
    let mut map = servers.refs.lock().expect("locks");
    if let Some(server_inst) = map.get_mut(&target.target_id.to_string()) {
        stop_server(server_inst);
        map.remove(&target.target_id.to_string());
    }
    match change_version(
        target.target_id,
        String::from(target.target_version.clone()),
    ) {
        Ok(_) => return String::from("1"),
        Err(e) => return String::from("-1"),
    };
}

#[post("/stop", data = "<target>")]
fn stop(target: Form<Target>, servers: State<Servers>) -> String {
    let mut map = servers.refs.lock().expect("locks");
    if !map.contains_key(&target.target_id.to_string()) {
        return String::from("-1");
    }
    let server_inst = map.get_mut(&target.target_id.to_string()).unwrap();
    let port = server_inst.port.to_string();
    stop_server(server_inst);
    map.remove(&target.target_id.to_string());
    String::from("1")
}

#[post("/start", data = "<target>")]
fn start(target: Form<Target>, servers: State<Servers>) -> String {
    let mut map = servers.refs.lock().expect("locks");
    if map.contains_key(&target.target_id.to_string()) {
        return String::from("0");
    }
    let server_inst = start_server(target.target_id);
    match server_inst {
        Ok(mut x) => {
            let port = x.port.clone().to_string();
            let mut rng = thread_rng();

            let stdout = x.server_process.stdout.take().unwrap();
            let ws_port = rng.gen_range(45565, 55565);
            let ws = TcpListener::bind(format!("127.0.0.1:{}", ws_port)).unwrap();

            println!("{} port", ws_port);

            let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let stdout_reader_mutex = lines.clone();
            let ws_thread_handle = thread::spawn(move || {
                let mut client_count = 0;
                for stream in ws.incoming() {
                    let mutex = lines.clone();
                    client_count += 1;
                    thread::spawn(move || {
                        let mut counter = 0;
                        loop {
                            let mut stdout_lines = mutex.lock().expect("locks");
                            for x in counter..stdout_lines.len(){
                                println!("{} output {}", stdout_lines[x], client_count);
                                counter += 1;
                            }
                            drop(stdout_lines);
                            sleep(time::Duration::from_secs(2));
                        }
                    });
                }
            });
            let stdout_thread_handle = thread::spawn(move || {
                let reader = BufReader::new(stdout).lines();
                reader.filter_map(|line| line.ok()).for_each(|line| {
                    let mut stdout_lines = stdout_reader_mutex.lock().expect("locks");
                    stdout_lines.push(line);
                })
            });
            x.stdout_join = Some(stdout_thread_handle);
            map.insert(target.target_id.to_string(), x);
            return String::from(port);
        }
        Err(e) => return String::from("-1"),
    };
}

#[post("/delete", data = "<target>")]
fn delete(target: Form<Target>, servers: State<Servers>) -> String {
    let mut map = servers.refs.lock().expect("locks");
    if let Some(server_inst) = map.get_mut(&target.target_id.to_string()) {
        stop_server(server_inst);
        map.remove(&target.target_id.to_string());
    }
    match delete_server(target.target_id) {
        Ok(_) => return String::from("1"),
        Err(e) => return String::from("-1"),
    };
}

#[post("/create", data = "<target>")]
fn create(target: Form<CreateTarget>) -> String {
    match create_server() {
        Ok(id) => return String::from(id.to_string()),
        Err(e) => return String::from("-1"),
    }
}

fn main() {
    rocket::ignite()
        .manage(Servers {
            refs: Mutex::new(HashMap::new()),
        })
        .mount("/", routes![create, start, stop, delete, version])
        .launch();
}
