//! Allows for simple control, and input / output of minecraft servers.
//!
//! # Examples
//! ```
//!use serbo;
//!use std::error::Error;
//!use std::io;
//!
//!fn main() -> Result<(), Box<dyn Error>> {
//!  let mut manager = serbo::Manager::new("servers", "versions","fabric-server-launch.jar");
//!  let port = 25565;
//!  let id = "1";
//!  loop {
//!    let reader = io::stdin();
//!    let mut buf = String::new();
//!    println!("Enter your command.");
//!    reader.read_line(&mut buf)?;
//!    match buf.trim() {
//!      "delete" => {
//!        match manager.delete(id){
//!          Ok(_) => println!("Server deleted."),
//!          Err(e) => println!("{}",e)
//!        }
//!      }
//!      "change_version" => {
//!        let mut send_buf = String::new();
//!        println!("Enter the version to change to.");
//!        reader.read_line(&mut send_buf)?;
//!        //Remove the newline from read_line
//!        send_buf = send_buf[..send_buf.chars().count() - 1].to_string();
//!        manager.change_version(id, &send_buf)?;
//!      }
//!      "create" => match manager.create(id, "1.16.1-fabric") {
//!        Ok(_) => println!("Server Created"),
//!        Err(e) => println!("{}", e),
//!      },
//!      "stop" => {
//!        //Stops the server
//!        println!("Server stopping.");
//!        manager.stop(id)?;
//!        break Ok(());
//!      }
//!      "start" => {
//!        //Starts the server
//!        println!("Server starting.");
//!        match manager.start(id, port) {
//!          Err(e) => println!("{}", e),
//!          Ok(_) => println!("Server started!"),
//!        };
//!      }
//!      "send" => {
//!        //Prompts for a command to send to the server
//!        let instance = manager.get(id);
//!        match instance {
//!          Some(i) => {
//!            let mut send_buf = String::new();
//!            println!("Enter the command to send to the server.");
//!            reader.read_line(&mut send_buf)?;
//!            //Remove the newline from read_line
//!            send_buf = send_buf[..send_buf.chars().count() - 1].to_string();
//!            i.send(send_buf)?;
//!          }
//!          None => println!("Server offline."),
//!        }
//!      }
//!      "get" => {
//!        //Gets the last 5 stdout lines
//!        let instance: &serbo::Instance = manager.get(id).unwrap();
//!        let vec = instance.get(0);
//!        let length = vec.len();
//!        //Create a vec from the last 5 lines
//!        let trimmed_vec;
//!        if length >= 5 {
//!          trimmed_vec = Vec::from(&vec[length - 5..]);
//!        } else {
//!          trimmed_vec = Vec::from(vec);
//!        }
//!        for line in trimmed_vec {
//!          println!("{}", line);
//!        }
//!      }
//!      _ => {
//!        println!("Unrecognized command");
//!      }
//!    }
//!  }
//!}
//! ```

use copy_dir::copy_dir;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::sleep;
use std::time;

type Result<T> = std::result::Result<T, Error>;


#[derive(Debug)]
pub enum Error {
  /// Arises when there is an error regarding IO
  IoError(std::io::Error),
  /// Arises when an offline server is attempted to be used
  ServerOffline(String),
  /// Arises when a server's files are missing
  ServerFilesMissing(String),
  /// Arises when attempting to create a server with the same id as an existing server
  ServerAlreadyExists(String),
  /// Arises when there is an error involving a server's stdin/stdout threads
  ThreadError(String)
}

impl std::error::Error for Error{
  fn description(&self) -> &str{
    match *self{
      Error::IoError(_) => "IOError",
      Error::ServerFilesMissing(_) => "MissingServer",
      Error::ServerOffline(_) => "ServerOffline",
      Error::ServerAlreadyExists(_) => "ServerAlreadyExists",
      Error::ThreadError(_) => "ThreadError"
    }
  }
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match *self {
      Error::IoError(ref a) => write!(f,"Io error: {}",a),
      Error::ServerFilesMissing(ref a) => write!(f,"Server missing with id:{}",a),
      Error::ServerOffline(ref a) => write!(f,"Server offline with id:{}",a),
      Error::ServerAlreadyExists(ref a) => write!(f,"Server already exists with id:{}",a),
      Error::ThreadError(ref a) => write!(f,"Error while creating stdin/stdout threads for id:{}",a)
    }
  }
}

impl From<std::io::Error> for Error{
  fn from(e:std::io::Error) -> Self{
    Error::IoError(e)
  }
}

/// Controls the creation and deleting of servers, and whether they are currently active.
pub struct Manager {
  servers: HashMap<String, Instance>,
  server_files_folder: String,
  version_folder: String,
  jar_name: String
}

impl Manager {
  /// Creates a new server manager
  /// # Arguments
  /// * `server_files_folder` - the folder that will hold each server's folder, which contains its server files.
  /// * `version_folder` - the folder containing the base files of servers for the MC versions that you wish to host. Used as a base to create new servers.
  /// # Examples
  /// ```
  ///   let manager = serbo::Manager::new("folder1","folder2");
  /// ```
  /// # Remarks
  /// The version_folder should be a folder that contains folders that are named the same as the MC server files they contain.
  pub fn new(server_files_folder: &str, version_folder: &str, jar_name:&str) -> Manager {
    Manager {
      servers: HashMap::new(),
      server_files_folder: server_files_folder.to_string(),
      version_folder: version_folder.to_string(),
      jar_name: jar_name.to_string()
    }
  }
  /// Creates a new MC server folder under the `server_files_folder`
  /// # Arguments
  /// * `id` - The id for the server
  /// * `version` - The target version for the server.
  /// * `jar_name` - The name of the jar file that should be executed to start the server.
  /// # Examples
  /// ```
  /// let manager = serbo::Manager::new("folder1","folder2");
  /// manager.create("1","1.16.1");
  /// ```
  /// # Remarks
  /// Returns a result that contains the id that was assigned to the server
  pub fn create(&mut self, id: &str, version: &str) -> Result<()> {
    if self.exists(id){
      return Err(Error::ServerAlreadyExists(id.to_string()));
    }
    let target_folder = format!("{}/{}", self.server_files_folder, id);
    let base_folder = format!("{}/{}", self.version_folder, version);
    copy_dir(base_folder, target_folder)?;
    Ok(())
  }
  /// Returns an Option<t> containing a [Instance](struct.Instance.html) that represents the currently online server represented by the provided id
  /// # Arguments
  /// * `id` - The id that represents the requested server
  /// # Examples
  /// ```
  /// let manager = serbo::Manager::new("folder1","folder2");
  /// //Returns an Option
  /// let instance:serbo::Instance = manager.get("1").unwrap();
  /// ```
  /// # Remarks
  /// Queries the currently online servers, for get to return, must have been launched by calling [start](struct.Manager.html#method.start)
  pub fn get(&mut self, id: &str) -> Option<&mut Instance> {
    self.servers.get_mut(id)
  }
  /// Checks if server files exist for a given id
  /// # Arguments
  /// * `id` - The id that represents the requested server
  pub fn exists(&mut self, id: &str) -> bool {
    Path::new(&format!("{}/{}", self.server_files_folder, id)).exists()
  }
  /// Checks if the server is online
  /// # Arguments
  /// * `id` - The id that represents the requested server
  /// # Remarks
  /// Queries the currently online servers, must have been launched by calling [start](struct.Manager.html#method.start)
  pub fn is_online(&mut self, id: &str) -> bool {
    match self.get(id) {
      Some(_) => true,
      None => false,
    }
  }
  /// Launches a server
  /// # Arguments
  /// * `id` - The id that represents the requested server
  /// * `port` - The port that the server should be started on
  pub fn start(&mut self, id: &str, port: u32) -> Result<u32> {
    
    if !self.exists(id){
      return Err(Error::ServerFilesMissing(id.to_string()));
    }
    
    let mut command = Command::new("java");
    command
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .args(&[
        "-Xmx1024M",
        "-Xms1024M",
        "-jar",
        &self.jar_name,
        "nogui",
        "--port",
        &port.to_string(),
      ])
      .current_dir(format!("{}/{}", self.server_files_folder, id.to_string()));
    let child = command.spawn()?;
    let mut serv_inst = Instance {
      server_process: child,
      stdout_join: None,
      stdin_join: None,
      console_log: Arc::new(Mutex::new(Vec::new())),
      stdin_queue: Arc::new(Mutex::new(Vec::new())),
      port: port,
      id: id.to_string()
    };
    let stdout = match serv_inst.server_process.stdout.take(){
      Some(e) => e,
      None => return Err(Error::ThreadError(id.to_string()))
    };
    let stdin = match serv_inst.server_process.stdin.take(){
      Some(e) => e,
      None => return Err(Error::ThreadError(id.to_string()))
    };
    let stdout_arc = serv_inst.console_log.clone();
    let stdin_arc = serv_inst.stdin_queue.clone();
    let stdout_thread_handle = thread::spawn(move || {
      let reader = BufReader::new(stdout).lines();
      reader.filter_map(|line| line.ok()).for_each(|line| {
        let mut lock = stdout_arc.lock().unwrap();
        lock.push(line);
      });
    });
    let stdin_thread_handle = thread::spawn(move || {
      let mut writer = BufWriter::new(stdin);
      loop {
        let mut vec = stdin_arc.lock().unwrap();
        vec.drain(..).for_each(|x| {
          writeln!(writer, "{}", x);
          writer.flush();
        });
        drop(vec);
        sleep(time::Duration::from_secs(2));
      }
    });
    serv_inst.stdin_join = Some(stdin_thread_handle);
    serv_inst.stdout_join = Some(stdout_thread_handle);
    &self.servers.insert(id.to_string(), serv_inst);
    Ok(port)
  }
  /// Stops a server
  /// # Arguments
  /// * `id` - The id that represents the requested server
  pub fn stop(&mut self, id: &str) -> Result<()> {
    let serv = self.servers.get_mut(id);
    if let Some(inst) = serv {
      inst.stop()?;
      inst.stdout_join.take().unwrap().join();
      inst.stdin_join.take().unwrap().join();
      return Ok(());
    }
    Err(Error::ServerOffline(id.to_string()))
  }
  /// Deletes a server's files
  /// # Arguments
  /// * `id` - The id that represents the requested server
  /// # Remarks
  /// Stops the server if it's currently running
  pub fn delete(&mut self, id: &str) -> Result<()> {
    let _ = self.stop(id);
    if !self.exists(id){
      return Err(Error::ServerFilesMissing(id.to_string()));
    }
    fs::remove_dir_all(format!("{}/{}", &self.server_files_folder, id))?;
    Ok(())
  }
  /// Changes a server's version
  /// # Arguments
  /// * `id` - The id that represents the requested server
  /// * `target` - The target version to be switched to
  /// # Remarks
  /// Stops the server if it's currently running
  pub fn change_version(&mut self, id: &str, target: &str) -> Result<()> {
    let _ = self.stop(id);
    if !self.exists(id){
      return Err(Error::ServerFilesMissing(id.to_string()));
    }
    fs::remove_file(format!("{}/{}/server.jar", self.server_files_folder, id))?;
    fs::copy(
      format!("{}/{}/server.jar", self.version_folder, target),
      format!("{}/{}/server.jar", self.server_files_folder, id),
    )?;
    Ok(())
  }
}

/// Represents a currently online server.
/// Created by calling [start](struct.Manager.html#method.start) from a [Manager](struct.Manager.html)
pub struct Instance {
  pub server_process: Child,
  stdout_join: Option<thread::JoinHandle<()>>,
  stdin_join: Option<thread::JoinHandle<()>>,
  console_log: Arc<Mutex<Vec<String>>>,
  stdin_queue: Arc<Mutex<Vec<String>>>,
  pub port: u32,
  pub id: String
}

impl Instance {
  /// Stops the server, killing the server process and the stdin
  /// and stdout threads
  pub fn stop(&mut self) -> Result<()> {
    self.server_process.kill()?;
    Ok(())
  }
  /// Sends a string to the server stdin
  /// # Arguments
  /// * `msg` - A String that contains the message to be sent to the server.
  ///
  /// # Remarks
  /// The message should not contain a trailing newline, as the send method handles it.
  pub fn send(&mut self, msg: String) -> Result<()> {
    let vec_lock = self.stdin_queue.clone();
    let mut vec = vec_lock.lock().unwrap();
    vec.push(msg);
    Ok(())
  }
  //// Gets the output from server stdout
  ///  # Arguments
  ///  * `start` The line number of the first line that should be returned
  ///
  /// # Remarks
  /// The returned Vec will contain the lines in the range of start to the end of output
  pub fn get(&self, start: u32) -> Vec<String> {
    let vec_lock = self.console_log.clone();
    let vec = vec_lock.lock().unwrap();
    let mut start_line = start as usize;
    if start_line > vec.len() {
      start_line = vec.len()
    }
    Vec::from(&vec[start_line..])
  }
}
