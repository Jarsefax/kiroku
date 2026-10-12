use crate::storage::Status;

/// Optional ANSI styling for command output. Plain rendering stays byte-for-byte unchanged.
#[derive(Clone, Copy, Default)]
pub struct Style {
    pub enabled: bool,
}

impl Style {
    fn paint(self, text: &str, code: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }

    pub fn heading(self, text: &str) -> String {
        self.paint(text, "1;36")
    }

    pub fn status(self, text: &str, status: Status) -> String {
        self.paint(
            text,
            match status {
                Status::Todo => "1;34",
                Status::Doing => "1;33",
                Status::Done => "1;32",
            },
        )
    }

    pub fn muted(self, text: &str) -> String {
        self.paint(text, "2")
    }

    pub fn error(self, text: &str) -> String {
        self.paint(text, "1;31")
    }
}
