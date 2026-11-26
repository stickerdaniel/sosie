use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum OutputError {
    NotRegular(PathBuf),
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Rollback {
        path: PathBuf,
        source: io::Error,
        failed: Vec<(PathBuf, io::Error)>,
    },
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::NotRegular(path) => {
                write!(f, "{}: exists and is not a regular file", path.display())
            }
            OutputError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            OutputError::Rollback {
                path,
                source,
                failed,
            } => {
                write!(f, "{}: {source}; rollback failed", path.display())?;
                for (path, e) in failed {
                    write!(f, ", {}: {e}", path.display())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for OutputError {}

#[derive(Debug)]
pub struct StaleBackup {
    pub path: PathBuf,
    pub error: io::Error,
}

impl fmt::Display for StaleBackup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}; the new files are in place, this backup of an old file stays",
            self.path.display(),
            self.error
        )
    }
}

pub struct Targets<const N: usize> {
    paths: [PathBuf; N],
    existing: [bool; N],
}

impl<const N: usize> Targets<N> {
    pub fn check(paths: [PathBuf; N]) -> Result<Self, OutputError> {
        let mut existing = [false; N];
        for (path, exists) in paths.iter().zip(&mut existing) {
            *exists = match fs::symlink_metadata(path) {
                Ok(meta) if meta.file_type().is_file() => true,
                Ok(_) => return Err(OutputError::NotRegular(path.clone())),
                Err(e) if e.kind() == io::ErrorKind::NotFound => false,
                Err(source) => {
                    return Err(OutputError::Io {
                        path: path.clone(),
                        source,
                    })
                }
            };
        }
        Ok(Targets { paths, existing })
    }

    pub fn paths(&self) -> &[PathBuf; N] {
        &self.paths
    }

    pub fn write(
        &self,
        contents: [&[u8]; N],
        rename: impl FnMut(&Path, &Path) -> io::Result<()>,
    ) -> Result<Vec<StaleBackup>, OutputError> {
        let mut commit = Commit {
            targets: self,
            rename,
            temps: Vec::with_capacity(N),
            backups: Vec::with_capacity(N),
            placed: 0,
        };
        match commit.run(contents) {
            Ok(()) => Ok(commit.finish()),
            Err((path, source)) => Err(commit.roll_back(path, source)),
        }
    }
}

fn reserve(path: &Path, tag: &str) -> io::Result<(PathBuf, File)> {
    let mut n = 0u64;
    loop {
        let mut name = OsString::from(".");
        name.push(path.file_name().unwrap_or_default());
        name.push(format!(".sosie-{}-{n}.{tag}", std::process::id()));
        let candidate = path.with_file_name(name);
        match File::create_new(&candidate) {
            Ok(file) => return Ok((candidate, file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(e),
        }
    }
}

struct Backup<'a> {
    original: &'a Path,
    path: PathBuf,
    moved: bool,
}

struct Commit<'a, const N: usize, R> {
    targets: &'a Targets<N>,
    rename: R,
    temps: Vec<PathBuf>,
    backups: Vec<Backup<'a>>,
    placed: usize,
}

impl<'a, const N: usize, R: FnMut(&Path, &Path) -> io::Result<()>> Commit<'a, N, R> {
    fn run(&mut self, contents: [&[u8]; N]) -> Result<(), (&'a Path, io::Error)> {
        let targets = self.targets;
        for (path, bytes) in targets.paths.iter().zip(contents) {
            let (temp, mut file) = reserve(path, "tmp").map_err(|e| (path.as_path(), e))?;
            self.temps.push(temp);
            file.write_all(bytes).map_err(|e| (path.as_path(), e))?;
        }
        for (path, _) in targets.paths.iter().zip(targets.existing).filter(|t| t.1) {
            let (backup, _) = reserve(path, "bak").map_err(|e| (path.as_path(), e))?;
            self.backups.push(Backup {
                original: path,
                path: backup,
                moved: false,
            });
            let backup = self.backups.last_mut().expect("just pushed");
            (self.rename)(path, &backup.path).map_err(|e| (path.as_path(), e))?;
            backup.moved = true;
        }
        for (temp, path) in self.temps.iter().zip(&targets.paths) {
            (self.rename)(temp, path).map_err(|e| (path.as_path(), e))?;
            self.placed += 1;
        }
        Ok(())
    }

    fn finish(self) -> Vec<StaleBackup> {
        self.backups
            .into_iter()
            .filter_map(|backup| {
                fs::remove_file(&backup.path)
                    .err()
                    .map(|error| StaleBackup {
                        path: backup.path,
                        error,
                    })
            })
            .collect()
    }

    fn roll_back(mut self, path: &Path, source: io::Error) -> OutputError {
        let mut failed = Vec::new();
        let created = self.targets.paths[..self.placed]
            .iter()
            .filter(|p| !self.backups.iter().any(|b| b.original == *p));
        for placed in created {
            if let Err(e) = fs::remove_file(placed) {
                failed.push((placed.clone(), e));
            }
        }
        for backup in &self.backups {
            let restored = if backup.moved {
                (self.rename)(&backup.path, backup.original)
            } else {
                fs::remove_file(&backup.path)
            };
            if let Err(e) = restored {
                failed.push((backup.path.clone(), e));
            }
        }
        for temp in &self.temps[self.placed..] {
            if let Err(e) = fs::remove_file(temp) {
                failed.push((temp.clone(), e));
            }
        }
        let path = path.to_path_buf();
        if failed.is_empty() {
            OutputError::Io { path, source }
        } else {
            OutputError::Rollback {
                path,
                source,
                failed,
            }
        }
    }
}
