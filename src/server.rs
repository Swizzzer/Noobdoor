mod shared;

use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use shared::*;


fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("0.0.0.0:4444")?;
    println!("Listening on port 4444...");
    
    let client = Arc::new(Mutex::new(None::<CryptoChannel>));
    let client_clone = client.clone();
    
    // 命令界面线程
    thread::spawn(move || {
        command_interface(client_clone);
    });
    
    // 主线程接受连接
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("收到新连接: {}", stream.peer_addr().unwrap());
                
                let mut crypto = CryptoChannel::new(stream);
                if let Err(e) = crypto.perform_key_exchange(true) {
                    println!("密钥交换失败: {}", e);
                    continue;
                }
                
                println!("密钥交换成功，建立新的加密连接");
                
                *client.lock().unwrap() = Some(crypto);
                
                // 心跳线程
                let client_heartbeat = client.clone();
                thread::spawn(move || {
                    heartbeat_loop(client_heartbeat);
                });
            }
            Err(e) => {
                println!("连接错误: {}", e);
            }
        }
    }
    
    Ok(())
}

fn command_interface(client: Arc<Mutex<Option<CryptoChannel>>>) {
    loop {
        println!("\n=== 请选择操作 ===");
        println!("1. 检查连接状态");
        println!("2. 执行系统命令");
        println!("3. 读取文件");
        println!("4. 上传文件");
        println!("5. 断开连接");
        println!("0. 退出程序");
        
        print!("请选择操作 [0-5]: ");
        io::stdout().flush().unwrap();
        
        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();
        
        match choice.trim() {
            "0" => {
                println!("退出程序");
                std::process::exit(0);
            },
            "1" => check_connection(&client),
            "2" => execute_remote_command(&client),
            "3" => read_remote_file(&client),
            "4" => upload_remote_file(&client),
            "5" => disconnect_client(&client),
            _ => println!("无效选择，请重试"),
        }
    }
}

fn check_connection(client: &Arc<Mutex<Option<CryptoChannel>>>) {
    let mut client_guard = client.lock().unwrap();
    
    if let Some(ref mut crypto) = *client_guard {
        println!("正在检查连接...");
        
        match crypto.send_command(&Command::Heartbeat) {
            Ok(_) => {
                match crypto.receive_response() {
                    Ok(Response::HeartbeatResponse) => {
                        println!("连接正常，客户端响应心跳");
                    },
                    Ok(_) => println!("收到异常响应"),
                    Err(e) => {
                        println!("连接异常: {}", e);
                        *client_guard = None;
                    }
                }
            },
            Err(e) => {
                println!("连接异常: {}", e);
                *client_guard = None;
            }
        }
    } else {
        println!("当前没有活跃连接");
    }
}

fn execute_remote_command(client: &Arc<Mutex<Option<CryptoChannel>>>) {
    let mut client_guard = client.lock().unwrap();
    
    if let Some(ref mut crypto) = *client_guard {
        print!("输入要执行的命令: ");
        io::stdout().flush().unwrap();
        
        let mut command = String::new();
        io::stdin().read_line(&mut command).unwrap();
        command = command.trim().to_string();
        
        if command.is_empty() {
            println!("命令不能为空");
            return;
        }
        
        println!("执行命令: {}", command);
        
        match crypto.send_command(&Command::ExecuteCommand { command }) {
            Ok(_) => {
                match crypto.receive_response() {
                    Ok(Response::CommandOutput { output }) => {
                        println!("命令输出:\n{}", output);
                    },
                    Ok(Response::Error { message }) => {
                        println!("执行错误: {}", message);
                    },
                    Ok(_) => println!("收到异常响应"),
                    Err(e) => {
                        println!("连接异常: {}", e);
                        *client_guard = None;
                    }
                }
            },
            Err(e) => {
                println!("连接异常: {}", e);
                *client_guard = None;
            }
        }
    } else {
        println!("当前没有活跃连接");
    }
}

fn read_remote_file(client: &Arc<Mutex<Option<CryptoChannel>>>) {
    let mut client_guard = client.lock().unwrap();
    
    if let Some(ref mut crypto) = *client_guard {
        print!("输入要读取的文件路径: ");
        io::stdout().flush().unwrap();
        
        let mut path = String::new();
        io::stdin().read_line(&mut path).unwrap();
        path = path.trim().to_string();
        
        if path.is_empty() {
            println!("路径不能为空");
            return;
        }
        
        println!("读取文件: {}", path);
        
        match crypto.send_command(&Command::ReadFile { path }) {
            Ok(_) => {
                match crypto.receive_response() {
                    Ok(Response::FileContent { content }) => {
                        print!("文件内容保存到哪里? [留空显示内容]: ");
                        io::stdout().flush().unwrap();
                        
                        let mut save_path = String::new();
                        io::stdin().read_line(&mut save_path).unwrap();
                        save_path = save_path.trim().to_string();
                        
                        if save_path.is_empty() {
                            if let Ok(text) = String::from_utf8(content.clone()) {
                                println!("文件内容 (UTF-8):\n{}", text);
                            } else {
                                println!("文件内容 (二进制): {} 字节", content.len());
                            }
                        } else {
                            match fs::write(&save_path, content) {
                                Ok(_) => println!("文件已保存到 {}", save_path),
                                Err(e) => println!("保存文件失败: {}", e),
                            }
                        }
                    },
                    Ok(Response::Error { message }) => {
                        println!("读取错误: {}", message);
                    },
                    Ok(_) => println!("收到异常响应"),
                    Err(e) => {
                        println!("连接异常: {}", e);
                        *client_guard = None;
                    }
                }
            },
            Err(e) => {
                println!("连接异常: {}", e);
                *client_guard = None;
            }
        }
    } else {
        println!("当前没有活跃连接");
    }
}

fn upload_remote_file(client: &Arc<Mutex<Option<CryptoChannel>>>) {
    let mut client_guard = client.lock().unwrap();
    
    if let Some(ref mut crypto) = *client_guard {
        print!("输入本地文件路径: ");
        io::stdout().flush().unwrap();
        
        let mut local_path = String::new();
        io::stdin().read_line(&mut local_path).unwrap();
        local_path = local_path.trim().to_string();
        
        if local_path.is_empty() {
            println!("路径不能为空");
            return;
        }
        
        print!("输入远程保存路径: ");
        io::stdout().flush().unwrap();
        
        let mut remote_path = String::new();
        io::stdin().read_line(&mut remote_path).unwrap();
        remote_path = remote_path.trim().to_string();
        
        if remote_path.is_empty() {
            println!("路径不能为空");
            return;
        }
        
        println!("上传文件 {} 到 {}", local_path, remote_path);
        
        match fs::read(&local_path) {
            Ok(content) => {
                match crypto.send_command(&Command::UploadFile { 
                    path: remote_path,
                    content
                }) {
                    Ok(_) => {
                        match crypto.receive_response() {
                            Ok(Response::UploadResponse { success, message }) => {
                                if success {
                                    println!("上传成功: {}", message);
                                } else {
                                    println!("上传失败: {}", message);
                                }
                            },
                            Ok(_) => println!("收到异常响应"),
                            Err(e) => {
                                println!("连接异常: {}", e);
                                *client_guard = None;
                            }
                        }
                    },
                    Err(e) => {
                        println!("连接异常: {}", e);
                        *client_guard = None;
                    }
                }
            },
            Err(e) => {
                println!("读取本地文件失败: {}", e);
            }
        }
    } else {
        println!("当前没有活跃连接");
    }
}

fn disconnect_client(client: &Arc<Mutex<Option<CryptoChannel>>>) {
    let mut client_guard = client.lock().unwrap();
    
    if let Some(ref mut crypto) = *client_guard {
        println!("断开客户端连接...");
        
        if let Err(e) = crypto.send_command(&Command::Disconnect) {
            println!("发送断开命令失败: {}", e);
        }
        
        *client_guard = None;
        println!("连接已断开");
    } else {
        println!("当前没有活跃连接");
    }
}

fn heartbeat_loop(client: Arc<Mutex<Option<CryptoChannel>>>) {
    let mut last_heartbeat = Instant::now();
    
    loop {
        thread::sleep(Duration::from_secs(5));
        
        let now = Instant::now();
        if now.duration_since(last_heartbeat) >= Duration::from_secs(30) {
            let mut client_guard = client.lock().unwrap();
            
            if let Some(ref mut crypto) = *client_guard {
                println!("发送心跳...");
                
                match crypto.send_command(&Command::Heartbeat) {
                    Ok(_) => {
                        match crypto.receive_response() {
                            Ok(Response::HeartbeatResponse) => {
                                println!("心跳响应正常");
                                last_heartbeat = now;
                            },
                            Ok(_) => println!("收到异常心跳响应"),
                            Err(e) => {
                                println!("心跳通信错误: {}", e);
                                *client_guard = None;
                                break;
                            }
                        }
                    },
                    Err(e) => {
                        println!("心跳通信错误: {}", e);
                        *client_guard = None;
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }
}
