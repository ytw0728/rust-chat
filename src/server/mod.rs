use std::collections::HashMap;
use std::error::Error;
use std::net::{SocketAddr, Ipv4Addr, IpAddr};
use std::sync::Arc;

use futures::SinkExt;
use tokio::io::{stdout, BufWriter, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, mpsc};
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};



#[tokio::main]
pub async fn run() {
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let socket_addr = SocketAddr::new(ip, 6142);
    let listener = TcpListener::bind(socket_addr).await.unwrap();
    let state = Arc::new(Mutex::new(Shared::new()));

    println!("Listening on {}:{}", socket_addr.ip(), socket_addr.port());

    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        let state = Arc::clone(&state);

        tokio::spawn(async move {
            if let Err(e) = process(state, stream, addr).await {
                println!("{:?}", e);
            }
        });
    }
}

type Tx = mpsc::UnboundedSender<String>;
type Rx = mpsc::UnboundedReceiver<String>;

/// The state for each connected client.
struct Peer {
    /// The TCP socket wrapped with the `Lines` codec, defined below.
    ///
    /// This handles sending and receiving data on the socket. When using
    /// `Lines`, we can work at the line level instead of having to manage the
    /// raw byte operations.
    read_stream: Framed<TcpStream, LinesCodec>,

    /// Receive half of the message channel.
    ///
    /// This is used to receive messages from peers. When a message is received
    /// off of this `Rx`, it will be written to the socket.
    rx: Rx,
}

struct Shared {
    peers: HashMap<SocketAddr, Tx>,
}
impl Shared {
    /// Create a new, empty, instance of `Shared`.
    fn new() -> Self {
        Shared {
            peers: HashMap::new(),
        }
    }

    /// Send a `LineCodec` encoded message to every peer, except
    /// for the sender.
    async fn broadcast(&mut self, sender: SocketAddr, username: Option<&str>, message: &str) {
        for peer in self.peers.iter_mut() {
            if *peer.0 != sender {
                let _ = peer.1.send(
                    match username {
                        Some(username) => {
                            format!("{}: {}", username, message)
                        },
                        None => {
                            message.to_string()
                        }
                    }
                );
            }
        }
    }
}
impl Peer {
    /// Create a new instance of `Peer`.
    async fn new(
        state: Arc<Mutex<Shared>>,
        read_stream: Framed<TcpStream, LinesCodec>,
    ) -> std::io::Result<Peer> {
        // Get the client socket address
        let addr = read_stream.get_ref().peer_addr()?;

        // Create a channel for this peer
        let (tx, rx) = mpsc::unbounded_channel();

        // Add an entry for this `Peer` in the shared state map.
        state.lock().await.peers.insert(addr, tx);

        Ok(Peer { read_stream, rx })
    }
}

/// Process an individual chat client
async fn process(
    state: Arc<Mutex<Shared>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let mut read_stream = Framed::new(stream, LinesCodec::new());

    // Send a prompt to the client to enter their username.
    read_stream.send("Enter username: ").await.unwrap();
    futures::SinkExt::<String>::flush(&mut read_stream).await.unwrap();

    let mut stdout = BufWriter::new(stdout());
    
    // Read the first line from the `LineCodec` stream to get the username.
    let username = match read_stream.next().await {
        Some(Ok(line)) => line,
        // We didn't get a line so we return early here.
        _ => {
            stdout.write_all(format!("Failed to get username from {}. Client disconnected.", addr).as_bytes()).await.unwrap();
            stdout.flush().await.unwrap();
            return Ok(());
        }
    };

    // Register our peer with state which internally sets up some channels.
    let mut peer = Peer::new(state.clone(), read_stream).await?;
    {
        let mut state = state.lock().await;
        let msg = format!("{} has connected.", username);

        stdout.write_all(format!("{}\n", msg).as_bytes()).await.unwrap();
        stdout.flush().await.unwrap();

        state.broadcast(addr, None, &msg).await;
    }

    // Process incoming messages until our stream is exhausted by a disconnect.
    loop {
        tokio::select! {
            // A message was received from a peer. Send it to the current user.
            Some(msg) = peer.rx.recv() => {
                peer.read_stream.send(&msg).await?;
            }
            result = peer.read_stream.next() => match result {
                // A message was received from the current user, we should
                // broadcast this message to the other users.
                Some(Ok(msg)) => {
                    let mut state = state.lock().await;
                    stdout.write_all(format!("[user: {}] {}\n", username, msg).as_bytes()).await.unwrap();
                    stdout.flush().await.unwrap();

                    state.broadcast(addr, Some(&username), &msg).await;
                }
                // An error occurred.
                Some(Err(e)) => {
                    stdout.write_all(format!("an error occurred while processing messages for {}; error = {:?}\n", username, e).as_bytes()).await.unwrap();
                    stdout.flush().await.unwrap();
                }
                // The stream has been exhausted.
                None => break,
            },
        }
    }

    // If this section is reached it means that the client was disconnected!
    // Let's let everyone still connected know about it.
    {
        let mut state = state.lock().await;
        state.peers.remove(&addr);

        let msg = format!("{} has left the chat", username);
        stdout.write_all(format!("{}\n", msg).as_bytes()).await.unwrap();
        stdout.flush().await.unwrap();
        state.broadcast(addr, None, &msg).await;
    }

    Ok(())
}
