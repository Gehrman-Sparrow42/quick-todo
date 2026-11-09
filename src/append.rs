use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

pub fn append_tasks(path: &Path, draft: &str, screenshot: Option<&[u8]>) -> io::Result<bool> {
    let mut tasks: Vec<&str> = draft
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
        if screenshot.is_none() {
            return Ok(false);
        }
        tasks.push("Screenshot");
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
    file.seek(SeekFrom::End(0))?;
    // Create only after the note has been opened, locked and validated.
    let attachment = screenshot
        .map(|bytes| create_attachment(path, bytes))
        .transpose()?;
    for (index, task) in tasks.iter().enumerate() {
        addition.push_str("- [ ] ");
        addition.push_str(task);
        addition.push_str(newline);
        if index == 0 {
            if let Some((_, filename)) = &attachment {
                addition.push_str("  ![Screenshot](quick-todo-attachments/");
                addition.push_str(filename);
                addition.push(')');
                addition.push_str(newline);
            }
        }
    }
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
        if let Some((attachment_path, _)) = &attachment {
            if let Err(cleanup_error) = fs::remove_file(attachment_path) {
                return Err(io::Error::new(save_error.kind(), format!("{save_error}. The append was rolled back, but screenshot cleanup failed at {}: {cleanup_error}", attachment_path.display())));
            }
        }
        return Err(io::Error::new(
            save_error.kind(),
            format!("{save_error}. The append was rolled back; your existing notes are unchanged."),
        ));
    }
    Ok(true)
}

static NEXT_ATTACHMENT: AtomicU64 = AtomicU64::new(0);

fn create_attachment(note: &Path, bytes: &[u8]) -> io::Result<(PathBuf, String)> {
    let parent = note
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let dir = parent.join("quick-todo-attachments");
    fs::create_dir_all(&dir)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    loop {
        let id = NEXT_ATTACHMENT.fetch_add(1, Ordering::Relaxed);
        let name = format!("screenshot-{stamp}-{}-{id}.png", std::process::id());
        let path = dir.join(&name);
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let result = file
            .write_all(bytes)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all());
        drop(file);
        if let Err(error) = result {
            return match fs::remove_file(&path) {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(io::Error::new(
                    error.kind(),
                    format!(
                        "{error}. Could not remove incomplete screenshot at {}: {cleanup_error}",
                        path.display()
                    ),
                )),
            };
        }
        return Ok((path, name));
    }
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
        assert!(append_tasks(&p, "- [ ] çiçek\n* iki\n- üç\n\n", None).unwrap());
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
        assert!(!append_tasks(&p, " \n\t", None).unwrap());
        assert!(append_tasks(&p, "task", None).unwrap());
        assert_eq!(fs::read(&p).unwrap(), b"abc\n- [ ] task\n");
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn rejects_non_utf8_without_changes() {
        let p = path();
        let bytes = [0xff, 0xfe, 0x61, 0];
        fs::write(&p, bytes).unwrap();
        assert!(append_tasks(&p, "task", None).is_err());
        assert_eq!(fs::read(&p).unwrap(), bytes);
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn missing_target_is_not_created() {
        let p = path();
        assert!(append_tasks(&p, "task", None).is_err());
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
        assert!(append_tasks(&p, "new task", None).is_err());
        assert_eq!(fs::read(&p).unwrap(), b"Existing notes\n");
        drop(lock);
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn empty_file_starts_with_a_task_without_extra_blank_line() {
        let p = path();
        fs::write(&p, b"").unwrap();
        assert!(append_tasks(&p, "first idea", None).unwrap());
        assert_eq!(fs::read(&p).unwrap(), b"- [ ] first idea\n");
        fs::remove_file(p).unwrap();
    }
    fn image_note() -> PathBuf {
        let p = path();
        let dir = p.with_extension("dir");
        fs::create_dir(&dir).unwrap();
        dir.join("Todo.md")
    }
    fn verify_image(p: &Path, before: &str, first: &str, after: &str) {
        let note = fs::read_to_string(p).unwrap();
        let rest = note
            .strip_prefix(&format!(
                "{before}{first}  ![Screenshot](quick-todo-attachments/"
            ))
            .unwrap();
        let (filename, suffix) = rest.split_once(')').unwrap();
        assert!(filename.starts_with("screenshot-") && filename.ends_with(".png"));
        assert_eq!(suffix, after);
        let image = p
            .parent()
            .unwrap()
            .join("quick-todo-attachments")
            .join(filename);
        assert_eq!(fs::read(&image).unwrap(), b"test png");
        fs::remove_file(image).unwrap();
        fs::remove_dir(p.parent().unwrap().join("quick-todo-attachments")).unwrap();
        fs::remove_file(p).unwrap();
        fs::remove_dir(p.parent().unwrap()).unwrap();
    }
    #[test]
    fn image_only_creates_screenshot_task() {
        let p = image_note();
        fs::write(&p, b"").unwrap();
        assert!(append_tasks(&p, " \n", Some(b"test png")).unwrap());
        verify_image(&p, "", "- [ ] Screenshot\n", "\n");
    }
    #[test]
    fn image_attaches_to_first_task_preserving_crlf() {
        let p = image_note();
        fs::write(&p, b"\xef\xbb\xbfExisting\r\n").unwrap();
        assert!(append_tasks(&p, "- [ ] first\n* second", Some(b"test png")).unwrap());
        verify_image(
            &p,
            "\u{feff}Existing\r\n",
            "- [ ] first\r\n",
            "\r\n- [ ] second\r\n",
        );
    }
    #[test]
    fn invalid_or_locked_target_does_not_create_image() {
        let p = image_note();
        fs::write(&p, b"\xff\xfe").unwrap();
        assert!(append_tasks(&p, "task", Some(b"test png")).is_err());
        assert!(!p.parent().unwrap().join("quick-todo-attachments").exists());
        fs::write(&p, b"existing").unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&p)
            .unwrap();
        assert!(append_tasks(&p, "task", Some(b"test png")).is_err());
        assert!(!p.parent().unwrap().join("quick-todo-attachments").exists());
        drop(lock);
        fs::remove_file(&p).unwrap();
        fs::remove_dir(p.parent().unwrap()).unwrap();
    }
}
