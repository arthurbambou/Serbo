use serbo;
use std::io::{self};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>>{
  let mut manager = serbo::Manager::new("servers", "versions");
  let port = 25565;
  let id = "1";
  if !manager.exists(id){
    manager.create(id, "1.16.1-fabric")?;
  }
  loop {
    let reader = io::stdin();
    let mut buf = String::new();
    println!("Enter your command.");
    reader.read_line(&mut buf)?;
    match buf.trim() {
      "change_version" => {
        let mut send_buf = String::new();
        println!("Enter the version to change to.");
        reader.read_line(&mut send_buf)?;
        //Remove the newline from read_line
        send_buf = send_buf[..send_buf.chars().count()-1].to_string();
        manager.change_version(id,&send_buf)?;
      },
      "stop" => {
        //Stops the server
        println!("Server stopping.");
        manager.stop(id)?;
        break Ok(());
      }
      "start" => {
        //Starts the server
        println!("Server starting.");
        manager.start(id, port)?;
      }
      "send" => {
        //Prompts for a command to send to the server
        let mut send_buf = String::new();
        println!("Enter the command to send to the server.");
        reader.read_line(&mut send_buf)?;
        //Remove the newline from read_line
        send_buf = send_buf[..send_buf.chars().count()-1].to_string();
        let instance = manager.get(id).unwrap();
        instance.send(send_buf);
      },
      "get" => {
        //Gets the last 5 stdout lines
        let instance:&serbo::Instance = manager.get(id).unwrap();
        let vec = instance.get(0);
        let length = vec.len();
        //Create a vec from the last 5 lines
        let trimmed_vec;
        if length >= 5{
         trimmed_vec = Vec::from(&vec[length-5..]);
        }
        else{
          trimmed_vec = Vec::from(vec);
        }
        for line in trimmed_vec{
          println!("{}",line);
        }
      },
      _ => {
        println!("Unrecognized command");
      }
    }
  }
}

