//! 本机 socket 的命名、查找与清理。

use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::error_::MptpIpcError;

/// socket 文件前缀。
pub const SOCKET_FILE_PREFIX: &str = "kb-";

/// socket 文件扩展名。
pub const SOCKET_FILE_EXTENSION: &str = "sock";

/// 客户端缺省愿意等服务端多久。
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// 生成 `<runtime_dir>/kb-<YYYYMMDD>-<uuid>.sock`。
pub fn new_socket_path_in(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(format!(
        "{SOCKET_FILE_PREFIX}{}-{}.{SOCKET_FILE_EXTENSION}",
        today_utc_compact(),
        uuid_like_()
    ))
}

/// 在运行时目录里找最新的、类型确实是 socket 的文件。
pub fn find_socket_in(runtime_dir: &Path) -> Result<Option<PathBuf>, MptpIpcError> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(runtime_dir).map_err(|source| MptpIpcError::RuntimeDir {
        path: runtime_dir.to_path_buf(),
        source,
    })? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some(SOCKET_FILE_EXTENSION) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.file_type().is_socket() {
            continue;
        }
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if newest
            .as_ref()
            .is_none_or(|(current, _)| modified > *current)
        {
            newest = Some((modified, path));
        }
    }
    Ok(newest.map(|(_, path)| path))
}

/// 清掉残留的 `kb-*.sock` 与旧的 `kb-*.ipc`。
pub(crate) fn clear_stale_sockets_(runtime_dir: &Path) -> Result<(), MptpIpcError> {
    let entries = std::fs::read_dir(runtime_dir).map_err(|source| MptpIpcError::RuntimeDir {
        path: runtime_dir.to_path_buf(),
        source,
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_old = matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some(SOCKET_FILE_EXTENSION) | Some("ipc")
        );
        if is_old {
            let _ = std::fs::remove_file(&path);
        }
    }
    Ok(())
}

/// `<YYYYMMDD>`（UTC）。
fn today_utc_compact() -> String {
    let seconds = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = seconds / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}{month:02}{day:02}")
}

/// 由 1970-01-01 起的天数算出公历日期。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// 生成一个 32 位十六进制随机串（不额外引入 uuid 依赖）。
fn uuid_like_() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0),
    );
    let first = hasher.finish();
    hasher.write_u64(first ^ std::process::id() as u64);
    let second = hasher.finish();
    format!("{first:016x}{second:016x}")
}
