//! The daemon's end of the control socket.
//!
//! Each connection gets a thread that parses request lines and hands them
//! to the daemon thread as messages, then writes back the answer. The
//! daemon thread stays the only owner of state; these threads only relay.

use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender, SyncSender};
use std::thread;

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{Listener, Stream};
use nexus_control::{Request, reply, write_frame};
use serde_json::json;

use crate::run::Msg;

/// Frames a slow viewer may fall behind by before frames are dropped for
/// it. Dropping keeps the daemon from ever waiting on a viewer.
const FRAME_QUEUE: usize = 2;

pub type Frame = Arc<[u8]>;
pub type Reply = Result<serde_json::Value, String>;

/// Accepts connections on a background thread.
pub fn serve(listener: Listener, tx: Sender<Msg>) {
    thread::Builder::new()
        .name("control".into())
        .spawn(move || {
            for stream in listener.incoming().filter_map(Result::ok) {
                let tx = tx.clone();
                let _ = thread::Builder::new()
                    .name("control-conn".into())
                    .spawn(move || connection(&stream, &tx));
            }
        })
        .expect("spawning a thread");
}

fn connection(stream: &Stream, tx: &Sender<Msg>) {
    let mut out = stream;
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { return };
        let answer = match Request::parse(&line) {
            Err(e) => reply(Err(e)),
            Ok(Request::Frames) => {
                let (frames_tx, frames_rx) = mpsc::sync_channel(FRAME_QUEUE);
                if tx.send(Msg::Subscribe(frames_tx)).is_err() {
                    return;
                }
                let hello = reply(Ok(json!({ "width": 640, "height": 48 })));
                if writeln!(out, "{hello}").is_err() {
                    return;
                }
                // From here on the connection carries only frames.
                for frame in frames_rx {
                    if write_frame(&mut out, &frame).is_err() {
                        return;
                    }
                }
                return;
            }
            Ok(request) => {
                let (reply_tx, reply_rx) = mpsc::channel();
                if tx.send(Msg::Control(request, reply_tx)).is_err() {
                    return;
                }
                match reply_rx.recv() {
                    Ok(r) => reply(r),
                    Err(_) => return,
                }
            }
        };
        if writeln!(out, "{answer}").is_err() {
            return;
        }
    }
}

/// Sends `frame` to every viewer, forgetting those that have gone.
pub fn broadcast(viewers: &mut Vec<SyncSender<Frame>>, frame: &Frame) {
    viewers.retain(|v| match v.try_send(frame.clone()) {
        Ok(()) | Err(mpsc::TrySendError::Full(_)) => true,
        Err(mpsc::TrySendError::Disconnected(_)) => false,
    });
}
