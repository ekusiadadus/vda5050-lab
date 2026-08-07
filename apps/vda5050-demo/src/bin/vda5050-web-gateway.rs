use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::ExitCode,
    time::Duration,
};

use clap::Parser;
use vda5050_demo::{LiveWebStore, WebBody, WebResponse};

const REQUEST_LIMIT: u64 = 8 * 1_024;

#[derive(Debug, Parser)]
#[command(name = "vda5050-web-gateway")]
#[command(about = "Read-only local artifact gateway for the isolated live demo")]
struct Cli {
    #[arg(long, default_value = "0.0.0.0:8080")]
    listen: String,
    #[arg(long, default_value = "/artifacts")]
    artifact_dir: PathBuf,
    #[arg(long, default_value = "/web")]
    static_dir: PathBuf,
    #[arg(long, default_value_t = 8 * 1_024 * 1_024)]
    max_file_bytes: u64,
}

fn main() -> ExitCode {
    match run(&Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vda5050-web-gateway: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let store = LiveWebStore::new(&cli.artifact_dir, &cli.static_dir, cli.max_file_bytes)?;
    let listener = TcpListener::bind(&cli.listen)?;
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                serve_connection(&store, &mut stream)?;
            }
            Err(error) => eprintln!("vda5050-web-gateway: accept failed: {error}"),
        }
    }
    Ok(())
}

fn serve_connection(
    store: &LiveWebStore,
    stream: &mut TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = read_request(stream)?;
    if u64::try_from(request.len()).unwrap_or(u64::MAX) > REQUEST_LIMIT {
        return write_response(stream, store.resolve("GET", "/request-too-large"));
    }
    let request_text = std::str::from_utf8(&request)?;
    let Some(request_line) = request_text.lines().next() else {
        return write_response(stream, store.resolve("GET", "/bad-request"));
    };
    let mut components = request_line.split_ascii_whitespace();
    let method = components.next().unwrap_or_default();
    let path = components.next().unwrap_or_default();
    let version = components.next().unwrap_or_default();
    let response = if components.next().is_none() && version == "HTTP/1.1" {
        store.resolve(method, path)
    } else {
        store.resolve("GET", "/bad-request")
    };
    write_response(stream, response)
}

fn read_request(stream: &mut TcpStream) -> Result<Vec<u8>, std::io::Error> {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1_024];
    while u64::try_from(request.len()).unwrap_or(u64::MAX) <= REQUEST_LIMIT {
        let received = stream.read(&mut chunk)?;
        if received == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..received]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    Ok(request)
}

fn write_response(
    stream: &mut TcpStream,
    response: WebResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let WebBody::Bytes(body) = response.body;
    let reason = match response.status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Content Too Large",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        reason,
        response.content_type,
        body.len()
    )?;
    for (name, value) in response.headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    stream.write_all(b"\r\n")?;
    stream.write_all(&body)?;
    stream.flush()?;
    Ok(())
}
