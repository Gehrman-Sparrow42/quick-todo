use std::fs::OpenOptions;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

pub fn append_tasks(path: &Path, draft: &str) -> io::Result<bool> {
    let tasks: Vec<&str> = draft
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            let text = line
                .strip_prefix("- [ ] ")
                .or_else(|| line.strip_prefix("- "))
                .or_else(|| line.strip_prefix("* "))
                .unwrap_or(line)
                .trim();
            (!text.is_empty()).then_some(text)
        })
        .collect();
    if tasks.is_empty() {
        return Ok(false);
    }
    // Existing file only, with other writers excluded until the append has flushed.
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(FILE_SHARE_READ)
        .open(path)?;
    let mut content = Vec::new();
    file.read_to_end(&mut content)?;
    if content.contains(&0)
        || content.starts_with(&[0xff, 0xfe])
        || content.starts_with(&[0xfe, 0xff])
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Target must be UTF-8 text",
        ));
    }
    std::str::from_utf8(&content)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Target must be UTF-8 text"))?;
    let newline = if content.windows(2).any(|w| w == b"\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let body = content
        .strip_prefix(&[0xef, 0xbb, 0xbf])
        .unwrap_or(&content);
    let mut addition = String::new();
    if !body.is_empty() && !body.ends_with(b"\n") {
        addition.push_str(newline);
    }
    for task in tasks {
        addition.push_str("- [ ] ");
        addition.push_str(task);
        addition.push_str(newline);
    }
    file.seek(SeekFrom::End(0))?;
    let original_len = content.len() as u64;
    let result = file
        .write_all(addition.as_bytes())
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all());
    if let Err(save_error) = result {
        // The handle excludes other writers and renames. Roll back only our new bytes.
        if let Err(rollback_error) = file.set_len(original_len).and_then(|_| file.sync_all()) {
            return Err(io::Error::new(
                save_error.kind(),
                format!("{save_error}. The file may contain a partial append; check it before retrying. Rollback failed: {rollback_error}"),
            ));
        }
        return Err(io::Error::new(
            save_error.kind(),
            format!("{save_error}. The append was rolled back; your existing notes are unchanged."),
        ));
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    fn path() -> std::path::PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-files");
        fs::create_dir_all(&dir).unwrap();
        dir.join(format!(
            "{}-{}.md",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    #[test]
    fn preserves_crlf_bom_and_turkish_and_normalizes() {
        let p = path();
        fs::write(&p, b"\xef\xbb\xbfOld\r\nLast").unwrap();
        assert!(append_tasks(&p, "- [ ] çiçek\n* iki\n- üç\n\n").unwrap());
        assert_eq!(
            fs::read(&p).unwrap(),
            "\u{feff}Old\r\nLast\r\n- [ ] çiçek\r\n- [ ] iki\r\n- [ ] üç\r\n".as_bytes()
        );
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn preserves_lf_and_ignores_whitespace() {
        let p = path();
        fs::write(&p, b"abc\n").unwrap();
        assert!(!append_tasks(&p, " \n\t").unwrap());
        assert!(append_tasks(&p, "task").unwrap());
        assert_eq!(fs::read(&p).unwrap(), b"abc\n- [ ] task\n");
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn rejects_non_utf8_without_changes() {
        let p = path();
        let bytes = [0xff, 0xfe, 0x61, 0];
        fs::write(&p, bytes).unwrap();
        assert!(append_tasks(&p, "task").is_err());
        assert_eq!(fs::read(&p).unwrap(), bytes);
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn missing_target_is_not_created() {
        let p = path();
        assert!(append_tasks(&p, "task").is_err());
        assert!(!p.exists());
    }
    #[test]
    fn locked_target_is_unchanged() {
        let p = path();
        fs::write(&p, b"Existing notes\n").unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&p)
            .unwrap();
        assert!(append_tasks(&p, "new task").is_err());
        assert_eq!(fs::read(&p).unwrap(), b"Existing notes\n");
        drop(lock);
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn empty_file_starts_with_a_task_without_extra_blank_line() {
        let p = path();
        fs::write(&p, b"").unwrap();
        assert!(append_tasks(&p, "first idea").unwrap());
        assert_eq!(fs::read(&p).unwrap(), b"- [ ] first idea\n");
        fs::remove_file(p).unwrap();
    }
}
