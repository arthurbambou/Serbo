//! Allows for simple control, and input / output of minecraft servers.
//!
//! # Examples
//! ```
//!use serbo;
//!use std::error::Error;
//!use std::io;
//!
//!fn main() -> Result<(), Box<dyn Error>> {
//!  let mut manager = serbo::Manager::new();
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
//!      "create" => match manager.create() {
//!        Ok(_) => println!("Server Created"),
//!        Err(e) => println!("{}", e),
//!      },
//!      "stop" => {
//!        //Stops the server
//!        println!("Server stopping.");
//!        manager.stop()?;
//!      }
//!      "start" => {
//!        //Starts the server
//!        println!("Server starting.");
//!        match manager.start(port) {
//!          Err(e) => println!("{}", e),
//!          Ok(_) => println!("Server started!"),
//!        };
//!      }
//!      "send" => {
//!        //Prompts for a command to send to the server
//!        if let Some(instance) = manager.get(){
//!          let mut send_buf = String::new();
//!          println!("Enter the command to send to the server.");
//!          reader.read_line(&mut send_buf)?;
//!          //Remove the newline from read_line
//!          send_buf = send_buf[..send_buf.chars().count() - 1].to_string();
//!          match instance.send(send_buf){
//!            Ok(()) => println!("Command sent."),
//!            Err(a) => {
//!              println!("{}",a);
//!            }
//!          };
//!        }
//!        else{
//!          println!("Server Offline.");
//!        }
//!      }
//!      "get" => {
//!        //Gets the last 5 stdout lines
//!        if let Some(instance) = manager.get(){
//!          let vec = instance.get(0);
//!          let length = vec.len();
//!          //Create a vec from the last 5 lines
//!          let trimmed_vec;
//!          if length >= 10 {
//!            trimmed_vec = Vec::from(&vec[length - 10..]);
//!          } else {
//!            trimmed_vec = Vec::from(vec);
//!          }
//!          for line in trimmed_vec {
//!            println!("{}", line);
//!          }
//!        }
//!        else {
//!          println!("Server Offline.")
//!        }
//!      }
//!      _ => {
//!        println!("Unrecognized command");
//!      }
//!    }
//!  }
//!}
//! ```

use std::fmt;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
  /// Arises when there is an error regarding IO
  IoError(std::io::Error),
  /// Arises when an offline server is attempted to be used
  ServerOffline(),
  /// Arises when attempting to start a server that is already online
  ServerAlreadyOnline(),
  /// Arises when a server's files are missing
  ServerFilesMissing(),
  /// Arises when attempting to create a server with the same id as an existing server
  ServerAlreadyExists(),
  /// Arises when there is an error involving a server's stdin/stdout threads
  ThreadError(String),
  /// Arises when the server processes needs to be referenced, but has unexpectedly ended.
  /// May occur due to the server process being killed, the server crashing or ingame methods
  /// to stop the server
  ServerProcessExited(),
  ServerStillStarting()
}

impl std::error::Error for Error {
  fn description(&self) -> &str {
    match *self {
      Error::IoError(_) => "IOError",
      Error::ServerFilesMissing() => "MissingServer",
      Error::ServerOffline() => "ServerOffline",
      Error::ServerAlreadyExists() => "ServerAlreadyExists",
      Error::ThreadError(_) => "ThreadError",
      Error::ServerProcessExited() => "ServerProcessExited",
      Error::ServerAlreadyOnline() => "ServerAlreadyOnline",
      Error::ServerStillStarting() => "ServerStillStarting"
    }
  }
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match *self {
      Error::IoError(ref a) => write!(f, "Io error: {}", a),
      Error::ServerFilesMissing() => write!(f, "Server files not found"),
      Error::ServerOffline() => write!(f, "Server offline while called."),
      Error::ServerAlreadyExists() => write!(f, "Server files already exists"),
      Error::ThreadError(ref a) => write!(f, "Error while creating {} thread for server", a),
      Error::ServerProcessExited() => write!(f,"Server processes needed, but has unexpectedly exited."),
      Error::ServerAlreadyOnline() => write!(f, "Attempted to start already online server"),
      Error::ServerStillStarting() => write!(f, "Attempted to stop a server that's mid-loading")
    }
  }
}

impl From<std::io::Error> for Error {
  fn from(e: std::io::Error) -> Self {
    Error::IoError(e)
  }
}

/// Controls the creation and deleting of servers, and whether they are currently active.
pub struct Manager {
  server: Option<Instance>,
}

impl Manager {
  /// Creates a new server manager
  /// # Arguments
  /// * `server_files_folder` - the folder that will hold each server's folder, which contains its server files.
  /// * `version_folder` - the folder containing the base files of servers for the MC versions that you wish to host. Used as a base to create new servers.
  /// # Examples
  /// ```
  ///   let manager = serbo::Manager::new();
  /// ```
  /// # Remarks
  /// The version_folder should be a folder that contains folders that are named the same as the MC server files they contain.
  pub fn new() -> Manager {
    Manager {
      server: None,
    }
  }
  /// Returns an Option<t> containing a [Instance](struct.Instance.html) that represents the currently online server represented by the provided id
  /// # Arguments
  /// * `id` - The id that represents the requested server
  /// # Examples
  /// ```
  /// let mut manager = serbo::Manager::new();
  /// //Returns an Option
  /// let instance = manager.get().unwrap();
  /// ```
  /// # Remarks
  /// Queries the currently online servers, for get to return, must have been launched by calling [start](struct.Manager.html#method.start)
  pub fn get(&mut self) -> Option<&mut Instance> {
    if let Some(ref mut server) = self.server {
      if let Ok(bol) = server.is_valid() {
        if bol {
          Some(server)
        } else {
          None
        }
      } else {
        None
      }
    } else {
      None
    }
  }
  /// Checks if server files exist for a given id
  /// # Arguments
  /// * `id` - The id that represents the requested server
  pub fn exists(&mut self) -> bool {
    Path::new(&format!("./server")).exists()
  }
  /// Checks if the server is online
  /// # Arguments
  /// * `id` - The id that represents the requested server
  /// # Remarks
  /// Queries the currently online servers, must have been launched by calling [start](struct.Manager.html#method.start)
  pub fn is_online(&mut self) -> bool {
    match self.get() {
      Some(_) => true,
      None => false,
    }
  }
  /// Launches a server
  /// # Arguments
  /// * `id` - The id that represents the requested server
  /// * `port` - The port that the server should be started on
  pub fn start(&mut self, port: u32) -> Result<u32> {
    if let Some(_) = self.server {
      Err(Error::ServerAlreadyOnline())
    } else {
      let mut command = Command::new("java");
      command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(&[
          "-Xmx4G",
          "-Xms1G",
          "-jar",
          "server.jar",
          "nogui",
          "--port",
          &port.to_string(),
        ])
        .current_dir(format!("./server"));
      let child = command.spawn()?;
      let mut serv_inst = Instance {
        server_process: child,
        stdout_join: None,
        stdin_join: None,
        console_log: Arc::new(Mutex::new(Vec::new())),
        stdin_queue: Arc::new(Mutex::new(Vec::new())),
        thread_cond: Arc::new(RwLock::new(true)),
        starting: Arc::new(RwLock::new(true)),
        port
      };
      let stdout = match serv_inst.server_process.stdout.take() {
        Some(e) => e,
        None => return Err(Error::ThreadError("stdout".to_string())),
      };
      let stdin = match serv_inst.server_process.stdin.take() {
        Some(e) => e,
        None => return Err(Error::ThreadError("stdin".to_string())),
      };

      let starting_lock = serv_inst.starting.clone();
      let stdout_arc = serv_inst.console_log.clone();
      let stdin_arc = serv_inst.stdin_queue.clone();
      let cond_reader1 = serv_inst.thread_cond.clone();
      let cond_reader2 = serv_inst.thread_cond.clone();

      let stdout_thread_handle = thread::spawn(move || {
        let mut reader = BufReader::new(stdout).lines();
        loop {
          let r1 = cond_reader1.read().unwrap();
          if !*r1{
            break;
          }
          drop(r1);
          if let Some(line) = reader.next() {
            match line {
              Ok(a) => {
                if a.len() >= 33  {
                  let b = &a[33..];
                  if b == "[Server] SERVER READY"{
                    println!("READY");
                    let mut g = starting_lock.write().unwrap();
                    *g = false;
                  }

                  let mut lock = stdout_arc.lock().unwrap();
                  lock.push(a);
                }
              },
              _ => {}
            };
          }
        }
      });

      let stdin_thread_handle = thread::spawn(move || {
        let mut writer = BufWriter::new(stdin);
        loop {
          let mut vec = stdin_arc.lock().unwrap();
          let r1 = cond_reader2.read().unwrap();
          if !*r1 && vec.len() == 0{
            break;
          }
          drop(r1);
          vec.drain(..).for_each(|x| {
            writeln!(writer, "{}", x);
            writer.flush();
          });
          drop(vec);
        }
      });
      serv_inst.send("/say SERVER READY".to_string())?;
      serv_inst.send("say SERVER READY".to_string())?;
      serv_inst.stdout_join = Some(stdout_thread_handle);
      serv_inst.stdin_join = Some(stdin_thread_handle);
      self.server.insert(serv_inst);
      Ok(port)
    }
  }
  /// Stops a server
  /// # Arguments
  /// * `id` - The id that represents the requested server
  pub fn stop(&mut self) -> Result<()> {
    if let Some(ref mut inst) = self.server {
      let is_starting = *inst.starting.read().unwrap();
      if !is_starting{
        inst.stop()?;
        let rw = inst.thread_cond.clone();
        let mut d = rw.write().unwrap();
        *d = false;
        drop(d);
        drop(rw);
        inst.stdout_join.take().unwrap().join();
        inst.stdin_join.take().unwrap().join();
        inst.server_process.wait();
        self.server.take();
        return Ok(());
      }
      return Err(Error::ServerStillStarting());
    }
    Err(Error::ServerOffline())
  }
}

/// Represents a currently online server.
/// Created by calling [start](struct.Manager.html#method.start) from a [Manager](struct.Manager.html)
#[derive(Debug)]
pub struct Instance {
  pub server_process: Child,
  stdout_join: Option<thread::JoinHandle<()>>,
  stdin_join: Option<thread::JoinHandle<()>>,
  console_log: Arc<Mutex<Vec<String>>>,
  stdin_queue: Arc<Mutex<Vec<String>>>,
  thread_cond: Arc<RwLock<bool>>,
  starting: Arc<RwLock<bool>>,
  pub port: u32,
}

impl Instance {
  /// Stops the server, killing the server process and the stdin
  /// and stdout threads
  pub fn stop(&mut self) -> Result<()> {
    let _ = self.process_check();
    self.send(String::from("/stop"))?;
    Ok(())
  }
  /// Checks if the server process is still valid (has not crashed or exited).
  pub fn is_valid(&mut self) -> Result<bool> {
    match self.server_process.try_wait()? {
      Some(_) => Ok(false),
      None => Ok(true),
    }
  }
  fn process_check(&mut self) -> Result<()> {
    match self.is_valid()? {
      true => Ok(()),
      false => Err(Error::ServerProcessExited()),
    }
  }
  /// Sends a string to the server stdin
  /// # Arguments
  /// * `msg` - A String that contains the message to be sent to the server.
  ///
  /// # Remarks
  /// The message should not contain a trailing newline, as the send method handles it.
  pub fn send(&mut self, msg: String) -> Result<()> {
    let _ = self.process_check()?;
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
