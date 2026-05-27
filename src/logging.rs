use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::{Mutex, OnceLock};
use time::OffsetDateTime;

pub struct Logger {
    writer: Mutex<BufWriter<File>>,
}

impl Logger {
    pub fn new() -> anyhow::Result<Self> {
        let today = OffsetDateTime::now_utc().date();
        let filename = format!("gh-guard-{}.jsonl", today);
        let path = crate::paths::logs_dir().join(&filename);

        std::fs::create_dir_all(path.parent().unwrap())?;

        let file = OpenOptions::new().create(true).append(true).open(&path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&path, perms)?;
        }

        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
        })
    }

    pub fn log(&self, event: &str, fields: HashMap<String, Value>) -> anyhow::Result<()> {
        let mut map = Map::new();
        map.insert(
            "timestamp".to_string(),
            Value::String(
                OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)?,
            ),
        );
        map.insert("event".to_string(), Value::String(event.to_string()));
        map.insert("pid".to_string(), Value::Number(std::process::id().into()));
        map.insert(
            "hostname".to_string(),
            Value::String(hostname::get()?.to_string_lossy().into()),
        );
        map.insert("username".to_string(), Value::String(whoami::username()));
        map.insert(
            "cwd".to_string(),
            Value::String(std::env::current_dir()?.to_string_lossy().into()),
        );

        for (k, v) in fields {
            map.insert(k, v);
        }

        let line = serde_json::to_string(&map)?;
        let mut writer = self.writer.lock().unwrap();
        writeln!(writer, "{}", line)?;
        writer.flush()?;
        Ok(())
    }
}

static GLOBAL_LOGGER: OnceLock<Logger> = OnceLock::new();

pub fn init_global_logger() -> anyhow::Result<()> {
    let logger = Logger::new()?;
    GLOBAL_LOGGER
        .set(logger)
        .map_err(|_| anyhow::anyhow!("global logger already initialized"))?;
    Ok(())
}

pub fn global_log(event: &str, fields: HashMap<String, Value>) -> anyhow::Result<()> {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        logger.log(event, fields)?;
    }
    Ok(())
}
