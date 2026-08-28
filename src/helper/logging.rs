//! Shorthands to setup logging

use crate::helper::errors::{Error, Result};
use clap_verbosity_flag::Verbosity;
use log::{debug, error, trace};
use std::fs::OpenOptions;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::{LazyLock, Mutex};
use tracing_indicatif::IndicatifLayer;
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

    let stderr_filter = tracing_subscriber::filter::LevelFilter::from(stderr_verbosity);

    let indicatif_layer = IndicatifLayer::new();
    let stderr_layer = fmt::layer()
        .with_writer(indicatif_layer.get_stderr_writer())
        .with_filter(stderr_filter);

    let subscriber = Registry::default()
        .with(file_layer) // gets everything
        .with(stderr_layer) // gets as configured by cli
        .with(indicatif_layer.with_filter(stderr_filter)); // needs filter again, but can't have it
                                                           // above due to type-fuckery
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global subscriber");
    tracing_log::LogTracer::init().expect("failed to connect log to tracing");
}

pub fn run_cmd_forwards_output(experiment: &str, mut cmd: Command) -> Result<Output> {
    use std::io::BufRead;

    let mut cmd = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            error!("invoking run for {experiment} failed: {e}");
            Error::HarnessRunError {
                experiment: experiment.to_string(),
                err: format!("could not spawn command: {e}"),
            }
        })?;

    trace!("attaching handlers to stdout/stderr of run from {experiment}");
    let stdout = cmd.stdout.take().ok_or(Error::HarnessRunError {
        experiment: experiment.to_string(),
        err: format!("failed to capture stdout of run from {experiment}"),
    })?;
    let stderr = cmd.stderr.take().ok_or(Error::HarnessRunError {
        experiment: experiment.to_string(),
        err: format!("failed to capture stderr of run from {experiment}"),
    })?;

    debug!("creating async runtime to handle output of run from {experiment}");
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        // .enable_io()
        .build()
        .map_err(Error::TokioError)?;
    let output = rt.block_on(async move {
        // to use experiment inside closure, it needs a clone which can be moved inside the closure
        let experiment_clone = experiment.to_string();
        let stdout_handler = tokio::spawn(async move {
            let stdout_reader = std::io::BufReader::new(stdout);
            for line in stdout_reader.lines() {
                match line {
                    Ok(line) => debug!("{experiment_clone} (stdout)> {line}"),
                    Err(e) => error!("failed to read from stdout of {experiment_clone}: {e}"),
                }
            }
        });

        let experiment_clone = experiment.to_string();
        let stderr_handler = tokio::spawn(async move {
            let stderr_reader = std::io::BufReader::new(stderr);
            for line in stderr_reader.lines() {
                match line {
                    Ok(line) => debug!("{experiment_clone} (stderr)> {line}"),
                    Err(e) => error!("failed to read from stderr of {experiment_clone}: {e}"),
                }
            }
        });

        trace!("handler attached, waiting for {experiment} to finish");
        let status = cmd.wait_with_output().map_err(|e| {
            error!("run of {experiment} failed: {e}");
            Error::HarnessRunError {
                experiment: experiment.to_string(),
                err: format!("could not wait for command: {e}"),
            }
        })?;

        stdout_handler.await?;
        stderr_handler.await?;

        Ok::<_, Error>(status)
    })?;

    Ok(output)
}
