use std::{collections::HashMap, fs, io::{Error, Write}, str::FromStr};

use base64::Engine;
use sha1::Digest;


#[derive(Debug, PartialEq, Eq)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    OPTIONS,
    OTHER
}

impl FromStr for HttpMethod {
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("GET") {
            Ok(HttpMethod::GET)
        } else if s.starts_with("POST") {
            Ok(HttpMethod::POST)
        } else if s.starts_with("PUT") {
            Ok(HttpMethod::PUT)
        } else if s.starts_with("DELETE") {
            Ok(HttpMethod::DELETE)
        } else if s.starts_with("OPTIONS") {
            Ok(HttpMethod::OPTIONS)
        } else {
            return Err("wtf this is not a http method");
        }
    }
    
    type Err = &'static str;
}

#[derive(Debug)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub version: String,
    pub path: String,
    pub headers: HashMap<String, String>,
}

impl HttpRequest {
    pub fn is_websocket_handshake(&self) -> bool {
        match self.headers.get("Upgrade") {
            Some(s) => s.eq("websocket"),
            None => false
        }
    }
}

pub struct HttpResponse {
    protocol_version: String,
    status_code: u16,
    status_msg: String,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
}

impl HttpResponse {
    fn default_headers() -> HashMap<String, String> {
        let mut ret = HashMap::<String, String>::new();
        ret.insert("Server".to_string(), "rust-001".to_string());
        ret.insert("Cross-Origin-Opener-Policy".to_string(), "same-origin".to_string());
        ret.insert("Cross-Origin-Embedder-Policy".to_string(), "require-corp".to_string());
        
        return ret;
    }
    fn with_content_length(mut self, size: usize) -> Self {
        self.headers.remove("content-length");
        self.headers.insert("content-length".to_string(), size.to_string());
        return self;
    }
    fn with_content_type(mut self, content_type: &str) -> Self {
        self.headers.remove("content-type");
        self.headers.insert("content-type".to_string(), content_type.to_string());
        return self;
    }
    pub fn not_found() -> HttpResponse {
        HttpResponse {
            protocol_version: "HTTP/1.1".to_string(),
            status_code: 404,
            status_msg: "No shit".to_string(),
            headers: Self::default_headers(),
            body: None,
        }.with_content_length(0)
    }
    pub fn file_content(filepath: &str) -> HttpResponse {
        match fs::read(filepath) {
            Err(err) => {
                println!("error reading file '{}': {}", filepath, err);
                Self::not_found()
            },
            Ok(payload) => {
                let len = payload.len();
                let file_extension = filepath.split('.').last();
                HttpResponse {
                    protocol_version: "HTTP/1.1".to_string(),
                    status_code: 200,
                    status_msg: "Take this".to_string(),
                    headers: Self::default_headers(),
                    body: Some(payload),
                }
                .with_content_length(len)
                .with_content_type(
                    match file_extension {
                        Some(ext) if ext.to_ascii_lowercase().eq("js") => {
                            "text/javascript"
                        },
                        Some(ext) if ext.to_ascii_lowercase().eq("html") => {
                            "text/html"
                        },
                        Some(ext) if ext.to_ascii_lowercase().eq("css") => {
                            "text/css"
                        },
                        _ => "application/octet-stream"
                    }
                )
            }
        }
    }

    pub fn websocket_handshake(req:&HttpRequest) -> HttpResponse {
        let key = req.headers.get("Sec-WebSocket-Key").unwrap();

        let mut hasher = sha1::Sha1::new();
        let fullstring = format!("{}{}", key, "258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
        let accept_key = match hasher.write(fullstring.as_bytes()) {
            Ok(_result) => {
                let finished = hasher.finalize();
                let ret = base64::engine::general_purpose::STANDARD.encode(finished);
                ret
            },
            Err(err) => {
                println!("Error writing hash: {}", err);
                "".to_string()
            }
        };
        let mut headers : HashMap<String, String> = HashMap::new();

        headers.insert("Sec-Websocket-Accept".to_string(), accept_key);
        headers.insert("Connection".to_string(), "upgrade".to_string());
        headers.insert("Upgrade".to_string(), "websocket".to_string());


        HttpResponse {
          protocol_version: "HTTP/1.1".to_string(),
          status_code: 101,
          status_msg: "Lets gooo".to_string(),
          headers: headers,
          body: None, 
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let status_line = format!(
            "{} {} {}", 
            self.protocol_version, 
            self.status_code, 
            self.status_msg
        );
        
        let headers_str = self.headers.iter()
            .map(|(key, value)| format!("{}: {}", key, value))
            .collect::<Vec<String>>()
            .join("\r\n");
        
        let http_header = format!("{}\r\n{}\r\n\r\n", status_line, headers_str);
        
        let mut result = http_header.into_bytes();
        
        if let Some(body) = &self.body {
            result.extend_from_slice(body);
        }
        
        result
    }

}

// Just websocket parsing & stringfier
pub struct WebSocketFrame;

impl WebSocketFrame {

    pub fn to_websocket(payload: Vec<u8>) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.push(0x81);
        
        let payload_len = payload.len();
        
        if payload_len <= 125 {
            frame.push(payload_len as u8);
        } else if payload_len <= 65535 {
            frame.push(126);
            frame.extend_from_slice(&(payload_len as u16).to_be_bytes());
        } else {
            frame.push(127);
            frame.extend_from_slice(&(payload_len as u64).to_be_bytes());
        }
        
        frame.extend(payload); // ✅ Move sem clone
        frame
    }

    pub fn parse(data: &mut Vec<u8>) -> Result<Vec<u8>, Error> {
        if data.len() < 2 {
            return Result::Err(Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid websocket frame",                                                   
            ));
        }
        if (data[0] & 0x01) == 0 {
            return Result::Err(Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "Can't handle multiframe payloads yet!!!",
            ));
        }
        let mut payload_start = 2;
        let masking_bit = data[1] >> 7;
        let mut mask : u32 = 0xFFFF;
        let mut payload_len : usize = (data[1] & 0x7F).into();
        if payload_len == 126 {
            // gotta read next 2 bytes
            payload_len = u16::from_be_bytes([data[2], data[3]]).into();
            payload_start += 2;
        } else if payload_len == 127 {
            // gotta read next 8 bytes
            payload_len = u64::from_be_bytes(data[2..=9].try_into().unwrap()).try_into().unwrap();
            payload_start +=8;
        }
        if masking_bit == 1 {
            mask = u32::from_be_bytes(data[payload_start..(payload_start+4)].try_into().unwrap());
            payload_start += 4;
        }

        if data.len() < (payload_start + payload_len) {
            return Result::Err(Error::new(
                std::io::ErrorKind::Interrupted,
                "Data not fully arrived yet",
            ));
        }
        
        let mut payloadvec : Vec<u8> = data.drain(0..payload_start+payload_len).collect();
        payloadvec.drain(0..payload_start);

        if masking_bit > 0 {
            payloadvec = payloadvec.iter().enumerate().map(
                |(index, byte)| (byte) ^ (mask.to_be_bytes())[index % 4]
            ).collect();
        }

        return Ok(payloadvec.to_vec());
    }
}
