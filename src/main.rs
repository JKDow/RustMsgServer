use tokio::{net::TcpListener, io::{AsyncReadExt, AsyncWriteExt}};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("localhost:8080").await.unwrap();

    let (mut socket, addr) = listener.accept().await.unwrap();

    let mut buffer = [0u8; 1024];

    let num_bytes = socket.read(&mut buffer).await.unwrap();
    println!("bytes received: {:?}", buffer);
    socket.write_all(&buffer[0..num_bytes]).await.unwrap();
}
