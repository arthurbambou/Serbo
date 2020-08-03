#![feature(proc_macro_hygiene, decl_macro)]

use rocket::request::{Form, FromForm};
use rocket::State;
use std::sync::Mutex;
use std::io::Write;
use rocket_contrib::json::Json;

use serbo;

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
    version:String
}

#[derive(FromForm)]
struct ConsoleWriteTarget {
    caller_id: u32,
    target_id: u32,
    msg: String
}

#[derive(FromForm)]
struct ConsoleTarget {
    caller_id: u32,
    target_id: u32,
    start_line: u32
}

struct StateStruct{
    servers: Mutex<serbo::Manager>
}

#[post("/writeConsole", data = "<target>")]
fn _write(target: Form<ConsoleWriteTarget>, state:State<StateStruct>) -> String{
    if let Some(instance) = state.servers.lock().unwrap().get(target.target_id.to_string()){
        instance.send(target.msg.clone());
        return String::from("1");
    }
    String::from("-1")
}

#[post("/version", data = "<target>")]
fn version(target: Form<VersionTarget>, state:State<StateStruct>) -> String {
    match state.servers.lock().unwrap().change_version(target.target_id.to_string(),target.target_version.clone()){
        Ok(_) => String::from("1"),
        Err(e) => String::from("-1")
  }
}

#[post("/stop", data = "<target>")]
fn stop(target: Form<Target>, state:State<StateStruct>) -> String {
    match state.servers.lock().unwrap().stop(target.target_id.to_string()){
        Ok(_) => String::from("1"),
        Err(e) => String::from("-1")
  }
}

#[post("/getConsole", data="<target>")]
fn get_console(target: Form<ConsoleTarget>, state:State<StateStruct>) -> Json<Vec<String>>{
    if let Some(instance) = state.servers.lock().unwrap().get(target.target_id.to_string()){
        return Json(instance.get(target.start_line))
    }
    Json(Vec::new())
}

#[post("/start", data = "<target>")]
fn start(target: Form<Target>, state:State<StateStruct>) -> String {
  match state.servers.lock().unwrap().start(target.target_id.to_string()){
    Ok(_) => String::from("1"),
    Err(e) => String::from("-1")
  }
}

#[post("/delete", data = "<target>")]
fn delete(target: Form<Target>, state:State<StateStruct>) -> String {
  match state.servers.lock().unwrap().delete(target.target_id.to_string()){
      Ok(_) => String::from("1"),
      Err(e) => String::from("-1")
  }
}

#[post("/create", data = "<target>")]
fn create(target: Form<CreateTarget>,state:State<StateStruct>) -> String {
    match state.servers.lock().unwrap().create(target.version.clone()){
        Ok(port) => String::from(port.to_string()),
        Err(e) => String::from("-1")
    }
}

fn main() {
    let state = StateStruct{
        servers:Mutex::new(serbo::Manager::new(String::from("servers"),String::from("server")))
    };
    rocket::ignite()
        .manage(state)
        .mount("/", routes![create, start, stop, delete, version,get_console,_write])
        .launch();
}
