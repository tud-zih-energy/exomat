//! Shorthands to setup logging

use crate::helper::errors::{Error, Result};
use clap_verbosity_flag::Verbosity;
use log::debug;
use std::fs::OpenOptions;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use tracing_subscriber::{
    fmt, fmt::format::DefaultFields, fmt::format::Format, fmt::writer::BoxMakeWriter, fmt::Layer,
    layer::SubscriberExt, prelude::*, reload, reload::Handle, Registry,
};

// note: the concrete type was computed by the compiler
type ReloadHandle = Handle<Layer<Registry, DefaultFields, Format, BoxMakeWriter>, Registry>;

static RELOAD_HANDLE: LazyLock<Mutex<Option<ReloadHandle>>> = LazyLock::new(|| Mutex::new(None));

pub fn set_logfile(logfile_path: &Path) -> Result<()> {
    debug!(
        "trying to open and attach logfile at {}",
        logfile_path.display()
    );
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(logfile_path)?;

    // in tests this does not work, as logging is not properly initialized
    // hence: skip
    if cfg!(test) {
        debug!("in test: skipping actually attaching logfile");
        return Ok(());
    }

    let file_layer = fmt::layer()
        .with_ansi(false) // no color in file (!)
        .with_writer(BoxMakeWriter::new(Mutex::new(file)));

    // okay, funky rust chain of calls to insert the file_layer through a singleton (which is tied
    // into the logging stack), here we go:
    RELOAD_HANDLE
        // (1a) acquire singleton
        .lock()
        // (1b) make error readable
        .map_err(|err| Error::LoggingError(format!("can not lock tracing reload handle: {err}")))?
        // (2) Turn into &mut ReloadHandle, otherwise ok_or() would move out of Option<ReloadHandle>
        .as_mut()
        // (3) unwrap option
        .ok_or(Error::LoggingError(
            "tracing reload handle not available, wtf?".to_string(),
        ))?
        // (4a) overwrite placeholder layer with file layer
        .modify(|layer| *layer = file_layer)
        // (4b) make error readable
        .map_err(|err| Error::LoggingError(format!("could not attach logfile: {err}")))
}

/// setup global logger
///
/// ## Panics
/// - failure during setup
/// - called more than once
pub fn setup_global_logger<T: clap_verbosity_flag::LogLevel>(stderr_verbosity: Verbosity<T>) {
    let initial_layer = fmt::layer().with_writer(BoxMakeWriter::new(std::io::sink));
    let (file_layer, reload_handle) = reload::Layer::new(initial_layer);
    RELOAD_HANDLE
        .lock()
        .expect("failed to acquire lock")
        .replace(reload_handle);

    let stderr_layer = fmt::layer().with_writer(std::io::stderr).with_filter(
        tracing_subscriber::filter::LevelFilter::from(stderr_verbosity),
    );

    let subscriber = Registry::default().with(file_layer).with(stderr_layer);
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global subscriber");
    tracing_log::LogTracer::init().expect("failed to connect log to tracing");
}
