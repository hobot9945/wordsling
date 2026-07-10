//! TCP server module for communication with the mobile client.
//! 
//! Responsibilities:
//! - Establishing a connection with the client (accepting connections).
//! - Recovering connections if the client drops or reconnects.
//! - Reading raw bytes from the socket, strictly parsing UTF-8 without timeouts.
//! - Sending parsed strings to the main logic via MPSC channel.
//! - Providing a write interface to send commands back to the client.
//! 
//! # Implementation Details
//! To avoid complex asynchronous runtimes (like Tokio), this module uses two 
//! standard `std::thread`s:
//! 1. `listener_thread`: Binds to the TCP port, accepts incoming connections, and
//!    places the latest valid `TcpStream` into a shared mutex.
//! 2. `reader_thread`: Polls the shared mutex for a new connection. Once acquired,
//!    it performs blocking reads with a timeout (to allow checking the shutdown flag).
//!    Received bytes are decoded as UTF-8. If a multi-byte character is split 
//!    across TCP packets, the incomplete bytes are buffered until the next read.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::str;

use hobolib::{prln, prntln};
use crate::glob;
use crate::glob::is_shutdown_requested;

/// Represents the connection interface with the mobile phone.
#[derive(Debug)]
pub struct PhoneInterface {
    /// Shared connection used for sending commands to the phone.
    pub write_connect: Arc<Mutex<Option<TcpStream>>>,
    /// Handle for the listener thread.
    listener_handle: Option<thread::JoinHandle<()>>,
    /// Handle for the reading thread.
    read_handle: Option<thread::JoinHandle<()>>,
}

impl PhoneInterface {
    /// Timeout for read operations to prevent blocking the thread indefinitely.
    const SOCKET_READ_TIMEOUT_MS: u64 = 200;
    /// Timeout for write operations.
    const SOCKET_WRITE_TIMEOUT_MS: u64 = 200;
    /// Period for checking the shutdown flag in the listener.
    const LISTENER_POLL_MS: u64 = 100;

    /// Creates a new `PhoneInterface` and spawns the background threads.
    ///
    /// # Arguments
    /// * `sender_outside` - A channel to send received UTF-8 strings to the main app.
    pub fn new(sender_outside: mpsc::Sender<String>) -> Self {
        let write_connect = Arc::new(Mutex::new(None));
        let write_connect_cl = Arc::clone(&write_connect);

        // This will hold the newest accepted connection.
        let pending_connect: Arc<Mutex<Option<TcpStream>>> = Arc::new(Mutex::new(None));
        let pending_connect_listener = Arc::clone(&pending_connect);

        // 1. Spawn Listener Thread
        let listener_handle = thread::spawn(move || {
            Self::_listener_loop(pending_connect_listener);
        });

        // 2. Spawn Reader Thread
        let read_handle = thread::spawn(move || {
            Self::_reader_loop(pending_connect, write_connect_cl, sender_outside);
        });

        PhoneInterface {
            write_connect,
            listener_handle: Some(listener_handle),
            read_handle: Some(read_handle),
        }
    }

    /// Writes a string to the socket. Useful for external calls.
    pub fn write(&self, signal: &str) -> Result<(), String> {
        if let Some(stream) = self.write_connect.lock().unwrap().as_mut() {
            Self::_write_to_stream(stream, signal)
        } else {
            Err(format!("Attempt to send '{}' to an unconnected socket.", signal))
        }
    }

    /// Internal function to write to a specific stream.
    fn _write_to_stream(stream: &mut TcpStream, signal: &str) -> Result<(), String> {
        let command_as_line = format!("{}\n", signal);
        if let Err(err) = stream.write_all(command_as_line.as_bytes()) {
            return Err(format!("Error writing '{}' to socket: {}", signal, err));
        }
        if let Err(err) = stream.flush() {
            return Err(format!("Error flushing socket buffer: {}", err));
        }
        Ok(())
    }

    /// The main loop for the listener thread.
    fn _listener_loop(pending_connect: Arc<Mutex<Option<TcpStream>>>) {
        let port = glob::config_port();
        let bind_addr = format!("0.0.0.0:{}", port);
        
        let listener = match TcpListener::bind(&bind_addr) {
            Ok(lst) => lst,
            Err(err) => {
                panic!("Failed to bind phone socket listener to {}: {}", bind_addr, err);
                return;
            }
        };

        // Non-blocking listener to periodically check for shutdown.
        if let Err(err) = listener.set_nonblocking(true) {
            panic!("Failed to set listener non-blocking: {}", err);
        }

        loop {
            if is_shutdown_requested() {
                break;
            }

            match listener.accept() {
                Ok((stream, _addr)) => {
                    prln!("Phone interface: New connection established.");
                    
                    // Set timeouts on the new stream.
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(Self::SOCKET_READ_TIMEOUT_MS)));
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(Self::SOCKET_WRITE_TIMEOUT_MS)));
                    let _ = stream.set_nonblocking(false);

                    // Provide the new connection to the reader thread.
                    *pending_connect.lock().unwrap() = Some(stream);
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    // No incoming connections, sleep briefly.
                    thread::sleep(Duration::from_millis(Self::LISTENER_POLL_MS));
                }
                Err(err) => {
                    prln!("Phone interface: Error accepting connection: {}", err);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }

    /// The main loop for the reader thread.
    fn _reader_loop(
        pending_connect: Arc<Mutex<Option<TcpStream>>>,
        write_connect: Arc<Mutex<Option<TcpStream>>>,
        sender: mpsc::Sender<String>,
    ) {
        let mut current_stream: Option<TcpStream> = None;
        let mut raw_buf = [0u8; 1024];
        
        // Buffer for incomplete UTF-8 sequences.
        let mut leftover_bytes: Vec<u8> = Vec::new();

        loop {
            if is_shutdown_requested() {
                break;
            }

            // 1. Check if a new connection has arrived.
            if pending_connect.lock().unwrap().is_some() {
                let new_stream = pending_connect.lock().unwrap().take().unwrap();
                
                // Clone for the write interface.
                if let Ok(cloned) = new_stream.try_clone() {
                    *write_connect.lock().unwrap() = Some(cloned);
                }

                current_stream = Some(new_stream);
                leftover_bytes.clear(); // Discard leftovers from the old connection.
            }

            // 2. Perform reading if we have an active connection.
            if let Some(stream) = current_stream.as_mut() {
                match stream.read(&mut raw_buf) {
                    Ok(0) => {
                        prln!("Phone interface: Connection closed by client (EOF).");
                        current_stream = None;
                        *write_connect.lock().unwrap() = None;
                    }
                    Ok(bytes_read) => {
                        leftover_bytes.extend_from_slice(&raw_buf[..bytes_read]);
                        Self::_process_utf8_buffer(&mut leftover_bytes, &sender);
                    }
                    Err(err) if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock => {
                        // Normal timeout, allows the loop to check `is_shutdown_requested()`.
                    }
                    Err(err) => {
                        prln!("Phone interface: Read error: {}", err);
                        current_stream = None;
                        *write_connect.lock().unwrap() = None;
                    }
                }
            } else {
                // No active connection, sleep before checking again.
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    /// Extracts valid UTF-8 strings from the buffer and sends them to the channel.
    /// Leaves incomplete UTF-8 byte sequences in the buffer for the next read.
    fn _process_utf8_buffer(buffer: &mut Vec<u8>, sender: &mpsc::Sender<String>) {
        loop {
            if buffer.is_empty() {
                break;
            }

            match str::from_utf8(buffer) {
                Ok(valid_str) => {
                    // The entire buffer is valid UTF-8.
                    let _ = sender.send(valid_str.to_string());
                    buffer.clear();
                    break;
                }
                Err(utf8_error) => {
                    let valid_len = utf8_error.valid_up_to();
                    
                    // Extract and send the valid portion.
                    if valid_len > 0 {
                        // Safety: we know this portion is valid based on `valid_up_to()`.
                        let valid_str = unsafe { str::from_utf8_unchecked(&buffer[..valid_len]) };
                        let _ = sender.send(valid_str.to_string());
                        buffer.drain(..valid_len);
                        continue; // Re-evaluate the rest of the buffer.
                    }

                    // The error is at the very beginning of the buffer.
                    if let Some(error_len) = utf8_error.error_len() {
                        // Invalid UTF-8 bytes encountered. Discard them to recover.
                        prln!("Phone interface: Dropping {} invalid UTF-8 bytes.", error_len);
                        buffer.drain(..error_len);
                        continue;
                    } else {
                        // Incomplete UTF-8 sequence at the end. Wait for more bytes.
                        break;
                    }
                }
            }
        }
    }
}

impl Drop for PhoneInterface {
    fn drop(&mut self) {
        if let Some(handle) = self.listener_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.read_handle.take() {
            let _ = handle.join();
        }
        prln!("Phone interface threads dropped.");
    }
}
