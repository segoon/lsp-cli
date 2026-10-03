use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(5);

pub(super) fn command(role: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("fixture executable"));
    command.args(["--fixture", role]);
    command.stdin(Stdio::null()).stdout(Stdio::null());
    command
}

pub(super) fn serve(role: &str, socket: &Path, workspace: &Path) {
    nix::unistd::setsid().expect("fixture should detach from its parent's session");
    let _descendant = match role {
        "parent" => Some(
            command("branch")
                .arg(socket)
                .arg(workspace)
                .spawn()
                .expect("spawn branch"),
        ),
        "branch" => Some(
            command("leaf")
                .arg(socket)
                .arg(workspace)
                .spawn()
                .expect("spawn leaf"),
        ),
        _ => None,
    };
    // Descendants deliberately outlive this fixture. The enclosing regression runner owns reaping.
    let mut stream = UnixStream::connect(socket).expect("connect fixture control socket");
    writeln!(stream, "{role} {}", std::process::id()).expect("report fixture ready");
    let mut instruction = [0];
    while stream.read_exact(&mut instruction).is_ok() {
        match instruction[0] {
            b'x' => return,
            b'w' => {
                std::fs::write(workspace.join("writer-marker"), role).expect("write case marker");
                stream.write_all(b"y").expect("acknowledge write");
            }
            _ => panic!("unknown fixture instruction"),
        }
    }
}

pub(super) struct Fixture {
    _control: tempfile::TempDir,
    root: Child,
    peers: BTreeMap<String, (u32, UnixStream)>,
}

impl Fixture {
    pub(super) fn start(workspace: &Path, tree: bool) -> Self {
        let control = tempfile::tempdir().expect("control directory");
        let socket = control.path().join("control.sock");
        let listener = UnixListener::bind(&socket).expect("control listener");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let role = if tree { "parent" } else { "leaf" };
        let root = command(role)
            .arg(&socket)
            .arg(workspace)
            .spawn()
            .expect("spawn fixture");
        let mut peers = BTreeMap::new();
        let deadline = Instant::now() + DEADLINE;
        let expected = if tree { 3 } else { 1 };
        while peers.len() < expected {
            assert!(Instant::now() < deadline, "fixture readiness deadline");
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(DEADLINE))
                        .expect("control read deadline");
                    let mut message = String::new();
                    loop {
                        let mut byte = [0];
                        stream.read_exact(&mut byte).expect("read readiness");
                        if byte[0] == b'\n' {
                            break;
                        }
                        message.push(char::from(byte[0]));
                    }
                    let (role, pid) = message.split_once(' ').expect("readiness fields");
                    peers.insert(
                        role.to_string(),
                        (pid.parse().expect("fixture PID"), stream),
                    );
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("accept fixture: {error}"),
            }
        }
        Self {
            _control: control,
            root,
            peers,
        }
    }

    pub(super) fn orphan(&mut self) {
        self.instruct("parent", b'x');
        assert!(self.root.wait().expect("reap fixture parent").success());
    }

    pub(super) fn instruct(&mut self, role: &str, instruction: u8) {
        let (_, stream) = self.peers.get_mut(role).expect("fixture role");
        stream
            .write_all(&[instruction])
            .expect("send fixture instruction");
        if instruction == b'w' {
            let mut ack = [0];
            stream
                .read_exact(&mut ack)
                .expect("read write acknowledgement");
            assert_eq!(ack, [b'y']);
        }
    }

    pub(super) fn pid(&self, role: &str) -> u32 {
        self.peers.get(role).expect("fixture role").0
    }

    pub(super) fn assert_gone(&self) {
        for (pid, _) in self.peers.values() {
            assert!(
                !Path::new("/proc").join(pid.to_string()).exists(),
                "fixture {pid} survived cleanup"
            );
        }
    }

    pub(super) fn stop_direct_child(&mut self) {
        // Failed-cleanup scenarios deliberately retain this direct child; the fixture still owns
        // its unreaped PID, so this backstop does not race with PID reuse.
        self.root.kill().expect("kill retained fixture");
        self.root.wait().expect("reap retained fixture");
    }
}
