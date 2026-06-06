use sidecar_core::{validate_broker_hello, BrokerIdentity, BrokerRequest, BrokerResponse};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

pub(crate) fn serve(project: &str, namespace: &str) -> Result<(), String> {
    let identity = BrokerIdentity::new(project, namespace);
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|err| format!("failed to bind broker listener: {err}"))?;
    let addr = listener
        .local_addr()
        .map_err(|err| format!("failed to read broker listener address: {err}"))?;
    println!("tcp://{addr}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let identity = identity.clone();
                std::thread::spawn(move || {
                    let _ = handle_client(stream, &identity);
                });
            }
            Err(err) => return Err(format!("broker listener failed: {err}")),
        }
    }
    Ok(())
}

fn handle_client(mut stream: TcpStream, identity: &BrokerIdentity) -> Result<(), String> {
    let mut line = String::new();
    {
        let mut reader = BufReader::new(
            stream
                .try_clone()
                .map_err(|err| format!("failed to clone broker stream: {err}"))?,
        );
        reader
            .read_line(&mut line)
            .map_err(|err| format!("failed to read broker request: {err}"))?;
    }
    if line.trim().is_empty() {
        return Ok(());
    }
    let request: BrokerRequest = serde_json::from_str(line.trim())
        .map_err(|err| format!("failed to parse broker request: {err}"))?;
    let response = match validate_broker_hello(&request, identity) {
        Ok(response) => response,
        Err(message) => BrokerResponse::HelloError { message },
    };
    let text = serde_json::to_string(&response).map_err(|err| err.to_string())?;
    stream
        .write_all(text.as_bytes())
        .and_then(|_| stream.write_all(b"\n"))
        .map_err(|err| format!("failed to write broker response: {err}"))
}

#[doc(hidden)]
pub mod __test {
    use super::handle_client;
    use sidecar_core::BrokerIdentity;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};

    pub fn round_trip(request: &str) -> Result<String, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|err| format!("bind test listener: {err}"))?;
        let addr = listener.local_addr().map_err(|err| err.to_string())?;
        let worker = std::thread::spawn(move || {
            let (stream, _) = listener.accept().map_err(|err| err.to_string())?;
            handle_client(stream, &BrokerIdentity::new("sidecar", "default"))
        });

        let mut stream = TcpStream::connect(addr).map_err(|err| err.to_string())?;
        stream
            .write_all(request.as_bytes())
            .and_then(|_| stream.write_all(b"\n"))
            .map_err(|err| err.to_string())?;
        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .map_err(|err| err.to_string())?;
        worker
            .join()
            .map_err(|_| "broker test worker panicked".to_string())??;
        Ok(line)
    }
}
