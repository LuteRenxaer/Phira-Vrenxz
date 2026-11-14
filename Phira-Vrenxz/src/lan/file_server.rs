//! 局域网文件下载服务端
//!
//! 房主作为文件服务器，提供谱面文件下载

use crate::lan::protocol::{LanMessage};
use anyhow::{Context, Result};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, info, warn};

/// 文件服务器
pub struct FileServer {
    /// TCP监听器
    listener: TcpListener,
    /// 谱面目录
    chart_dir: String,
    /// 当前正在服务的谱面
    current_chart: Arc<Mutex<Option<String>>>,
    /// 服务器运行标志
    running: Arc<Mutex<bool>>,
}

impl FileServer {
    /// 创建新的文件服务器
    pub fn new(port: u16, chart_dir: String) -> Result<Self> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
            .with_context(|| format!("Failed to bind file server on port {}", port))?;

        Ok(Self {
            listener,
            chart_dir,
            current_chart: Arc::new(Mutex::new(None)),
            running: Arc::new(Mutex::new(false)),
        })
    }

    /// 启动服务器
    pub fn start(&self) -> Result<()> {
        *self.running.lock().unwrap() = true;

        let listener = self.listener.try_clone()?;
        let chart_dir = self.chart_dir.clone();
        let current_chart = Arc::clone(&self.current_chart);
        let running = Arc::clone(&self.running);

        thread::spawn(move || {
            info!("File server started on port {}", listener.local_addr().unwrap().port());

            while *running.lock().unwrap() {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        let chart_dir = chart_dir.clone();
                        let current_chart = Arc::clone(&current_chart);

                        thread::spawn(move || {
                            if let Err(e) = Self::handle_client(stream, addr, &chart_dir, &current_chart) {
                                warn!("Failed to handle client {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Failed to accept connection: {}", e);
                    }
                }
            }

            info!("File server stopped");
        });

        Ok(())
    }

    /// 停止服务器
    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    /// 设置当前服务的谱面
    pub fn set_current_chart(&self, chart_id: String) {
        *self.current_chart.lock().unwrap() = Some(chart_id);
    }

    /// 当前监听端口
    pub fn port(&self) -> u16 {
        self.listener.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    /// 处理客户端连接
    fn handle_client(
        mut stream: TcpStream,
        addr: std::net::SocketAddr,
        chart_dir: &str,
        current_chart: &Arc<Mutex<Option<String>>>,
    ) -> Result<()> {
        let mut buffer = vec![0u8; 4096];
        
        // 读取请求
        match stream.read(&mut buffer) {
            Ok(0) => {
                info!("Client disconnected: {}", addr);
                return Ok(());
            }
            Ok(n) => {
                let request = String::from_utf8_lossy(&buffer[..n]);
                debug!("Request from {}: {}", addr, request);

                // 解析请求
                if let Some(chart_id) = Self::parse_request(&request) {
                    // 检查是否有当前谱面
                    let current = current_chart.lock().unwrap();
                    if current.as_ref().map_or(false, |id| id == &chart_id) {
                        // 发送谱面文件
                        Self::send_chart(&mut stream, chart_dir, &chart_id)?;
                    } else {
                        // 发送错误响应
                        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
                        stream.write_all(response.as_bytes())?;
                    }
                } else {
                    // 发送错误响应
                    let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
                    stream.write_all(response.as_bytes())?;
                }
            }
            Err(e) => {
                warn!("Failed to read from client {}: {}", addr, e);
                return Err(e.into());
            }
        }

        Ok(())
    }

    /// 解析请求
    fn parse_request(request: &str) -> Option<String> {
        // 简化的请求解析：假设请求格式为 "GET /download/{chart_id} HTTP/1.1"
        let lines: Vec<&str> = request.lines().collect();
        if lines.is_empty() {
            return None;
        }

        let parts: Vec<&str> = lines[0].split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }

        let path = parts[1];
        let path_parts: Vec<&str> = path.split('/').collect();
        
        if path_parts.len() >= 3 && path_parts[1] == "download" {
            Some(path_parts[2].to_string())
        } else {
            None
        }
    }

    /// 发送谱面文件
    fn send_chart(stream: &mut TcpStream, chart_dir: &str, chart_id: &str) -> Result<()> {
        let chart_path = format!("{}/download/{}", chart_dir, chart_id);
        let chart_path = Path::new(&chart_path);

        if !chart_path.exists() {
            warn!("Chart not found: {}", chart_path.display());
            let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
            stream.write_all(response.as_bytes())?;
            return Ok(());
        }

        // 检查是否为目录
        if !chart_path.is_dir() {
            warn!("Chart path is not a directory: {}", chart_path.display());
            let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
            stream.write_all(response.as_bytes())?;
            return Ok(());
        }

        // 打包谱面目录为 ZIP
        let zip_data = Self::pack_chart_dir(chart_path)?;
        
        // 发送 HTTP 响应
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/zip\r\nConnection: close\r\n\r\n",
            zip_data.len()
        );
        stream.write_all(response.as_bytes())?;
        stream.write_all(&zip_data)?;
        stream.flush()?;

        info!("Chart {} sent successfully", chart_id);
        Ok(())
    }

    /// 打包谱面目录为 ZIP
    fn pack_chart_dir(chart_path: &Path) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            // 递归遍历目录并添加到 ZIP
            Self::add_directory_to_zip(&mut zip, options, chart_path, chart_path)?;
            zip.finish()?;
        }

        Ok(out)
    }

    /// 递归添加目录到 ZIP
    fn add_directory_to_zip(
        zip: &mut zip::ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
        options: zip::write::SimpleFileOptions,
        base: &Path,
        current: &Path,
    ) -> Result<()> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(base)?;

            if path.is_dir() {
                // 添加目录
                let dir_name = format!("{}/", relative.to_string_lossy());
                zip.add_directory(dir_name, options)?;
                // 递归处理子目录
                Self::add_directory_to_zip(zip, options, base, &path)?;
            } else {
                // 添加文件
                let file_name = relative.to_string_lossy();
                zip.start_file(file_name, options)?;
                
                let mut file = fs::File::open(&path)?;
                std::io::copy(&mut file, zip)?;
            }
        }

        Ok(())
    }
}

/// 文件下载客户端
pub struct FileDownloader {
    /// 服务器地址
    server_addr: String,
    /// 下载进度回调
    progress_callback: Box<dyn Fn(u8) + Send>,
}

impl FileDownloader {
    /// 创建新的下载器
    pub fn new(server_addr: String, progress_callback: Box<dyn Fn(u8) + Send>) -> Self {
        Self {
            server_addr,
            progress_callback,
        }
    }

    /// 下载谱面
    pub fn download_chart(&self, chart_id: &str, save_dir: &str) -> Result<()> {
        let mut stream = TcpStream::connect(&self.server_addr)
            .with_context(|| format!("Failed to connect to file server: {}", self.server_addr))?;

        // 发送下载请求
        let request = format!("GET /download/{} HTTP/1.1\r\nHost: {}\r\n\r\n", chart_id, self.server_addr);
        stream.write_all(request.as_bytes())?;

        // 读取响应
        let mut response = Vec::new();
        let mut buffer = vec![0u8; 4096];
        
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    response.extend_from_slice(&buffer[..n]);
                    
                    // 检查是否收到完整的响应头
                    if let Some(headers_end) = Self::find_headers_end(&response) {
                        let headers = String::from_utf8_lossy(&response[..headers_end]);
                        let body = &response[headers_end..];
                        
                        // 解析响应头
                        if let Some(content_length) = Self::parse_content_length(&headers) {
                            let total_size = content_length as usize;
                            let mut received = body.len();
                            
                            // 继续读取剩余数据
                            while received < total_size {
                                match stream.read(&mut buffer) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        received += n;
                                        response.extend_from_slice(&buffer[..n]);
                                        
                                        // 更新进度
                                        let progress = ((received * 100) / total_size) as u8;
                                        (self.progress_callback)(progress);
                                    }
                                    Err(e) => return Err(e.into()),
                                }
                            }
                        }
                        break;
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        // 保存文件（save_dir 为解压目标目录）
        let save_path = format!("{}/{}.zip", save_dir, chart_id);
        fs::write(&save_path, response)?;
        
        // 解压文件到 save_dir
        Self::extract_zip(&save_path, save_dir)?;
        
        // 删除 ZIP 文件
        fs::remove_file(save_path)?;

        Ok(())
    }

    /// 查找响应头结束位置
    fn find_headers_end(data: &[u8]) -> Option<usize> {
        // 查找连续的 \r\n\r\n
        for i in 0..data.len() - 3 {
            if data[i] == b'\r' && data[i + 1] == b'\n' && data[i + 2] == b'\r' && data[i + 3] == b'\n' {
                return Some(i + 4);
            }
        }
        None
    }

    /// 解析 Content-Length
    fn parse_content_length(headers: &str) -> Option<u64> {
        for line in headers.lines() {
            if line.starts_with("Content-Length:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(length) = parts[1].parse::<u64>() {
                        return Some(length);
                    }
                }
            }
        }
        None
    }

    /// 解压 ZIP 文件
    fn extract_zip(zip_path: &str, extract_to: &str) -> Result<()> {
        let file = fs::File::open(zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = format!("{}/{}", extract_to, file.name());
            
            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = Path::new(&outpath).parent() {
                    if !p.exists() {
                        fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }
        
        Ok(())
    }
}
