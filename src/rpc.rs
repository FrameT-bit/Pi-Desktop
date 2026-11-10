
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::state::{path_with_pi, Connection, Transport};

pub enum RpcEvent {
    Conn(Connection),
    Record(Value),
    Stderr(String),
}

pub struct RpcHandle {
    cmd_tx: Sender<Msg>,
    pub events: async_channel::Receiver<RpcEvent>,
}

enum Msg {
    Send(Value),
    Restart(Transport),
}

impl RpcHandle {
    pub fn send(&self, value: Value) {
        let _ = self.cmd_tx.send(Msg::Send(value));
    }

    pub fn restart(&self, transport: Transport) {
        let _ = self.cmd_tx.send(Msg::Restart(transport));
    }
}

pub fn spawn(transport: Transport) -> RpcHandle {
    let (cmd_tx, cmd_rx) = mpsc::channel::<Msg>();
    let (ev_tx, ev_rx) = async_channel::unbounded::<RpcEvent>();

    thread::Builder::new()
        .name("pi-rpc".into())
        .spawn(move || supervisor(cmd_rx, ev_tx, transport))
        .expect("spawn thread pi-rpc");

    RpcHandle {
        cmd_tx,
        events: ev_rx,
    }
}

fn build_command(transport: &Transport) -> Result<Command, String> {
    match transport {
        Transport::Local(local) => {
            let Some(bin) = local.bin.as_deref() else {
                return Err("not found below".to_string());
            };
            let mut command = Command::new(bin);
            command.current_dir(&local.workspace);
            command.args(local.args());
            command.env("PATH", path_with_pi(bin));
            command.env("PI_CODING_AGENT_DIR", crate::config::pi_agent_dir());
            Ok(command)
        }
        Transport::Remote { target, command } => {
            let mut process = Command::new("ssh");
            process
                .arg("-T")
                .arg("-o")
                .arg("BatchMode=yes")
                .arg("-o")
                .arg("ServerAliveInterval=15")
                .arg("-o")
                .arg("ServerAliveCountMax=3")
                .arg("-o")
                .arg("ConnectTimeout=8")
                .arg(target)
                .arg(command);
            Ok(process)
        }
        Transport::Missing { expected } => {
            Err(format!("not found: {expected}"))
        }
    }
}

fn next_backoff(seconds: u64) -> u64 {
    (seconds * 2).min(8)
}

fn supervisor(
    cmd_rx: Receiver<Msg>,
    ev: async_channel::Sender<RpcEvent>,
    mut transport: Transport,
) {
    let mut restart_transport: Option<Transport>;
    let mut backoff = 1u64;

    loop {
        if ev.is_closed() {
            return;
        }

        if let Transport::Missing { expected } = &transport {
            let _ = ev.send_blocking(RpcEvent::Conn(Connection::Disconnected(format!(
                "not found: {expected}"
            ))));
            loop {
                if ev.is_closed() {
                    return;
                }
                match cmd_rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(Msg::Restart(next)) => {
                        transport = next;
                        break;
                    }
                    Ok(Msg::Send(_)) => {}
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            continue;
        }

        let _ = ev.send_blocking(RpcEvent::Conn(Connection::Connecting));

        let spawned = build_command(&transport)
            .and_then(|mut command| {
                command
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|err| format!("Pi process: {err}"))
            });

        let mut child = match spawned {
            Ok(child) => child,
            Err(err) => {
                let _ = ev.send_blocking(RpcEvent::Conn(Connection::Disconnected(err)));
                thread::sleep(Duration::from_secs(backoff));
                backoff = next_backoff(backoff);
                continue;
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let mut stdin = child.stdin.take();

        let (line_tx, line_rx) = mpsc::channel::<Value>();

        if let Some(stdout) = stdout {
            thread::Builder::new()
                .name("pi-rpc-out".into())
                .spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        let line = line.trim_end_matches('\r');
                        if line.trim().is_empty() {
                            continue;
                        }
                        if let Ok(value) = serde_json::from_str::<Value>(line) {
                            if line_tx.send(value).is_err() {
                                break;
                            }
                        }
                    }
                })
                .ok();
        }

        if let Some(stderr) = stderr {
            let ev_err = ev.clone();
            thread::Builder::new()
                .name("pi-rpc-err".into())
                .spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        let line = line.trim().to_string();
                        if !line.is_empty() {
                            let _ = ev_err.send_blocking(RpcEvent::Stderr(line));
                        }
                    }
                })
                .ok();
        }

        let started = Instant::now();
        let mut first: Option<Value> = None;
        let mut alive = true;
        while started.elapsed() < Duration::from_millis(800) {
            match line_rx.recv_timeout(Duration::from_millis(25)) {
                Ok(value) => {
                    first = Some(value);
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    alive = false;
                    break;
                }
            }
            if matches!(child.try_wait(), Ok(Some(_))) {
                alive = false;
                break;
            }
        }
        if alive {
            alive = matches!(child.try_wait(), Ok(None));
        }

        if alive {
            backoff = 1;
            let _ = ev.send_blocking(RpcEvent::Conn(Connection::Connected));
        }

        if let Some(value) = first {
            let _ = ev.send_blocking(RpcEvent::Record(value));
        }

        let mut pending: Vec<Value> = Vec::new();
        restart_transport = None;
        loop {
            if ev.is_closed() {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }

            match line_rx.recv_timeout(Duration::from_millis(20)) {
                Ok(value) => {
                    let _ = ev.send_blocking(RpcEvent::Record(value));
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {}
            }

            while let Ok(msg) = cmd_rx.try_recv() {
                match msg {
                    Msg::Send(cmd) => pending.push(cmd),
                    Msg::Restart(next) => {
                        restart_transport = Some(next);
                        break;
                    }
                }
            }

            if restart_transport.is_some() {
                pending.clear();
                break;
            }

            if let Some(sink) = stdin.as_mut() {
                while let Some(cmd) = pending.first() {
                    let mut line = serde_json::to_string(cmd).unwrap_or_default();
                    line.push('\n');
                    if sink.write_all(line.as_bytes()).is_err() || sink.flush().is_err() {
                        break;
                    }
                    pending.remove(0);
                }
            }

            if let Ok(Some(_status)) = child.try_wait() {
                break;
            }
        }

        if restart_transport.is_some() {
            let _ = child.kill();
        }
        drop(stdin.take());
        let _ = child.wait();

        while let Ok(value) = line_rx.try_recv() {
            let _ = ev.send_blocking(RpcEvent::Record(value));
        }

        if let Some(next) = restart_transport.take() {
            transport = next;
            backoff = 1;
            continue;
        }

        let _ = ev.send_blocking(RpcEvent::Conn(Connection::Disconnected(
            "closed connection".to_string(),
        )));

        thread::sleep(Duration::from_secs(backoff));
        backoff = next_backoff(backoff);
    }
}
