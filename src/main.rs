use std::{io::Write, net::SocketAddr, path::PathBuf, pin::Pin, str::FromStr, time::Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_openssl::SslStream;

fn server(path: PathBuf) {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            let mut f = tokio::fs::File::open(path).await.unwrap();
            let size = f.metadata().await.unwrap().len();

            let socket = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();

            println!("Listening on {}", socket.local_addr().unwrap());

            let mut ssl =
                openssl::ssl::SslAcceptor::mozilla_intermediate(openssl::ssl::SslMethod::tls())
                    .unwrap();

            ssl.set_private_key_file("src/key.pem", openssl::ssl::SslFiletype::PEM)
                .unwrap();

            ssl.set_certificate_chain_file("src/cert.pem").unwrap();
            ssl.set_verify(openssl::ssl::SslVerifyMode::NONE);

            let ssl = ssl.build();

            let (tx, _) = socket.accept().await.unwrap();
            let mut tx =
                SslStream::new(openssl::ssl::Ssl::new(ssl.context()).unwrap(), tx).unwrap();

            Pin::new(&mut tx).accept().await.unwrap();

            tx.write_u64_le(size).await.unwrap();

            let start = Instant::now();
            tokio::io::copy(&mut f, &mut tx).await.unwrap();
            let start = start.elapsed();
            println!(
                "File sent in {start:?} ({} bytes/s)",
                size as f64 / start.as_secs_f64()
            );
        });
}

fn client() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            let ip_port = {
                print!("Enter peer IP address and port: ");
                std::io::stdout().lock().flush().unwrap();

                let mut addr = String::new();
                std::io::stdin().read_line(&mut addr).unwrap();

                SocketAddr::from_str(addr.trim()).unwrap()
            };

            let rx = tokio::net::TcpStream::connect(ip_port).await.unwrap();

            let mut ssl =
                openssl::ssl::SslConnector::builder(openssl::ssl::SslMethod::tls()).unwrap();

            ssl.set_ca_file("src/cert.pem").unwrap();
            ssl.set_verify(openssl::ssl::SslVerifyMode::NONE);

            let ssl = ssl
                .build()
                .configure()
                .unwrap()
                .into_ssl("tcptransfer")
                .unwrap();
            let mut rx = SslStream::new(ssl, rx).unwrap();

            Pin::new(&mut rx).connect().await.unwrap();

            let size = rx.read_u64_le().await.unwrap();

            let mut f = tokio::fs::File::create(std::env::temp_dir().join("received.bin"))
                .await
                .unwrap();

            let start = Instant::now();
            tokio::io::copy(&mut rx.take(size), &mut f).await.unwrap();
            let start = start.elapsed();
            println!(
                "File received in {start:?} ({} bytes/s)",
                size as f64 / start.as_secs_f64()
            );
        });
}

fn main() {
    let path = {
        print!("Enter file path or press enter to receive: ");
        std::io::stdout().lock().flush().unwrap();

        let mut path = String::new();
        std::io::stdin().read_line(&mut path).unwrap();

        let path = path.trim();

        if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path))
        }
    };

    if let Some(path) = path {
        server(path);
    } else {
        client();
    }
}
