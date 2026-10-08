//! Read RSS without asking `ps` to collect every process's thread information.
//!
//! `PROC_PIDTASKINFO` and Apple ps's `TASK_BASIC_INFO` read the same XNU
//! `phys_mem` ledger. libproc restricts other users' processes, so those PIDs
//! still need the system ps. Any uncertain or incomplete result asks the caller
//! to use its complete ps collector instead of publishing an undercount.

use std::collections::BTreeSet;
use std::io;
use std::mem::{size_of, MaybeUninit};

const MAX_PID_CAPACITY: usize = 1_048_576;
const MAX_CENSUS_ATTEMPTS: usize = 4;
// <sys/proc_info.h>; libc exposes proc_listpids but not this selector.
const PROC_ALL_PIDS: u32 = 1;
// Leave room for the executable and other arguments below macOS's ARG_MAX.
const MAX_PID_ARGUMENT_BYTES: usize = 64 * 1024 - 64;

#[derive(Debug)]
pub(crate) struct Snapshot {
    pub(crate) native_rss_kib: u64,
    pub(crate) denied_pids: Vec<i32>,
}

impl Snapshot {
    /// The caller must skip ps when `denied_pids` is empty.
    pub(crate) fn fallback_args(&self) -> io::Result<Vec<String>> {
        if self.denied_pids.is_empty() {
            return Err(invalid("no PIDs need the memory fallback"));
        }
        let mut ids = String::new();
        for pid in &self.denied_pids {
            if *pid <= 0 {
                return Err(invalid("invalid memory fallback PID"));
            }
            let text = pid.to_string();
            if ids.len() + text.len() + 1 > MAX_PID_ARGUMENT_BYTES {
                return Err(invalid("memory fallback PID argument is too large"));
            }
            if !ids.is_empty() {
                ids.push(',');
            }
            ids.push_str(&text);
        }
        // -x avoids expensive terminal lookups for unselected processes. -a
        // would include every process and must not accompany this PID filter.
        Ok(vec![
            "-x".into(),
            "-p".into(),
            ids,
            "-o".into(),
            "pid=,rss=".into(),
        ])
    }

    /// Merge only a successful system ps result. The caller checks its status.
    pub(crate) fn total_rss_kib(&self, output: &[u8]) -> io::Result<u64> {
        self.total_with_census(output, pid_census)
    }

    fn total_with_census(
        &self,
        output: &[u8],
        census: impl FnOnce() -> io::Result<Vec<i32>>,
    ) -> io::Result<u64> {
        let text = std::str::from_utf8(output)
            .map_err(|_| invalid("memory fallback output is not UTF-8"))?;
        let mut missing: BTreeSet<i32> = self.denied_pids.iter().copied().collect();
        if missing.len() != self.denied_pids.len() || missing.iter().any(|pid| *pid <= 0) {
            return Err(invalid("invalid memory fallback PID set"));
        }
        let mut total = self.native_rss_kib;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let (pid, rss) = parse_row(line)?;
            if !missing.remove(&pid) {
                return Err(invalid("unexpected or repeated memory fallback PID"));
            }
            total = add_rss(total, rss)?;
        }
        if !missing.is_empty() {
            // A missing row is safe only when a new complete census positively
            // confirms that its process exited. EPERM is never treated as exit.
            let live = census()?;
            if live.iter().any(|pid| missing.contains(pid)) {
                return Err(invalid("memory fallback omitted a live process"));
            }
        }
        Ok(total)
    }
}

pub(crate) fn sample() -> io::Result<Snapshot> {
    sample_pids(pid_census()?, task_rss)
}

#[derive(Debug, PartialEq)]
enum TaskRss {
    Resident(u64),
    Denied,
    Exited,
}

fn sample_pids(
    pids: Vec<i32>,
    mut read: impl FnMut(i32) -> io::Result<TaskRss>,
) -> io::Result<Snapshot> {
    let mut snapshot = Snapshot {
        native_rss_kib: 0,
        denied_pids: Vec::new(),
    };
    for pid in pids {
        match read(pid)? {
            // ps truncates each process independently to KiB before summing.
            TaskRss::Resident(bytes) => {
                snapshot.native_rss_kib = add_rss(snapshot.native_rss_kib, bytes / 1024)?;
            }
            TaskRss::Denied => snapshot.denied_pids.push(pid),
            TaskRss::Exited => {}
        }
    }
    Ok(snapshot)
}

fn task_rss(pid: i32) -> io::Result<TaskRss> {
    let mut info = MaybeUninit::<libc::proc_taskinfo>::uninit();
    // The SDK-backed libc type has the layout expected by PROC_PIDTASKINFO.
    // It is read only after libproc confirms the complete structure was written.
    let bytes = unsafe {
        *libc::__error() = 0;
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            size_of::<libc::proc_taskinfo>() as i32,
        )
    };
    let errno = io::Error::last_os_error().raw_os_error().unwrap_or(0);
    if let Some(unavailable) = unavailable_task(bytes, errno)? {
        return Ok(unavailable);
    }
    Ok(TaskRss::Resident(unsafe {
        info.assume_init().pti_resident_size
    }))
}

fn unavailable_task(bytes: i32, errno: i32) -> io::Result<Option<TaskRss>> {
    if bytes == size_of::<libc::proc_taskinfo>() as i32 {
        return Ok(None);
    }
    if bytes > 0 {
        return Err(invalid("incomplete native task information"));
    }
    match errno {
        libc::EPERM | libc::EACCES => Ok(Some(TaskRss::Denied)),
        libc::ESRCH => Ok(Some(TaskRss::Exited)),
        0 => Err(invalid("native task information failed without an error")),
        _ => Err(io::Error::from_raw_os_error(errno)),
    }
}

fn pid_census() -> io::Result<Vec<i32>> {
    enumerate_pids(|pids| {
        let pointer = if pids.is_empty() {
            std::ptr::null_mut()
        } else {
            pids.as_mut_ptr().cast()
        };
        let bytes = unsafe {
            libc::proc_listpids(
                PROC_ALL_PIDS,
                0,
                pointer,
                std::mem::size_of_val(pids) as i32,
            )
        };
        if bytes <= 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(bytes as usize)
    })
}

fn enumerate_pids(mut list: impl FnMut(&mut [i32]) -> io::Result<usize>) -> io::Result<Vec<i32>> {
    let mut capacity = pid_count(list(&mut [])?)?
        .checked_add(64)
        .ok_or_else(|| invalid("native PID capacity overflow"))?;
    for _ in 0..MAX_CENSUS_ATTEMPTS {
        if capacity > MAX_PID_CAPACITY {
            return Err(invalid("native PID census exceeds its bound"));
        }
        let mut pids = Vec::new();
        pids.try_reserve_exact(capacity).map_err(io::Error::other)?;
        pids.resize(capacity, 0);
        let count = pid_count(list(&mut pids)?)?;
        if count > capacity {
            return Err(invalid("native PID census exceeded its buffer"));
        }
        if count == capacity {
            capacity = capacity
                .checked_mul(2)
                .ok_or_else(|| invalid("native PID capacity overflow"))?;
            continue;
        }
        pids.truncate(count);
        if pids.iter().any(|pid| *pid < 0) {
            return Err(invalid("native PID census returned an invalid PID"));
        }
        pids.retain(|pid| *pid != 0);
        pids.sort_unstable();
        if pids.is_empty() || pids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid("native PID census is empty or repeats a PID"));
        }
        return Ok(pids);
    }
    Err(invalid("native PID census kept growing"))
}

fn pid_count(bytes: usize) -> io::Result<usize> {
    if bytes == 0 || !bytes.is_multiple_of(size_of::<i32>()) {
        return Err(invalid("native PID census returned a partial count"));
    }
    Ok(bytes / size_of::<i32>())
}

fn parse_row(line: &str) -> io::Result<(i32, u64)> {
    let mut fields = line.split_ascii_whitespace();
    let (Some(pid), Some(rss), None) = (fields.next(), fields.next(), fields.next()) else {
        return Err(invalid("malformed memory fallback row"));
    };
    if !pid.bytes().all(|byte| byte.is_ascii_digit())
        || !rss.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid("memory fallback contains a nonnumeric value"));
    }
    let pid = pid
        .parse::<i32>()
        .map_err(|_| invalid("invalid fallback PID"))?;
    let rss = rss
        .parse::<u64>()
        .map_err(|_| invalid("invalid fallback RSS"))?;
    if pid <= 0 {
        return Err(invalid("invalid fallback PID"));
    }
    Ok((pid, rss))
}

fn add_rss(total: u64, rss: u64) -> io::Result<u64> {
    total
        .checked_add(rss)
        .ok_or_else(|| invalid("resident memory total overflow"))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            native_rss_kib: 50,
            denied_pids: vec![10, 20],
        }
    }

    #[test]
    fn combines_only_expected_rows_and_keeps_zero_rss() {
        let total = snapshot()
            .total_with_census(b" 20 0\n 10 300\n", || panic!("unneeded census"))
            .unwrap();
        assert_eq!(total, 350);
        assert_eq!(
            snapshot().fallback_args().unwrap(),
            ["-x", "-p", "10,20", "-o", "pid=,rss="]
        );
    }

    #[test]
    fn missing_rows_require_positive_exit_evidence() {
        assert_eq!(
            snapshot()
                .total_with_census(b"10 4\n", || Ok(vec![10, 30]))
                .unwrap(),
            54
        );
        assert!(snapshot()
            .total_with_census(b"10 4\n", || Ok(vec![20]))
            .is_err());
        assert!(snapshot()
            .total_with_census(b"10 4\n", || Err(io::Error::from_raw_os_error(libc::EPERM)))
            .is_err());
    }

    #[test]
    fn rejects_malformed_duplicate_unexpected_and_overflowing_rows() {
        for output in [
            &b"PID RSS\n"[..],
            b"10\n",
            b"10 1 extra\n",
            b"10 -1\n",
            b"+10 1\n",
            b"0 1\n",
            b"10 1\n10 1\n",
            b"30 1\n",
            b"2147483648 1\n",
            b"10 18446744073709551616\n",
            b"10 18446744073709551615\n",
            b"\xff\n",
        ] {
            assert!(snapshot().total_with_census(output, || Ok(vec![])).is_err());
        }
    }

    #[test]
    fn bounds_fallback_arguments_and_rejects_empty_selection() {
        assert!(Snapshot {
            native_rss_kib: 0,
            denied_pids: vec![]
        }
        .fallback_args()
        .is_err());
        assert!(Snapshot {
            native_rss_kib: 0,
            denied_pids: vec![-1]
        }
        .fallback_args()
        .is_err());
        let many = Snapshot {
            native_rss_kib: 0,
            denied_pids: (1..20_000).collect(),
        };
        assert!(many.fallback_args().is_err());
    }

    #[test]
    fn truncates_each_resident_separately_and_retains_denied_pids() {
        let result = sample_pids(vec![1, 2, 3, 4], |pid| {
            Ok(match pid {
                1 | 2 => TaskRss::Resident(2047),
                3 => TaskRss::Denied,
                _ => TaskRss::Exited,
            })
        })
        .unwrap();
        assert_eq!(result.native_rss_kib, 2);
        assert_eq!(result.denied_pids, vec![3]);
        assert!(sample_pids(vec![1], |_| Err(invalid("incomplete"))).is_err());
    }

    #[test]
    fn permissions_are_not_exits_and_partial_ffi_is_never_read() {
        let size = size_of::<libc::proc_taskinfo>() as i32;
        assert_eq!(unavailable_task(size, libc::EPERM).unwrap(), None);
        assert_eq!(
            unavailable_task(0, libc::EPERM).unwrap(),
            Some(TaskRss::Denied)
        );
        assert_eq!(
            unavailable_task(0, libc::EACCES).unwrap(),
            Some(TaskRss::Denied)
        );
        assert_eq!(
            unavailable_task(0, libc::ESRCH).unwrap(),
            Some(TaskRss::Exited)
        );
        for (bytes, errno) in [
            (size - 1, libc::ESRCH),
            (0, 0),
            (0, libc::EINVAL),
            (0, libc::ENOSYS),
        ] {
            assert!(unavailable_task(bytes, errno).is_err());
        }
    }

    #[test]
    fn census_retries_full_buffers_and_excludes_kernel_pid() {
        let mut calls = 0;
        let result = enumerate_pids(|pids| {
            calls += 1;
            match calls {
                1 => Ok(4),
                2 => Ok(std::mem::size_of_val(pids)),
                _ => {
                    pids[..3].copy_from_slice(&[2, 0, 1]);
                    Ok(12)
                }
            }
        })
        .unwrap();
        assert_eq!(result, [1, 2]);
        assert_eq!(calls, 3);
    }

    #[test]
    fn census_rejects_partial_failed_oversized_and_repeated_results() {
        assert!(enumerate_pids(|_| Ok(3)).is_err());
        assert!(enumerate_pids(|_| Ok(0)).is_err());
        assert!(enumerate_pids(|_| Ok(usize::MAX - 3)).is_err());
        assert!(enumerate_pids(|_| Err(io::Error::from_raw_os_error(libc::EPERM))).is_err());
        assert!(enumerate_pids(|pids| Ok(if pids.is_empty() {
            4
        } else {
            std::mem::size_of_val(pids)
        }))
        .is_err());
        for entries in [[1, 1], [-1, 2], [0, 0]] {
            assert!(enumerate_pids(|pids| {
                if !pids.is_empty() {
                    pids[..2].copy_from_slice(&entries);
                }
                Ok(8)
            })
            .is_err());
        }
        assert!(enumerate_pids(|pids| Ok(if pids.is_empty() {
            4
        } else {
            std::mem::size_of_val(pids) + 4
        }))
        .is_err());
    }
}
