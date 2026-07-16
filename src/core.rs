//! Core application module.
//!
//! Responsible for orchestrating the application's processing pipeline.
//! Creates all pipeline stages, wires them together via channels,
//! and manages their lifecycle.
//!
//! # PIPELINE
//! TcpServer -> Lexer -> TextProcessor -> ScreenWriter
//!
//! Each stage runs in its own thread and communicates via `mpsc` channels.
//! An independent `UserActivityTracker` runs in a separate thread.
//!
//! # SHUTDOWN
//! Cascading channel closure: when the TCP server stops, it drops its sender,
//! which causes each downstream stage to exit its blocking `recv()` loop in sequence.

use std::sync::mpsc;
use hobolib::prntln;
use crate::lexeme_transfer::LexemeTransfer;
use crate::lexer::Lexer;
use crate::log_inf;
use crate::screen_writer::ScreenWriter;
use crate::tcp_server::TcpServer;
use crate::text_processor::TextProcessor;
use crate::user_activity_tracker::UserActivityTracker;
use crate::screen_transfer::ScreenTransfer;

pub struct Core {
    // Pipeline stages are stored in order.
    // They will be dropped in reverse field order (Rust guarantees this),
    // but actual shutdown is driven by cascading channel closure, not drop order.
    _tcp_server: TcpServer,
    _lexer: Lexer,
    _text_processor: TextProcessor,
    _screen_writer: ScreenWriter,
    _user_activity_tracker: UserActivityTracker,
}

impl Core {

    /// Constructor.
    ///
    /// Creates the full processing pipeline:
    /// 1. Allocates three `mpsc` channels to connect the stages.
    /// 2. Spawns all pipeline stages, each in its own thread.
    /// 3. Spawns the independent `UserActivityTracker`.
    pub fn new() -> Self {

        // Channel: TcpServer -> Lexer (carries raw text chunks).
        let (text_tx, text_rx) = mpsc::channel::<String>();

        // Channel: Lexer -> TextProcessor (carries parsed lexemes).
        let (lexeme_transfer_tx, lexeme_transfer_rx) = 
            mpsc::channel::<LexemeTransfer>();

        // Channel: TextProcessor -> ScreenWriter (carries output commands for focused window).
        let (screen_transfer_tx, screen_transfer_rx) = 
            mpsc::channel::<ScreenTransfer>();

        // Spawn pipeline stages in forward order.
        let tcp_server = TcpServer::new(text_tx);
        let lexer = Lexer::new(text_rx, lexeme_transfer_tx);
        let text_processor = TextProcessor::new(lexeme_transfer_rx, screen_transfer_tx);
        let screen_writer = ScreenWriter::new(screen_transfer_rx);

        // Spawn independent tracker.
        let user_activity_tracker = UserActivityTracker::new();

        log_inf!("Core: pipeline started");

        Core {
            _tcp_server: tcp_server,
            _lexer: lexer,
            _text_processor: text_processor,
            _screen_writer: screen_writer,
            _user_activity_tracker: user_activity_tracker,
        }
    }   // new()

}   // impl Core

impl Drop for Core {

    /// Destructor.
    /// Pipeline stages are dropped in reverse field order.
    /// Actual thread termination is handled by each stage's own `Drop` impl.
    fn drop(&mut self) {
        prntln!("Core dropped");
    }

}   // impl Drop for Core
