//! Optional Discord RPC. Only explicit home/playing calls publish an activity.
//! The UI only updates a latest-value mailbox; all IPC happens on one worker.
//! Dropping Presence disconnects the wake queue. The worker closes its pipe as
//! soon as its current bounded transaction ends (Drop never joins on the UI).

#[cfg(any(windows, test))]
use serde_json::{json, Value};

pub struct Presence {
    #[cfg(windows)]
    wake: Option<std::sync::mpsc::SyncSender<()>>,
    #[cfg(windows)]
    latest: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

pub fn start() -> Presence {
    #[cfg(windows)]
    {
        use std::sync::{mpsc, Arc, Mutex};
        let (wake, receiver) = mpsc::sync_channel(1);
        let latest = Arc::new(Mutex::new(None));
        let mailbox = Arc::clone(&latest);
        // Spawn failure, like Discord absence, must not prevent launcher use.
        let spawned = std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || windows::worker(receiver, mailbox));
        Presence {
            wake: spawned.ok().map(|_| wake),
            latest,
        }
    }
    #[cfg(not(windows))]
    {
        Presence {}
    }
}

impl Presence {
    /// Associated convenience constructor; the free start() is also exported.
    pub fn start() -> Self {
        start()
    }

    pub fn playing(&self, title: &str) {
        self.publish(&format!("Playing {title}"));
    }

    pub fn home(&self) {
        self.publish("In the home");
    }

    fn publish(&self, state: &str) {
        #[cfg(windows)]
        if let Some(wake) = &self.wake {
            if let Ok(mut latest) = self.latest.lock() {
                // Discord limits state to 128 characters. Preserve UTF-8.
                *latest = Some(state.chars().take(128).collect());
            }
            // Full means a wake is already pending; the newest value wins.
            let _ = wake.try_send(());
        }
        #[cfg(not(windows))]
        let _ = state;
    }
}

impl Drop for Presence {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            // Discard pending activity, then wake a blocked recv by disconnecting.
            if let Ok(mut latest) = self.latest.lock() {
                *latest = None;
            }
            self.wake.take();
        }
    }
}

#[cfg(any(windows, test))]
fn activity(state: &str, timestamp: u64, nonce: u64) -> Value {
    json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": std::process::id(),
            "activity": {
                "details": "Private Mewing Launcher",
                "state": state,
                "assets": { "large_image": "main_logo", "large_text": "Mewing Launcher" },
                "timestamps": { "start": timestamp }
            }
        },
        "nonce": nonce.to_string()
    })
}

#[cfg(any(windows, test))]
fn frame(opcode: u32, payload: &[u8]) -> std::io::Result<Vec<u8>> {
    let len = u32::try_from(payload.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "RPC frame too large"))?;
    let mut bytes = Vec::with_capacity(8 + payload.len());
    bytes.extend_from_slice(&opcode.to_le_bytes());
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

#[cfg(windows)]
mod windows {
    use super::{activity, frame};
    use serde_json::{json, Value};
    use std::ffi::c_void;
    use std::io::{self, ErrorKind};
    use std::ptr::{null, null_mut};
    use std::sync::{mpsc::Receiver, Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    type Handle = *mut c_void;
    const INVALID_HANDLE: Handle = -1isize as Handle;
    const IO_PENDING: i32 = 997;
    const MAX_FRAME: usize = 64 * 1024;
    const TRANSACTION_TIMEOUT: Duration = Duration::from_millis(500);

    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        event: Handle,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(name: *const u16, access: u32, share: u32, security: *const c_void,
            creation: u32, flags: u32, template: Handle) -> Handle;
        fn CreateEventW(security: *const c_void, manual_reset: i32, initial: i32,
            name: *const u16) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn ReadFile(handle: Handle, buffer: *mut c_void, size: u32,
            transferred: *mut u32, overlapped: *mut Overlapped) -> i32;
        fn WriteFile(handle: Handle, buffer: *const c_void, size: u32,
            transferred: *mut u32, overlapped: *mut Overlapped) -> i32;
        fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
        fn CancelIoEx(handle: Handle, overlapped: *mut Overlapped) -> i32;
        fn GetOverlappedResult(handle: Handle, overlapped: *mut Overlapped,
            transferred: *mut u32, wait: i32) -> i32;
    }

    struct OwnedHandle(Handle);
    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: each wrapper owns one valid handle, closed exactly once.
            unsafe { CloseHandle(self.0); }
        }
    }

    struct Pipe(OwnedHandle);
    impl Pipe {
        fn connect() -> io::Result<Self> {
            for index in 0..10 {
                let name: Vec<u16> = format!(r"\\.\pipe\discord-ipc-{index}")
                    .encode_utf16().chain(Some(0)).collect();
                // No WaitNamedPipe/retry loop: absent or busy Discord is optional.
                // SAFETY: name is NUL terminated; all other pointers are null.
                let handle = unsafe {
                    CreateFileW(name.as_ptr(), 0xC0000000, 0, null(), 3,
                        0x40000000, null_mut()) // read/write, OPEN_EXISTING, OVERLAPPED
                };
                if handle != INVALID_HANDLE {
                    return Ok(Self(OwnedHandle(handle)));
                }
            }
            Err(io::Error::new(ErrorKind::NotConnected, "Discord is unavailable"))
        }

        fn transfer(&self, buffer: &mut [u8], write: bool, deadline: Instant) -> io::Result<()> {
            let mut offset = 0;
            while offset < buffer.len() {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(io::Error::new(ErrorKind::TimedOut, "Discord IPC deadline"));
                }
                // SAFETY: null security/name are permitted; this event is owned locally.
                let event = unsafe { CreateEventW(null(), 1, 0, null()) };
                if event.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let event = OwnedHandle(event);
                let mut op = Overlapped {
                    internal: 0, internal_high: 0, offset: 0, offset_high: 0, event: event.0,
                };
                let mut count = 0;
                let size = u32::try_from(buffer.len() - offset)
                    .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "RPC buffer too large"))?;
                // SAFETY: buffer and OVERLAPPED remain alive until completion or
                // cancellation has completed. One operation is outstanding at a time.
                let result = unsafe {
                    if write {
                        WriteFile(self.0.0, buffer[offset..].as_ptr().cast(), size, null_mut(), &mut op)
                    } else {
                        ReadFile(self.0.0, buffer[offset..].as_mut_ptr().cast(), size, null_mut(), &mut op)
                    }
                };
                if result == 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(IO_PENDING) {
                        return Err(error);
                    }
                    let milliseconds = deadline.saturating_duration_since(Instant::now())
                        .as_millis().min(u32::MAX as u128 - 1) as u32;
                    // Event wait, not polling. Cancel stalled operations at the deadline.
                    let wait = unsafe { WaitForSingleObject(event.0, milliseconds) };
                    if wait != 0 {
                        // SAFETY: cancel and drain before releasing op/buffer/event.
                        unsafe {
                            CancelIoEx(self.0.0, &mut op);
                            GetOverlappedResult(self.0.0, &mut op, &mut count, 1);
                        }
                        return Err(io::Error::new(ErrorKind::TimedOut, "Discord IPC stalled"));
                    }
                }
                // SAFETY: immediate completion or the signalled event means no wait.
                if unsafe { GetOverlappedResult(self.0.0, &mut op, &mut count, 0) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                if count == 0 {
                    return Err(io::Error::new(ErrorKind::UnexpectedEof, "Discord IPC closed"));
                }
                offset += count as usize;
            }
            Ok(())
        }

        fn send(&self, opcode: u32, payload: &[u8], deadline: Instant) -> io::Result<()> {
            self.transfer(&mut frame(opcode, payload)?, true, deadline)
        }

        fn receive(&self, deadline: Instant) -> io::Result<(u32, Vec<u8>)> {
            let mut header = [0; 8];
            self.transfer(&mut header, false, deadline)?;
            let opcode = u32::from_le_bytes(header[..4].try_into().unwrap());
            let size = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
            if size > MAX_FRAME {
                return Err(io::Error::new(ErrorKind::InvalidData, "Oversized Discord frame"));
            }
            let mut payload = vec![0; size];
            self.transfer(&mut payload, false, deadline)?;
            Ok((opcode, payload))
        }

        fn response(&self, nonce: Option<&str>, deadline: Instant) -> io::Result<()> {
            // Handle protocol ping/pong, but cap unrelated frames and total IO time.
            for _ in 0..8 {
                let (opcode, bytes) = self.receive(deadline)?;
                match opcode {
                    1 => {
                        let value: Value = serde_json::from_slice(&bytes)?;
                        if value["evt"] == "ERROR" {
                            return Err(io::Error::new(ErrorKind::InvalidData, "Discord rejected RPC"));
                        }
                        if let Some(nonce) = nonce {
                            if value["nonce"].as_str() == Some(nonce) && value["cmd"] == "SET_ACTIVITY" {
                                return Ok(());
                            }
                        } else if value["evt"] == "READY" {
                            return Ok(());
                        }
                    }
                    3 => self.send(4, &bytes, deadline)?,
                    4 => {}
                    _ => return Err(io::Error::new(ErrorKind::ConnectionAborted, "Discord closed RPC")),
                }
            }
            Err(io::Error::new(ErrorKind::InvalidData, "No matching Discord response"))
        }

        fn handshake(&self) -> io::Result<()> {
            let deadline = Instant::now() + TRANSACTION_TIMEOUT;
            let payload = serde_json::to_vec(&json!({
                "v": 1, "client_id": "1386513637031678042"
            }))?;
            self.send(0, &payload, deadline)?;
            self.response(None, deadline)
        }

        fn update(&self, state: &str, nonce: u64) -> io::Result<()> {
            let deadline = Instant::now() + TRANSACTION_TIMEOUT;
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
                .unwrap_or_default().as_secs();
            let payload = serde_json::to_vec(&activity(state, timestamp, nonce))?;
            self.send(1, &payload, deadline)?;
            self.response(Some(&nonce.to_string()), deadline)
        }
    }

    pub(super) fn worker(receiver: Receiver<()>, latest: Arc<Mutex<Option<String>>>) {
        let mut pipe: Option<Pipe> = None;
        let mut nonce = 0u64;
        while receiver.recv().is_ok() {
            let state = latest.lock().ok().and_then(|mut value| value.take());
            let Some(state) = state else { continue };
            if pipe.is_none() {
                pipe = Pipe::connect().ok().and_then(|candidate| {
                    candidate.handshake().ok().map(|_| candidate)
                });
            }
            nonce = nonce.wrapping_add(1);
            if let Some(connection) = &pipe {
                if connection.update(&state, nonce).is_err() {
                    // Never retry/poll now. The next activity tries a fresh pipe.
                    pipe = None;
                }
            }
        }
        // OwnedHandle closes the connection here, removing this process's activity.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_use_little_endian_opcode_and_byte_length() {
        let payload = "ñ".as_bytes();
        let bytes = frame(1, payload).unwrap();
        assert_eq!(&bytes[..8], &[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&bytes[8..], payload);
        assert_eq!(frame(0, b"").unwrap(), vec![0; 8]);
    }

    #[test]
    fn activity_has_original_branding_and_rpc_shape() {
        for state in ["In the home", "Playing Minecraft"] {
            let value = activity(state, 1234, 7);
            assert_eq!(value["cmd"], "SET_ACTIVITY");
            assert_eq!(value["nonce"], "7");
            assert_eq!(value["args"]["pid"], std::process::id());
            let activity = &value["args"]["activity"];
            assert_eq!(activity["details"], "Private Mewing Launcher");
            assert_eq!(activity["state"], state);
            assert_eq!(activity["timestamps"]["start"], 1234);
            assert_eq!(activity["assets"]["large_image"], "main_logo");
            assert_eq!(activity["assets"]["large_text"], "Mewing Launcher");
        }
    }
}
