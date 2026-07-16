//! TCP server module for communication with the mobile client.
//! 
//! Responsibilities:
//! - Establishing a connection with the client (accepting connections).
//! - Recovering connections if the client drops or reconnects.
//! - Reading raw bytes from the socket and assembling valid UTF-8 strings.
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

use hobolib::glob::is_shutdown_requested;
use crate::{fatal, glob, log_inf, log_wrn};

/// Represents the connection interface with the mobile phone.
#[derive(Debug)]
pub struct TcpServer {

    /// Shared connection used for sending commands to the phone.
    pub write_connect: Arc<Mutex<Option<TcpStream>>>,

    /// Handle for the listener thread.
    listener_handle: Option<thread::JoinHandle<()>>,

    /// Handle for the reading thread.
    read_handle: Option<thread::JoinHandle<()>>,
}

impl TcpServer {

    /// Timeout for read operations to prevent blocking the thread indefinitely.
    const SOCKET_READ_TIMEOUT_MS: u64 = 200;

    /// Timeout for write operations.
    const SOCKET_WRITE_TIMEOUT_MS: u64 = 200;

    /// Period for checking the shutdown flag in the listener.
    const LISTENER_POLL_MS: u64 = 200;

    /// Creates a new `TcpServer` and spawns the background threads.
    ///
    /// # Arguments
    /// * `text_tx` - A channel to send received UTF-8 strings to the main app.
    pub fn new(text_tx: mpsc::Sender<String>) -> Self {

        // This will hold the connection clone for writing to the socket.
        let write_connect = Arc::new(Mutex::new(None));
        let write_connect_cl = Arc::clone(&write_connect);

        // This will hold the newest accepted connection.
        let pending_connect: Arc<Mutex<Option<TcpStream>>> = Arc::new(Mutex::new(None));
        let listener_connect = Arc::clone(&pending_connect);

        // 1. Spawn Listener Thread
        let listener_handle = thread::spawn(move || {
            Self::_listener_loop(listener_connect);
        });

        // 2. Spawn Reader Thread
        let read_handle = thread::spawn(move || {
            Self::_reader_loop(pending_connect, write_connect_cl, text_tx);
        });

        TcpServer {
            write_connect,
            listener_handle: Some(listener_handle),
            read_handle: Some(read_handle),
        }
    }

    /// Sends a string to the connected client via the shared write socket.
    ///
    /// # Arguments
    /// - `str_tx`: the string to send. A newline is appended automatically.
    ///
    /// # Returns
    /// - `Ok(())` if the data was written and flushed successfully.
    /// - `Err(String)` if no connection exists or a write error occurred.
    pub fn write(&self, str_tx: &str) -> Result<(), String> {
        if let Some(stream) = self.write_connect.lock().unwrap().as_mut() {
            Self::_write_to_stream(stream, str_tx)
        } else {
            Err(format!("Attempt to send '{}' to an unconnected socket.", str_tx))
        }
    }

    /// Writes a string followed by a newline to the given TCP stream and flushes it.
    ///
    /// # Arguments
    /// - `stream`: the target TCP stream.
    /// - `str_tx`: the string to send. A newline is appended automatically.
    ///
    /// # Returns
    /// - `Ok(())` on success.
    /// - `Err(String)` if `write_all` or `flush` fails.
    fn _write_to_stream(stream: &mut TcpStream, str_tx: &str) -> Result<(), String> {
        let command_as_line = format!("{}\n", str_tx);
        if let Err(err) = stream.write_all(command_as_line.as_bytes()) {
            return Err(format!("Error writing '{}' to socket: {}", str_tx, err));
        }
        if let Err(err) = stream.flush() {
            return Err(format!("Error flushing socket buffer: {}", err));
        }
        Ok(())
    }

    /// Main loop for the listener thread.
    ///
    /// Binds to the configured TCP port and accepts incoming client connections
    /// in non-blocking mode.
    ///
    /// Algorithm:
    /// - Binds a `TcpListener` to `0.0.0.0:<port>` and sets it to non-blocking mode.
    /// - In a loop, checks the shutdown flag and calls `accept()`.
    /// - On a new connection, configures read/write timeouts and places the stream
    ///   into `pending_connect` for the reader thread to pick up.
    /// - On `WouldBlock`, sleeps briefly before retrying.
    /// - On bind or setup failure, triggers `fatal!` and returns.
    ///
    /// # Arguments
    /// - `pending_connect`: shared state where the accepted `TcpStream` is placed
    ///   for the reader thread to consume.
    ///
    /// # Side effects
    /// Writes into `pending_connect`. Previous unconsumed connections are silently replaced.
    fn _listener_loop(pending_connect: Arc<Mutex<Option<TcpStream>>>) {
        let port = glob::appconf_port();
        let bind_addr = format!("0.0.0.0:{}", port);
        
        let listener = match TcpListener::bind(&bind_addr) {
            Ok(lst) => lst,
            Err(err) => {
                fatal!("Failed to bind phone socket listener to {}: {}", bind_addr, err);
                return;
            }
        };

        // Non-blocking listener to periodically check for shutdown.
        if let Err(err) = listener.set_nonblocking(true) {
            fatal!("Failed to set listener non-blocking: {}", err);
            return;
        }

        loop {
            if is_shutdown_requested() {
                break;
            }

            match listener.accept() {
                Ok((stream, _addr)) => {
                    log_inf!("Tcp server: New connection established.");
                    
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
                    log_inf!("Tcp server: Error accepting connection: {}", err);
                    thread::sleep(Duration::from_secs(10));
                }
            }
        }
    }

    /// Main loop for the socket reading thread.
    ///
    /// Runs in a dedicated thread. It waits for a new connection from the listener thread,
    /// and then continuously reads raw bytes from it.
    ///
    /// Algorithm:
    /// - Checks the global application shutdown request flag.
    /// - Checks `pending_connect` for a newly accepted connection. If found, it intercepts
    ///   the connection, replaces the current active stream, and clears old buffers.
    /// - Reads from the active socket in chunks with a timeout.
    /// - Appends read data to leftovers from previous packets and delegates UTF-8 parsing.
    ///
    /// # Arguments
    /// - `pending_connect`: shared state where the listener thread places new incoming TCP connections.
    /// - `write_connect`: shared state used to expose a clone of the active socket to the outside
    ///   (for sending commands back to the client).
    /// - `text_tx`: channel for sending decoded strings to the logical core.
    ///
    /// # Side effects
    /// Modifies `write_connect`, setting or clearing it when a client connects or disconnects.
    fn _reader_loop(
        pending_connect: Arc<Mutex<Option<TcpStream>>>,
        write_connect: Arc<Mutex<Option<TcpStream>>>,
        text_tx: mpsc::Sender<String>,
    ) {
        let mut current_stream: Option<TcpStream> = None;
        let mut raw_buf = [0u8; 1024];

        // Buffer for accumulating incomplete UTF-8 sequences split across TCP packet boundaries
        let mut leftover_bytes: Vec<u8> = Vec::new();

        loop {

            // Exit loop if shutdown requested.
            if is_shutdown_requested() {
                break;
            }   // if

            // 1. Check if a new connection arrived from the listener
            if let Some(new_stream) = pending_connect.lock().unwrap().take() {

                // Clone the socket to expose a write interface to other parts of the app
                if let Ok(cloned) = new_stream.try_clone() {
                    *write_connect.lock().unwrap() = Some(cloned);
                }   // if

                current_stream = Some(new_stream);
                leftover_bytes.clear(); // Discard residual bytes from the previous connection
            }   // if

            // 2. Read data if there is an active connection
            if let Some(stream) = current_stream.as_mut() {
                match stream.read(&mut raw_buf) {
                    Ok(0) => {
                        log_inf!("Phone interface: Connection closed by client (EOF).");
                        current_stream = None;
                        *write_connect.lock().unwrap() = None;
                    }
                    Ok(bytes_read) => {
                        leftover_bytes.extend_from_slice(&raw_buf[..bytes_read]);
                        Self::_process_utf8_buffer(&mut leftover_bytes, &text_tx);
                    }
                    Err(err) if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock => {
                        // Normal read timeout, allows the loop to check `is_shutdown_requested()`
                    }
                    Err(err) => {
                        log_inf!("Phone interface: Read error: {}", err);
                        current_stream = None;
                        *write_connect.lock().unwrap() = None;
                    }
                }   // match
            } else {
                // Wait briefly before the next check if no active connection exists
                thread::sleep(Duration::from_millis(Self::LISTENER_POLL_MS));
            }   // if
        }   // loop
    }   // _reader_loop()

    /// Extracts valid UTF-8 strings from the buffer and sends them to the channel.
    ///
    /// This function handles TCP stream fragmentation. It processes the buffer until
    /// it either consumes all bytes, or encounters an incomplete multi-byte UTF-8
    /// sequence at the end of the buffer (which is then left for the next read cycle).
    ///
    /// Algorithm:
    /// - Loops over the buffer while it has data.
    /// - Attempts to parse the buffer as UTF-8.
    /// - If successful, sends the string and clears the buffer.
    /// - If a UTF-8 error occurs, checks the valid portion before the error:
    ///   - If there is valid text, extracts it, sends it, removes those bytes, and restarts the loop.
    ///   - If the error is exactly at the beginning, checks the nature of the error:
    ///     - Corrupted data (has `error_len`): drops the bad bytes to recover.
    ///     - Incomplete sequence (no `error_len`): stops processing to wait for the next TCP packet.
    ///
    /// # Parameters
    /// - `buffer`: Mutable reference to the byte buffer containing received data.
    /// - `text_tx`: Channel to pass successfully decoded strings to the logic core.
    fn _process_utf8_buffer(buffer: &mut Vec<u8>, text_tx: &mpsc::Sender<String>) {

        loop {
            if buffer.is_empty() {
                break;
            }   // if

            match str::from_utf8(buffer) {
                Ok(valid_str) => {
                    // The entire buffer is valid UTF-8.
                    let _ = text_tx.send(valid_str.to_string());
                    buffer.clear();
                    break;
                }
                Err(utf8_error) => {
                    let valid_len = utf8_error.valid_up_to();

                    // Extract and send the valid portion before the error.
                    if valid_len > 0 {
                        // Safety: we know this portion is valid based on `valid_up_to()`.
                        let valid_str = unsafe { str::from_utf8_unchecked(&buffer[..valid_len]) };
                        let _ = text_tx.send(valid_str.to_string());
                        buffer.drain(..valid_len);
                        continue; // Re-evaluate the rest of the buffer.
                    }   // if

                    // The error is at the very beginning of the buffer.
                    if let Some(error_len) = utf8_error.error_len() {
                        // Invalid UTF-8 bytes encountered. Discard them to recover.
                        log_wrn!("Tcp server: Dropping {} invalid UTF-8 bytes.", error_len);
                        buffer.drain(..error_len);
                        continue;
                    } else {
                        // Incomplete UTF-8 sequence at the end. Wait for more bytes.
                        log_inf!("Tcp server: принята часть UTF-8 символа в конце посылки.");
                        break;
                    }   // if
                }
            }   // match
        }   // loop
    }   // _process_utf8_buffer()
}

impl Drop for TcpServer {

    /// Waits for both background threads (listener and reader) to finish.
    ///
    /// Called automatically when `TcpServer` goes out of scope. Ensures clean
    /// thread shutdown by joining both handles before the struct is deallocated.
    fn drop(&mut self) {
        if let Some(handle) = self.listener_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.read_handle.take() {
            let _ = handle.join();
        }
        log_inf!("Tcp server threads dropped.");
    }
}
