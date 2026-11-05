//! Real I/O: a TCP echo server with concurrent clients, and async file access.
//! Run with: cargo run --example tokio_io

use tokio::fs;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

/// Handles one connection: reads lines and sends each one back in upper case.
async fn handle_client(stream: TcpStream) -> io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        writer.write_all(line.to_uppercase().as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }
    Ok(()) // next_line returned None: the client closed its side.
}

/// Accepts connections forever, with one task per client, so that a slow client
/// doesn't hold up the others.
async fn run_server(listener: TcpListener) -> io::Result<()> {
    loop {
        let (stream, peer) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(error) = handle_client(stream).await {
                eprintln!("server: client {peer} failed: {error}");
            }
        });
    }
}

/// Connects, sends one line, and returns the server's reply.
async fn run_client(addr: std::net::SocketAddr, message: String) -> io::Result<String> {
    let mut stream = TcpStream::connect(addr).await?;
    stream.write_all(message.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    let mut reply = String::new();
    BufReader::new(&mut stream).read_line(&mut reply).await?;
    Ok(reply.trim_end().to_string())
}

async fn file_demo() -> io::Result<()> {
    let path = std::env::temp_dir().join("tokio_io_example.txt");

    // Write the whole file at once, then read it back line by line.
    fs::write(&path, "first line\nsecond line\n").await?;
    let file = fs::File::open(&path).await?;
    let mut lines = BufReader::new(file).lines();
    while let Some(line) = lines.next_line().await? {
        println!("file:   {line}");
    }

    // Errors work as usual, with `?` or by matching on them.
    fs::remove_file(&path).await?;
    match fs::read_to_string(&path).await {
        Ok(_) => println!("file:   unexpectedly still there"),
        Err(error) => println!("file:   after removal: {error}"),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // Port 0 lets the OS pick a free port, so the example can't clash with anything.
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    println!("server: listening on {addr}");
    let server = tokio::spawn(run_server(listener));

    let mut clients = JoinSet::new();
    for id in 0..3 {
        clients.spawn(run_client(addr, format!("hello from client {id}")));
    }
    while let Some(result) = clients.join_next().await {
        println!("client: got reply: {}", result.expect("client task panicked")?);
    }

    // `abort` cancels a task at its next .await. Tasks are also dropped when the runtime ends.
    server.abort();

    file_demo().await
}
