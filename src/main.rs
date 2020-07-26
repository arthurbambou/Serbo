#![feature(proc_macro_hygiene, decl_macro)]

use rocket::request::{Form,FromForm};
use rocket::State;
use std::process::{Command, Stdio,Child};
use std::fs;
use std::io;
use std::io::{Error};
use rand::{thread_rng, Rng};
use std::io::{BufRead, BufReader};
use std::collections::HashMap;
use std::sync::Mutex;


#[macro_use] extern crate rocket;

struct Servers {
    refs:Mutex<HashMap<String,Child>>
}

#[derive(FromForm)]
struct Target{
    target_id: u32,
    caller_id:u32
}

fn create_server() -> io::Result<u32> {
    let id = rand::random::<u32>();
    fs::create_dir(format!("servers/{}",id))?;
    fs::copy("server/eula.txt", format!("servers/{}/eula.txt",id))?;
    fs::copy("server/minecraft_server.1.16.1.jar", format!("servers/{}/minecraft_server.1.16.1.jar",id))?;
    Ok(id)
}

fn start_server(id:u32) -> Result<Child,Error>{
    let mut rng = thread_rng();
    let port = rng.gen_range(25565,65565);
    let mut command = Command::new("java");
    println!("{}",port);
    command.stdout(Stdio::piped())
    .args(&["-Xmx1024M","-Xms1024M","-jar","minecraft_server.1.16.1.jar","nogui","--port",&port.to_string()])
    .current_dir(format!("servers/{}",id.to_string()));
    let mut child = command.spawn()?;
    /*
    let stdout = child.stdout.take().unwrap();
    
    let reader = BufReader::new(stdout);
    reader
        .lines()
        .filter_map(|line| line.ok())
        .for_each(|line| println!("{} oberma", line));
        */
    Ok(child)
    
}

#[post("/stop", data="<target>")]
fn stop(target:Form<Target>, servers: State<Servers>) -> String{

        let mut map = servers.refs.lock().expect("locks");
        if !map.contains_key(&target.target_id.to_string()) {
            return String::from("Server not running")
        }
        let child = map.get_mut(&target.target_id.to_string()).unwrap();
        child.kill().expect("Not running");
        map.remove(&target.target_id.to_string());
    
    String::from("Server Stopped")
}

#[post("/start", data="<target>")]
fn start(target:Form<Target>, servers: State<Servers>) -> String{

        let mut map = servers.refs.lock().expect("locks");
        if map.contains_key(&target.target_id.to_string()) {
            return String::from("Already Running")
        }
        let child = start_server(target.target_id);
        match child{
            Ok(x) => map.insert(target.target_id.to_string(),x),
            Err(e) => return String::from("Server Started")
        };
    
    String::from("Server Started")
}

#[post("/create", data="<target>")]
fn create(target:Form<Target>) -> String {
    match create_server(){
        Ok(id) => println!("{}",id),
        Err(e) => println!("{}",e)
    }
    String::from("Hello")
}

fn main() {
    rocket::ignite()
    .manage(Servers{ refs:Mutex::new(HashMap::new()) })
    .mount("/", routes![create,start,stop]).launch();
}
