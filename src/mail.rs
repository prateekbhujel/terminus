use std::sync::{Arc, Mutex};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MailMessage {
    pub to: String,
    pub subject: String,
    pub lines: Vec<String>,
    pub action_text: Option<String>,
    pub action_url: Option<String>,
    pub timestamp: String,
}

impl MailMessage {
    pub fn to(recipient: impl Into<String>) -> Self {
        Self {
            to: recipient.into(),
            subject: "Notification".to_string(),
            lines: Vec::new(),
            action_text: None,
            action_url: None,
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }

    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = subject.into();
        self
    }

    pub fn line(mut self, line: impl Into<String>) -> Self {
        self.lines.push(line.into());
        self
    }

    pub fn action(mut self, text: impl Into<String>, url: impl Into<String>) -> Self {
        self.action_text = Some(text.into());
        self.action_url = Some(url.into());
        self
    }

    pub fn send_via(&self, mailer: &Mailer) -> Result<(), String> {
        mailer.send(self.clone())
    }

    /// Render email content as responsive HTML
    pub fn render_html(&self) -> String {
        let lines_html = self
            .lines
            .iter()
            .map(|l| format!("<p style=\"margin: 0 0 16px; font-size: 15px; line-height: 24px; color: #374151;\">{}</p>", l))
            .collect::<Vec<_>>()
            .join("\n");

        let action_html = if let (Some(text), Some(url)) = (&self.action_text, &self.action_url) {
            format!(
                r#"<table role="presentation" border="0" cellpadding="0" cellspacing="0" style="margin: 24px 0;">
                    <tr>
                        <td align="center" style="border-radius: 6px; background-color: #111827;">
                            <a href="{url}" target="_blank" style="display: inline-block; padding: 12px 24px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; font-size: 14px; font-weight: 600; color: #ffffff; text-decoration: none; border-radius: 6px;">{text} &rarr;</a>
                        </td>
                    </tr>
                </table>"#
            )
        } else {
            String::new()
        };

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{subject}</title>
</head>
<body style="margin: 0; padding: 0; background-color: #f3f4f6; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; -webkit-font-smoothing: antialiased;">
    <table role="presentation" border="0" cellpadding="0" cellspacing="0" width="100%" style="background-color: #f3f4f6; padding: 40px 0;">
        <tr>
            <td align="center">
                <table role="presentation" border="0" cellpadding="0" cellspacing="0" width="100%" style="max-width: 580px; background-color: #ffffff; border-radius: 8px; border: 1px solid #e5e7eb; overflow: hidden; box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);">
                    <!-- Header -->
                    <tr>
                        <td style="padding: 28px 32px; border-bottom: 1px solid #f3f4f6; background-color: #fafafa;">
                            <h1 style="margin: 0; font-size: 18px; font-weight: 600; color: #111827;">{subject}</h1>
                        </td>
                    </tr>
                    <!-- Content -->
                    <tr>
                        <td style="padding: 32px;">
                            {lines_html}
                            {action_html}
                            <hr style="border: none; border-top: 1px solid #e5e7eb; margin: 28px 0 20px;">
                            <p style="margin: 0; font-size: 13px; color: #6b7280; line-height: 20px;">
                                Sent to <strong>{to}</strong> &bull; {timestamp}
                            </p>
                        </td>
                    </tr>
                </table>
            </td>
        </tr>
    </table>
</body>
</html>"#,
            subject = self.subject,
            lines_html = lines_html,
            action_html = action_html,
            to = self.to,
            timestamp = self.timestamp,
        )
    }

    /// Render email content as clean plain text
    pub fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("SUBJECT: {}\n", self.subject));
        out.push_str(&format!("TO: {}\n", self.to));
        out.push_str(&format!("DATE: {}\n\n", self.timestamp));

        for line in &self.lines {
            out.push_str(line);
            out.push_str("\n\n");
        }

        if let (Some(text), Some(url)) = (&self.action_text, &self.action_url) {
            out.push_str(&format!("{}: {}\n\n", text, url));
        }

        out.push_str("--\nSent via Terminus Framework\n");
        out
    }
}

/// Pluggable email delivery transport
pub trait MailTransport: Send + Sync {
    fn deliver(&self, msg: &MailMessage) -> Result<(), String>;
}

/// In-memory array transport for testing and local assertions
#[derive(Clone, Default)]
pub struct ArrayTransport {
    mailbox: Arc<Mutex<Vec<MailMessage>>>,
}

impl ArrayTransport {
    pub fn new() -> Self {
        Self {
            mailbox: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn messages(&self) -> Vec<MailMessage> {
        self.mailbox.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.mailbox.lock().unwrap().clear();
    }
}

impl MailTransport for ArrayTransport {
    fn deliver(&self, msg: &MailMessage) -> Result<(), String> {
        self.mailbox.lock().unwrap().push(msg.clone());
        Ok(())
    }
}

/// Log transport writing email deliveries to tracing / stdout
pub struct LogTransport;

impl MailTransport for LogTransport {
    fn deliver(&self, msg: &MailMessage) -> Result<(), String> {
        tracing::info!(to = %msg.to, subject = %msg.subject, "Delivered email message");
        Ok(())
    }
}

/// File transport saving rendered HTML emails to disk
pub struct FileTransport {
    outbox_dir: PathBuf,
}

impl FileTransport {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        let outbox_dir = dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&outbox_dir);
        Self { outbox_dir }
    }
}

impl MailTransport for FileTransport {
    fn deliver(&self, msg: &MailMessage) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp_millis();
        let safe_to = msg.to.replace(['@', '.', ' '], "_");
        let filename = format!("{}_{}.html", now, safe_to);
        let path = self.outbox_dir.join(filename);
        std::fs::write(&path, msg.render_html()).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct Mailer {
    transport: Arc<dyn MailTransport>,
    array_ref: Option<ArrayTransport>,
}

impl Default for Mailer {
    fn default() -> Self {
        Self::new()
    }
}

impl Mailer {
    /// Create a new in-memory mailer (default)
    pub fn new() -> Self {
        let array = ArrayTransport::new();
        Self {
            transport: Arc::new(array.clone()),
            array_ref: Some(array),
        }
    }

    /// Create a mailer with custom transport
    pub fn with_transport(transport: impl MailTransport + 'static) -> Self {
        Self {
            transport: Arc::new(transport),
            array_ref: None,
        }
    }

    /// Create a log-based mailer
    pub fn log() -> Self {
        Self::with_transport(LogTransport)
    }

    /// Create a file-outbox mailer
    pub fn file(dir: impl AsRef<Path>) -> Self {
        Self::with_transport(FileTransport::new(dir))
    }

    /// Deliver an email message via configured transport
    pub fn send(&self, msg: MailMessage) -> Result<(), String> {
        self.transport.deliver(&msg)
    }

    /// Get sent messages if using array transport
    pub fn sent_messages(&self) -> Vec<MailMessage> {
        self.array_ref
            .as_ref()
            .map(|a| a.messages())
            .unwrap_or_default()
    }

    /// Clear sent messages if using array transport
    pub fn clear(&self) {
        if let Some(a) = &self.array_ref {
            a.clear();
        }
    }
}
