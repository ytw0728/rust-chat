

use std::{net::{Ipv4Addr, IpAddr, SocketAddr}};


use futures::{SinkExt, StreamExt};
use tokio::{net::{TcpStream}, io::{BufWriter, AsyncWriteExt, AsyncBufReadExt}};
use tokio_util::codec::{Framed, LinesCodec, FramedRead};

#[tokio::main]
pub async fn run() {
    let ip = IpAddr::V4(Ipv4Addr::new(127,0,0,1));
    let socket_addr = SocketAddr::new(ip, 6142);
    let connection = TcpStream::connect(socket_addr).await.unwrap();
    let mut server = Framed::new(connection, LinesCodec::new_with_max_length(1024));
    
    let mut stdin = FramedRead::new(tokio::io::stdin(), LinesCodec::new_with_max_length(1024));
    let mut stdout = BufWriter::new(tokio::io::stdout());

    if let Some(Ok(line)) = server.next().await {
        stdout.write_all(line.as_bytes()).await.unwrap();
        stdout.flush().await.unwrap();
    };

    let username = stdin.next().await.unwrap().unwrap();

    server.send(username.trim()).await.unwrap();
    futures::SinkExt::<String>::flush(&mut server).await.unwrap();

    let (mut send_sink, mut read_stream) = server.split::<String>();
    
    let writing_process = tokio::spawn(async move {
        stdout.write_all("> ".as_bytes()).await.unwrap();
        stdout.flush().await.unwrap();
        while let Some(Ok(msg)) = stdin.next().await {           
            send_sink.send(msg.trim().into()).await.unwrap();
            futures::SinkExt::<String>::flush(&mut send_sink).await.unwrap();
            stdout.write_all("> ".as_bytes()).await.unwrap();
            stdout.flush().await.unwrap();
        }
    });

    let mut stdout = BufWriter::new(tokio::io::stdout());
    let reading_process = tokio::spawn(async move {
        while let Some(Ok(line)) = read_stream.next().await {
            stdout.write_all(format!("\x1b[2K{}\n> ", line).as_bytes()).await.unwrap();
            stdout.flush().await.unwrap();
        };
    });

    let mut stdout = BufWriter::new(tokio::io::stdout());
    match tokio::join!(writing_process, reading_process) {
        (Err(e), Ok(_)) => {
            stdout.write_all(e.to_string().as_bytes()).await.unwrap();
            stdout.flush().await.unwrap();
        },
        (Ok(_), Err(e)) => {
            stdout.write_all(e.to_string().as_bytes()).await.unwrap();
            stdout.flush().await.unwrap();
        },
        (Err(e1), Err(e2)) => {
            stdout.write_all(format!("{} {}", e1, e2).as_bytes()).await.unwrap();
            stdout.flush().await.unwrap();
        },
        _ => ()
    };
}