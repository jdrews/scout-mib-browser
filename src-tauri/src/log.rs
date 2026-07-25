use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::Emitter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;

/// Tauri event name for real-time log entry emission.
pub const LOG_ENTRY_EVENT: &str = "system-log-entry";

/// A single log entry for the UI.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

/// Shared in-memory ring buffer of recent log entries.
#[derive(Clone)]
pub struct LogBuffer {
    inner: Arc<Mutex<Vec<LogEntry>>>,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new(2000)
    }
}

impl LogBuffer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::with_capacity(max_entries))),
        }
    }

    /// Pushes an entry and trims to max capacity.
    pub fn push(&self, entry: LogEntry) {
        let mut buf = self.inner.lock().unwrap();
        buf.push(entry);
        let len = buf.len();
        if len > 2000 {
            buf.drain(..len - 2000);
        }
    }

    /// Returns all entries (oldest first).
    pub fn entries(&self) -> Vec<LogEntry> {
        self.inner.lock().unwrap().clone()
    }

    /// Clears the buffer.
    pub fn clear(&self) {
        self.inner.lock().unwrap().clear();
    }
}

/// Holds the file appender guard so it lives for the app lifetime.
static APPENDER_GUARD: std::sync::OnceLock<
    std::sync::Mutex<Option<tracing_appender::non_blocking::WorkerGuard>>,
> = std::sync::OnceLock::new();

/// Optional Tauri AppHandle stored statically for event emission from tracing layer.
static TAURI_APP_HANDLE: std::sync::OnceLock<std::sync::Mutex<Option<tauri::AppHandle>>> =
    std::sync::OnceLock::new();

/// Set the Tauri app handle after initialization so the UI layer can emit events.
pub fn set_tauri_app_handle(handle: tauri::AppHandle) {
    TAURI_APP_HANDLE
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap()
        .replace(handle);
}

/// Custom tracing layer that pushes entries to the shared buffer and emits Tauri events.
struct UiLogLayer {
    buffer: LogBuffer,
}

impl<S> Layer<S> for UiLogLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let level = metadata.level().to_string();
        let target = metadata.target().to_string();

        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);
        let message = visitor.message.unwrap_or_default();

        let timestamp = chrono::Local::now().format("%H:%M:%S%.3f").to_string();

        let entry = LogEntry {
            timestamp,
            level,
            target,
            message,
        };

        self.buffer.push(entry.clone());

        if let Some(handle_lock) = TAURI_APP_HANDLE.get() {
            if let Some(app) = handle_lock.lock().unwrap().as_ref() {
                let _ = app.emit_to("main", LOG_ENTRY_EVENT, &entry);
            }
        }
    }
}

/// Visitor to extract the message field from a tracing event.
#[derive(Default)]
struct LogVisitor {
    message: Option<String>,
}

impl tracing::field::Visit for LogVisitor {
    fn record_str(&mut self, field: &tracing_core::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &tracing_core::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{:?}", value));
        }
    }
}

/// Initializes the tracing subscriber with a rotating file appender and UI log layer.
/// Returns the shared LogBuffer for Tauri state management.
pub fn init_logging() -> Result<LogBuffer, String> {
    let config_dir = config_dir();
    std::fs::create_dir_all(&config_dir).map_err(|e| {
        format!(
            "Failed to create config dir {}: {}",
            config_dir.display(),
            e
        )
    })?;

    // Rotating file appender: daily rotation, wrapped in non-blocking.
    let file_appender = tracing_appender::rolling::daily(&config_dir, "scout.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Store the guard so it lives for the app lifetime.
    APPENDER_GUARD.get_or_init(|| std::sync::Mutex::new(Some(guard)));

    let buffer = LogBuffer::new(2000);
    let ui_layer = UiLogLayer {
        buffer: buffer.clone(),
    };

    // Format layer writes to the rotating file.
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .without_time()
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")));

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(ui_layer)
        .init();

    Ok(buffer)
}

/// Returns the config directory path (`~/.config/scout`).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(PathBuf::new)
        .join("scout")
}

#[tauri::command]
pub fn log_read(buffer: tauri::State<LogBuffer>) -> Vec<LogEntry> {
    buffer.entries()
}

#[tauri::command]
pub fn log_clear(buffer: tauri::State<LogBuffer>) {
    buffer.clear();
}

#[tauri::command]
pub fn log_path() -> String {
    config_dir().join("scout.log").to_string_lossy().to_string()
}
