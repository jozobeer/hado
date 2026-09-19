use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub fn executable(source: &str, source_path: &str, output: &Path) -> Result<(), String> {
    executable_with(output, |object| {
        crate::compile_source_to_object(source, source_path, object)
    })
}

pub(crate) fn executable_with(
    output: &Path,
    compile: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let scratch = Scratch::beside(output)?;
    let object = scratch.0.join("program.o");
    let executable = scratch.0.join("program");
    compile(&object)?;
    crate::link_executable(&object, &executable)?;
    std::fs::rename(executable, output).map_err(|e| e.to_string())
}

struct Scratch(PathBuf);

impl Scratch {
    fn beside(output: &Path) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        loop {
            let serial = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(".hd-build-{}-{serial}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
