use std::net::SocketAddr;

use tokio::{
    net::{TcpListener, TcpStream}, 
    io::{AsyncWriteExt, BufReader, AsyncBufReadExt}, sync::broadcast
};

enum ServerCommand {
    Host(String),
    Join(String),
    Leave,
    Shutdown, 
    Msg(String),
    Quit,
    Help,
    Status
}

#[derive(PartialEq)]
enum Mode {
    Admin,
    Host(String),
    Client(String),
}

#[tokio::main]
async fn main() {
    let mut mode = Mode::Admin;
    println!(">>Welcome to the chat server");
    print_help();
    loop {
        match get_command().await {
            ServerCommand::Host(addr) => {
                println!(">>Hosting on {}", addr);
                mode = Mode::Host{0: addr};
            },
            ServerCommand::Join(addr) => {
                println!(">>Joining {}", addr);
                mode = Mode::Client{0: addr};
            } 
            ServerCommand::Leave => println!(">>Cannot leave when not in a session"),
            ServerCommand::Shutdown => println!(">>Cannot shutdown when not hosting server"),
            ServerCommand::Msg(_) => println!(">>Cannot send message when not in a session"),
            ServerCommand::Quit => {
                println!(">>Quitting");
                return;
            }
            ServerCommand::Help => print_help(),
            ServerCommand::Status => println!(">>Not in a session or hosting a server"),
        } 
        while let Mode::Host(addr) = &mode {
            tokio::select! {
                host_return = host_server(addr.clone()) => {
                    match host_return {
                        Ok(_) => {},
                        Err(_) => {
                            println!(">>Error hosting server");
                            mode = Mode::Admin;
                        }
                    }
                }   
                command = get_command() => {
                    match command {
                        ServerCommand::Host(_) => println!(">>Already hosting"),
                        ServerCommand::Join(_) => println!(">>Already hosting, cannot join another session"),
                        ServerCommand::Leave => {
                            println!(">>Can only leave if in a session");
                            mode = Mode::Admin;
                        },
                        ServerCommand::Shutdown => {
                            println!(">>Shutting down");
                            return;
                        },
                        ServerCommand::Msg(_) => println!("Cannot send message when hosting"),
                        ServerCommand::Quit => {
                            println!(">>Quitting");
                            return;
                        }
                        ServerCommand::Help => print_help(),
                        ServerCommand::Status => println!(">>Currently hosting on {}", addr),
                    }
                }
            }
        }

        if let Mode::Client(addr) = &mode {
           match join_session(addr.clone()).await {
               Ok(_) => {},
               Err(_) => {
                   println!(">>Error joining server");
                   mode = Mode::Admin;
               }
           } 
        }
    }
}

async fn get_command() -> ServerCommand {
    let mut input = String::new();
    let in_stream = tokio::io::stdin();
    let mut reader = BufReader::new(in_stream);
    loop {
        input.clear();
        match reader.read_line(&mut input).await {
            Ok(read_bytes) => {
                if read_bytes == 0 {
                    println!(">>Read 0 from stdin, shutting down");
                    return ServerCommand::Shutdown;
                }
            },
            Err(_) => {
                println!(">>Error reading from stdin, shutting down");
                return ServerCommand::Shutdown;
            },
        }
        let words: Vec<&str> = input.split_whitespace().collect();
        match words[0].trim() {
            "host" => {
                if words.len() == 1 {
                    return ServerCommand::Host{0: "localhost:8080".to_string()};
                }
                else if words.len() != 2 {
                    println!(">>host only takes one argument - the address to host on");
                    continue;
                }
                return ServerCommand::Host{0: words[1].to_string()};
            }
            "join" => {
                if words.len() == 1 {
                    return ServerCommand::Join{0: "localhost:8080".to_string()};
                }
                else if words.len() != 2 {
                    println!(">>join onlt takes one argument - the address to join");
                    continue;
                }
                return ServerCommand::Join{0: words[1].to_string()};
            } 
            "leave" => return ServerCommand::Leave,
            "shutdown" => return ServerCommand::Shutdown,
            "msg" => {
                if words.len() < 2 {
                    println!(">>Please provide a message to send");
                    continue;
                }
                return ServerCommand::Msg{0: words[1..].join(" ")};
            }
            "quit" => return ServerCommand::Quit,
            "help" => return ServerCommand::Help,
            "status" => return ServerCommand::Status,
            _ => println!(">>Invalid command")
        }
    }
}

async fn host_server(bind: String) -> Result<(),()> {
    let listener = match TcpListener::bind(bind).await {
        Ok(listener) => listener,
        Err(_) => {
            println!(">>Failed to bind to address");
            return Err(());
        }
    };

    let (tx,_rx) = broadcast::channel(10);

    loop {
        let tx = tx.clone();
        
        let (socket, addr) = match listener.accept().await {
            Ok((socket, addr)) => (socket, addr),
            Err(_) => {
                println!(">>Failed to accept connection");
                continue;
            }
        };
        println!("->Server received new client, creating connection"); //DEBUG
        tokio::spawn(handle_server_connection(socket, tx, addr));
    }
}

async fn handle_server_connection(mut socket: TcpStream, tx: broadcast::Sender<(String, SocketAddr)>, addr: SocketAddr) -> Result<(), ()> {

    let (read, mut writer) = socket.split();
    let mut rx = tx.subscribe();
    let mut reader = BufReader::new(read);
    let mut line = String::new();

    loop {
        tokio::select! {
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(read_bytes) => {
                        if read_bytes == 0 {
                            println!(">>Read 0 bytes from socket, closing connection to that client");
                        }
                    }
                    Err(_) => {
                        println!(">>Failed to read from socket, closing connection");
                        return Err(());
                    }
                }

                match tx.send((line.clone(), addr)) {
                    Ok(_) => {},
                    Err(_) => {
                        println!(">>Failed to send message to broadcast channel");
                        return Err(());
                    }
                }
                println!("->Server reveived message and sent to broadcast: {}", line); //DEBUG
                line.clear(); 
            }
            result = rx.recv() => {
                let (msg, other_addr) = match result {
                    Ok((msg, addr)) => (msg, addr),
                    Err(_) => {
                        println!(">>Failed to receive message from broadcast channel");
                        return Err(());
                    }
                }; 
                println!("->Server received message from broadcast: {}", msg); //DEBUG
                if addr != other_addr {
                    match writer.write_all(msg.as_bytes()).await {
                        Ok(_) => {},
                        Err(_) => {
                            println!(">>Failed to write to socket");
                            return Err(());
                        }
                    };
                }
            }
        }
    }
}

fn print_help() {
    println!(">>Commands:");
    println!(">>host <address> - host a server on the given address");
    println!(">>join <address> - join a server on the given address");
    println!(">>leave - leave the current server");
    println!(">>shutdown - shutdown the current server");
    println!(">>msg <message> - send a message to the current server");
    println!(">>quit - quit the program");
    println!(">>help - print this help message");
    println!(">>status - print the current status");
}

async fn join_session(addr: String) -> Result<(),()> {
    let socket = match TcpStream::connect(addr.clone()).await {
        Ok(socket) => socket,
        Err(_) => {
            println!(">>Failed to connect to server");
            return Err(());
        }
    };

    let (socket_read, mut socket_write) = socket.into_split();
    let mut socket_reader = BufReader::new(socket_read);
    let mut line = String::new();

    let mut name = String::new();
    let mut io_reader = BufReader::new(tokio::io::stdin());
    println!(">>Enter your name:");
    loop {
        name.clear();
        match io_reader.read_line(&mut name).await {
            Ok(n) => {
                if n == 0 {
                    println!(">>Please enter a non 0 length name");
                    continue;
                }
                break;
            },
            Err(_) => {
                println!(">>Failed to read name");
                return Err(());
            }
        }
    }
    println!(">>Thankyou {}, you are not connected to the server", name);
    loop {
        tokio::select! {
            result = socket_reader.read_line(&mut line) => {
                match result {
                    Ok(read_bytes) => {
                        if read_bytes == 0 {
                            println!(">>Read 0 bytes from socket, closing connection");
                            return Ok(());
                        }
                    }
                    Err(_) => {
                        println!(">>Failed to read from socket, closing connection");
                        return Ok(()); // Returning Ok except for quit so signify finishing program 
                    }
                }
                println!("->Client received msg");
                println!("{}", line);
                line.clear();
            }
            result = get_command() => {
                match result {
                    ServerCommand::Host(_) => println!(">>Already in a session, cannot host"),
                    ServerCommand::Join(_) => println!(">>Already in a session, cannot join another session"),
                    ServerCommand::Leave => {
                        println!(">>Leaving");
                        return Ok(());
                    },
                    ServerCommand::Shutdown => println!(">>Cannot shutdown when not hosting server"),
                    ServerCommand::Msg(msg) => {
                        //send msg 
                        let msg = format!("{}: {}", name, msg);
                        if let Err(_) = socket_write.write_all(msg.as_bytes()).await {
                            println!(">>Failed to send message");
                            return Err(());
                        }
                    }
                    ServerCommand::Quit => {
                        println!(">>Quitting");
                        return Err(());
                    }
                    ServerCommand::Help => print_help(), 
                    ServerCommand::Status => println!(">>Currently in session with {}", addr),
                }
            }
        }
    }
}