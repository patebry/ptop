//! Compatibility with vtop 0.6.1 upgrade.js. Effects only run after explicit `u`.
use std::process::Command;

pub fn check() -> Option<String> {
    let output = Command::new("npm")
        .args(["info", "--json", "vtop"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    latest(&output.stdout)
}

fn latest(bytes: &[u8]) -> Option<String> {
    let metadata: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let version = metadata.get("dist-tags")?.get("latest")?.as_str()?;
    (version != "0.6.1" && !version.is_empty()).then(|| version.to_owned())
}

/// Output, password challenges and restart conditions follow upgrade.js/node-sudo.
pub fn install(theme: &str) -> i32 {
    use std::io::Read;
    use std::process::Stdio;
    println!("\nInstalling vtop update...\n\n ** You will need to enter your password to upgrade ** \n\nnpm install -g vtop");
    let mut child = match Command::new("sudo")
        .args([
            "-S",
            "-p",
            "#node-sudo-passwd#",
            "npm",
            "install",
            "-g",
            "vtop",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let streams: Vec<(bool, Box<dyn Read + Send>)> = vec![
        (true, Box::new(child.stdout.take().unwrap())),
        (false, Box::new(child.stderr.take().unwrap())),
    ];
    let workers: Vec<_> = streams
        .into_iter()
        .map(|(stdout, mut stream)| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut buffer = [0; 8192];
                loop {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx.send((stdout, buffer[..n].to_vec())).is_err() {
                                break;
                            }
                        }
                    }
                }
            })
        })
        .collect();
    drop(tx);
    let mut module = ModulePath::Absent;
    for (stdout, bytes) in rx {
        let chunk = String::from_utf8_lossy(&bytes);
        let challenge = !stdout
            && chunk
                .trim()
                .split('\n')
                .any(|line| line == "#node-sudo-passwd#");
        let mut password = challenge.then(PasswordInput::start);
        println!("{chunk}");
        if let Some(input) = password.as_mut() {
            use std::io::Write;
            if let Some(mut answer) = input.read() {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(&answer);
                    let _ = stdin.write_all(b"\n");
                }
                answer.fill(0);
            }
        }
        drop(password);
        if stdout {
            module.observe(&chunk);
        }
    }
    for worker in workers {
        let _ = worker.join();
    }
    let _ = child.wait();
    println!("Finished updating. Clearing cache and relaunching...");
    std::thread::sleep(std::time::Duration::from_secs(1));
    let mut argv: Vec<String> = std::env::args()
        .filter(|arg| !matches!(arg.as_str(), "--fix" | "--vtop-parity"))
        .collect();
    if let Ok(executable) = std::env::current_exe() {
        argv[0] = executable.to_string_lossy().into_owned();
    }
    let state = serde_json::json!({
        "argv": argv,
        "theme": theme,
        "module": match &module { ModulePath::Path(path) => serde_json::json!(path), _ => serde_json::json!(false) },
        "undefined": matches!(module, ModulePath::Undefined),
    });
    Command::new("node")
        .args(["-e", include_str!("restart.js"), &state.to_string()])
        .status()
        .ok()
        .and_then(|status| status.code())
        .unwrap_or(1)
}
// node-read uses libuv's raw input mode while retaining output processing.
// Keep the original terminal state even if input is canceled or disconnected.
struct PasswordInput {
    saved: Option<libc::termios>,
}
impl PasswordInput {
    fn start() -> Self {
        use std::io::Write;
        let mut saved = std::mem::MaybeUninit::<libc::termios>::uninit();
        let saved = unsafe {
            if libc::tcgetattr(0, saved.as_mut_ptr()) == 0 {
                let saved = saved.assume_init();
                let mut raw = saved;
                raw.c_iflag &=
                    !(libc::BRKINT | libc::ICRNL | libc::INPCK | libc::ISTRIP | libc::IXON);
                raw.c_cflag |= libc::CS8;
                raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);
                raw.c_cc[libc::VMIN] = 1;
                raw.c_cc[libc::VTIME] = 0;
                libc::tcsetattr(0, libc::TCSANOW, &raw);
                Some(saved)
            } else {
                None
            }
        };
        if saved.is_some() {
            let mut size = std::mem::MaybeUninit::<libc::winsize>::uninit();
            let columns = unsafe {
                if libc::ioctl(1, libc::TIOCGWINSZ, size.as_mut_ptr()) == 0 {
                    size.assume_init().ws_col
                } else {
                    0
                }
            };
            if columns > 0 {
                print!("\x1b[1G\x1b[0JPassword: \x1b[11G");
            } else {
                print!("Password: ");
            }
        } else {
            print!("Password: ");
        }
        let _ = std::io::stdout().flush();
        Self { saved }
    }
    fn read(&mut self) -> Option<Vec<u8>> {
        use std::io::Write;
        let mut answer = Vec::new();
        let mut cursor = 0;
        let mut byte = [0];
        loop {
            if !read_input_byte(&mut byte) {
                return Some(b"undefined".to_vec());
            }
            match byte[0] {
                b'\r' | b'\n' => {
                    if self.saved.is_some() {
                        let _ = std::io::stdout().write_all(b"\r\n");
                        let _ = std::io::stdout().flush();
                    }
                    return Some(answer);
                }
                3 => {
                    answer.fill(0);
                    return Some(b"undefined".to_vec());
                }
                1 => cursor = 0,
                5 => cursor = answer.len(),
                2 => cursor = previous_character(&answer, cursor),
                6 => cursor = next_character(&answer, cursor),
                8 | 127 if cursor > 0 => {
                    let previous = previous_character(&answer, cursor);
                    answer.drain(previous..cursor);
                    cursor = previous;
                }
                4 if answer.is_empty() => return None,
                4 if cursor < answer.len() => {
                    let next = next_character(&answer, cursor);
                    answer.drain(cursor..next);
                }
                27 => {
                    let sequence = read_escape_sequence();
                    match sequence.as_slice() {
                        b"[D" | b"OD" => cursor = previous_character(&answer, cursor),
                        b"[C" | b"OC" => cursor = next_character(&answer, cursor),
                        b"[H" | b"OH" | b"[1~" => cursor = 0,
                        b"[F" | b"OF" | b"[4~" => cursor = answer.len(),
                        b"[3~" if cursor < answer.len() => {
                            let next = next_character(&answer, cursor);
                            answer.drain(cursor..next);
                        }
                        _ => {}
                    }
                }
                21 => {
                    answer.drain(..cursor);
                    cursor = 0;
                }
                11 => {
                    answer.truncate(cursor);
                }
                23 => {
                    let start = word_left_start(&answer, cursor);
                    answer.drain(start..cursor);
                    cursor = start;
                }
                0..=31 | 127 => {}
                value => {
                    answer.insert(cursor, value);
                    cursor += 1;
                }
            }
        }
    }
}
fn word_left_start(bytes: &[u8], cursor: usize) -> usize {
    // Node readline: reverse, then /^\s*(?:[^\w\s]+|\w+)?/.
    let prefix = String::from_utf8_lossy(&bytes[..cursor]);
    let mut chars = prefix.char_indices().rev().peekable();
    let mut start = cursor;
    while let Some(&(offset, ch)) = chars.peek() {
        if !ch.is_whitespace() {
            break;
        }
        start = offset;
        chars.next();
    }
    let word = chars
        .peek()
        .is_some_and(|(_, ch)| ch.is_ascii_alphanumeric() || *ch == '_');
    for (offset, ch) in chars {
        if ch.is_whitespace() || (ch.is_ascii_alphanumeric() || ch == '_') != word {
            break;
        }
        start = offset;
    }
    start
}
fn read_input_byte(byte: &mut [u8; 1]) -> bool {
    // Use the descriptor directly: std::Stdin's buffered read can hide escape
    // bytes from poll, incorrectly turning an already received sequence into Escape.
    loop {
        let count = unsafe { libc::read(0, byte.as_mut_ptr().cast(), 1) };
        if count >= 0 {
            return count == 1;
        }
        if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return false;
        }
    }
}
fn read_escape_sequence() -> Vec<u8> {
    let mut sequence = Vec::new();
    let mut byte = [0];
    loop {
        let mut fd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut fd, 1, 500) } <= 0 || !read_input_byte(&mut byte) {
            break;
        }
        sequence.push(byte[0]);
        if (sequence.len() == 1 && !matches!(byte[0], b'[' | b'O'))
            || sequence.len() > 8
            || (sequence.len() > 1 && (byte[0].is_ascii_alphabetic() || byte[0] == b'~'))
        {
            break;
        }
    }
    sequence
}
fn previous_character(bytes: &[u8], cursor: usize) -> usize {
    let mut previous = cursor.saturating_sub(1);
    while previous > 0 && bytes[previous] & 0xc0 == 0x80 {
        previous -= 1;
    }
    previous
}
fn next_character(bytes: &[u8], cursor: usize) -> usize {
    let mut next = (cursor + 1).min(bytes.len());
    while next < bytes.len() && bytes[next] & 0xc0 == 0x80 {
        next += 1;
    }
    next
}
impl Drop for PasswordInput {
    fn drop(&mut self) {
        if let Some(saved) = &self.saved {
            unsafe {
                libc::tcsetattr(0, libc::TCSANOW, saved);
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum ModulePath {
    Absent,
    Undefined,
    Path(String),
}
impl ModulePath {
    fn observe(&mut self, chunk: &str) {
        if chunk.contains("vtop.js") {
            *self = chunk
                .trim()
                .split(' ')
                .nth(2)
                .map_or(Self::Undefined, |path| Self::Path(path.into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notice_matches_upstream_inequality_not_semver_order() {
        assert_eq!(latest(br#"{"dist-tags":{"latest":"0.6.1"}}"#), None);
        assert_eq!(
            latest(br#"{"dist-tags":{"latest":"0.5.0"}}"#),
            Some("0.5.0".into())
        );
        assert_eq!(
            latest(br#"{"dist-tags":{"latest":"9.0.0"}}"#),
            Some("9.0.0".into())
        );
        assert_eq!(latest(b"bad metadata"), None);
    }
    #[test]
    fn module_parser_matches_original_updater_trace() {
        let traces: serde_json::Value =
            serde_json::from_str(include_str!("../tests/upstream-upgrade.json")).unwrap();
        for case in traces.as_array().unwrap() {
            let mut path = ModulePath::Absent;
            for log in case["logs"].as_array().unwrap() {
                path.observe(log[1].as_str().unwrap().trim_end_matches('\n'));
            }
            let expected = &case["effects"].as_array().unwrap().last().unwrap()[2];
            let actual = match path {
                ModulePath::Absent => serde_json::json!(false),
                ModulePath::Undefined => serde_json::json!("undefined"),
                ModulePath::Path(p) => serde_json::json!(p),
            };
            assert_eq!(&actual, expected, "{}", case["name"]);
        }
    }
}
