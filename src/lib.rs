use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::{BufRead, BufReader, BufWriter, Error, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::sleep;
use std::time;

pub struct Manager {
  servers: HashMap<String, Instance>,
  folder: String,
  version_folder: String,
}

impl Manager {
  pub fn new(folder: String, version_folder: String) -> Manager {
    Manager {
      servers: HashMap::new(),
      folder: folder,
      version_folder: version_folder,
    }
  }
  pub fn create(&self, version: String) -> io::Result<u32> {
    let id = rand::random::<u32>();
    fs::create_dir(format!("{}/{}", self.folder, id))?;
    fs::copy(
      format!("server/{}/eula.txt", version),
      format!("{}/{}/eula.txt", self.folder, id),
    )?;
    fs::copy(
      format!("server/{}/eula.txt", version),
      format!("{}/{}/server.jar",self.folder, id),
    )?;
    Ok(id)
  }
  pub fn get(&mut self, id: String) -> Option<&mut Instance> {
    self.servers.get_mut(&id)
  }
  pub fn start(&mut self, id: String) -> Result<u32, Error> {
    let mut rng = thread_rng();
    let port = rng.gen_range(25565, 35565);
    let mut command = Command::new("java");
    command
      .stdin(Stdio::piped())
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
      .current_dir(format!("{}/{}", self.folder, id.to_string()));
    let child = command.spawn()?;
    let mut serv_inst = Instance {
      server_process: child,
      stdout_join: None,
      stdin_join: None,
      console_log: Arc::new(Mutex::new(Vec::new())),
      stdin_queue: Arc::new(Mutex::new(Vec::new())),
      port: port,
    };
    let stdout = serv_inst.server_process.stdout.take().unwrap();
    let stdin = serv_inst.server_process.stdin.take().unwrap();
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
    &self.servers.insert(id, serv_inst);
    Ok(port)
  }
  pub fn stop(&mut self, id: String) -> io::Result<()> {
    let serv = self.servers.get_mut(&id);
    if let Some(inst) = serv{
      inst.stop()?;
      inst.stdout_join.take().unwrap().join();
      inst.stdin_join.take().unwrap().join();
    }
    Ok(())
  }
  pub fn delete(&mut self, id: String) -> io::Result<()> {
    self.stop(id.clone())?;
    fs::remove_dir_all(format!("{}/{}", &self.folder, id))?;
    Ok(())
  }
  pub fn change_version(&mut self, id: String, target: String) -> Result<(), Error> {
    self.stop(id.clone())?;
    fs::remove_file(format!("{}/{}/server.jar", self.folder, id))?;
    fs::copy(
      format!("{}/{}/server.jar", self.version_folder, target),
      format!("{}/{}/server.jar", self.folder, id),
    )?;
    Ok(())
  }
}

pub struct Instance {
  pub server_process: Child,
  stdout_join: Option<thread::JoinHandle<()>>,
  stdin_join: Option<thread::JoinHandle<()>>,
  console_log: Arc<Mutex<Vec<String>>>,
  stdin_queue: Arc<Mutex<Vec<String>>>,
  pub port: u32,
}

impl Instance {
  pub fn stop(&mut self) -> io::Result<()> {
    self.server_process.kill()?;
    Ok(())
  }
  pub fn send(&mut self, msg: String) {
    let vec_lock = self.stdin_queue.clone();
    let mut vec = vec_lock.lock().unwrap();
    vec.push(msg);
  }
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
