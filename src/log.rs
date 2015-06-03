//!Log tools.

use std::io::{self, Write};
use std::fs;
use std::sync::Mutex;

pub type Result = io::Result<()>;

///Common trait for log tools.
pub trait Log: Send + Sync {
    ///Print a note to the log or return eventual errors.
    fn try_note(&self, message: &str) -> Result;
    ///Print a warning to the log or return eventual errors.
    fn try_warning(&self, message: &str) -> Result;
    ///Print an error to the log or return eventual errors.
    fn try_error(&self, message: &str) -> Result;

    ///Print a note to the log and ignore any errors.
    #[allow(unused_must_use)]
    #[inline]
    fn note(&self, message: &str) {
        self.try_note(message);
    }
    ///Print a warning to the log and ignore any errors.
    #[allow(unused_must_use)]
    #[inline]
    fn warning(&self, message: &str) {
        self.try_warning(message);
    }
    ///Print an error to the log and ignore any errors.
    #[allow(unused_must_use)]
    #[inline]
    fn error(&self, message: &str) {
        self.try_error(message);
    }
}

///A quiet log tool. Nothing will be printed anywhere.
pub struct Quiet;

impl Log for Quiet {
    fn try_note(&self, _message: &str) -> Result {
        Ok(())
    }

    fn try_warning(&self, _message: &str) -> Result {
        Ok(())
    }

    fn try_error(&self, _message: &str) -> Result {
        Ok(())
    }
}

///Log tool for printing to standard output.
pub struct StdOut;

impl Log for StdOut {
    fn try_note(&self, message: &str) -> Result {
        println!("note: {}", message);
        Ok(())
    }

    fn try_warning(&self, message: &str) -> Result {
        println!("warning: {}", message);
        Ok(())
    }

    fn try_error(&self, message: &str) -> Result {
        println!("error: {}", message);
        Ok(())
    }
}

///Log tool for printing to a file.
pub struct File {
    file: Mutex<fs::File>
}

impl File {
    pub fn new(file: fs::File) -> File {
        File {
            file: Mutex::new(file)
        }
    }
}

impl Log for File {
    fn try_note(&self, message: &str) -> Result {
        let mut f = match self.file.lock() {
            Ok(f) => f,
            Err(_e) => return Err(io::Error::new(io::ErrorKind::Other, "poisoned log file lock"))
        };
        write!(f, "note: {}", message)
    }

    fn try_warning(&self, message: &str) -> Result {
        let mut f = match self.file.lock() {
            Ok(f) => f,
            Err(_e) => return Err(io::Error::new(io::ErrorKind::Other, "poisoned log file lock"))
        };
        write!(f, "warning: {}", message)
    }

    fn try_error(&self, message: &str) -> Result {
        let mut f = match self.file.lock() {
            Ok(f) => f,
            Err(_e) => return Err(io::Error::new(io::ErrorKind::Other, "poisoned log file lock"))
        };
        write!(f, "error: {}", message)
    }
}

#[cfg(test)]
mod test {
    use std::fs;
    use log;
    use Server;
    use Context;
    use Response;
    use tempdir;

    #[test]
    fn log_to_file() {
        let dir = tempdir::TempDir::new("log_to_file").unwrap();
        let file = fs::File::create(&dir.path().join("test.log")).unwrap();
        Server {
            log: Box::new(log::File::new(file)),
            ..Server::new(|_: Context, _: Response| {})
        }.build();
    }
}