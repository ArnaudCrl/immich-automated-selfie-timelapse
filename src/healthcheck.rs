use std::net::TcpStream;
use std::process;
use std::time::Duration;

fn main() {
    // Try to connect to the local server on port 5000
    // We give it a 2-second timeout
    let address = "127.0.0.1:5000";
    if TcpStream::connect_timeout(&address.parse().unwrap(), Duration::from_secs(2)).is_ok() {
        process::exit(0); // Exit 0 = Healthy
    } else {
        process::exit(1); // Exit 1 = Unhealthy
    }
}