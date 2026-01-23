pub mod harness {
    pub mod env;
    pub mod run;
    pub mod skeleton;
    pub mod table;
}
pub mod helper {
    pub mod archivist;
    pub mod errors;
    pub mod fs_names;
}

use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use spdlog::formatter::{pattern, PatternFormatter};
use spdlog::sink::FileSink;
use std::{path::PathBuf, sync::Arc};

use helper::archivist::find_marker_pwd;
use helper::errors::Error;
use helper::fs_names::*;

/// Initializes logging for all severity levels from info and up.
///
/// Logging will still work if this function is not called, however, only
/// messages of info-severity or higher will be recorded.
///
/// Logging messages will be handled by `spdlog` and printed to stdout.
///
/// ## Usage of return value
/// The returned `Multiprogress` can be used to stop log messages from interfering
/// with an `indicatif` progress bar. If you do not have a progress bar, you can
/// savely ignore this.
///
/// ```
/// use exomat::activate_logging;
/// use log::info;
/// use indicatif::{MultiProgress, ProgressBar};
///
/// let logging_handler = activate_logging(log::LevelFilter::Info);
/// let prog_bar = ProgressBar::new(42);
///
/// // protect progress bar from log Interference
/// let prog_bar = logging_handler.add(prog_bar);
///
/// // do work
/// for i in 1..10 {
///     info!("This will look nice and not intermingle!");
///     prog_bar.inc(1);
/// }
///
/// prog_bar.finish();
/// ```
pub fn activate_logging(verbosity: log::LevelFilter) -> MultiProgress {
    // configure the logger, default logger does not work because it gets messed up
    // when having multiple sinks with different level filters
    let pattern = pattern!("[{date} {time}.{millisecond}] [{level}] {payload}{eol}");
    let logger = spdlog::Logger::builder()
        .level_filter(spdlog::LevelFilter::All)
        .sink(Arc::new(
            spdlog::sink::StdStreamSink::builder()
                .formatter(Box::new(PatternFormatter::new(pattern)))
                .level_filter(spdlog::LevelFilter::from(verbosity))
                .std_stream(spdlog::sink::StdStream::Stdout)
                .build()
                .unwrap(),
        ))
        .build()
        .unwrap();

    // configure the logger, init spdlog, log and log_bridge
    spdlog::set_default_logger(Arc::new(logger));
    spdlog::re_export::log::set_max_level(spdlog::re_export::log::LevelFilter::Trace);

    let multi = MultiProgress::new();
    let log_wrapper = LogWrapper::new(multi.clone(), spdlog::log_crate_proxy());
    log_wrapper.try_init().map_err(Error::LoggerError).unwrap();

    multi
}

/// Disables log output to stdout.
///
/// warning: will reset the effect of duplicate_log_to_file()!
fn disable_console_log() {
    // create logger that logs to log_file
    let pattern = pattern!("[{date} {time}.{millisecond}] [{level}] {payload}{eol}");
    let new_logger = spdlog::Logger::builder()
        .level_filter(spdlog::LevelFilter::All)
        .sink(Arc::new(
            spdlog::sink::StdStreamSink::builder()
                .formatter(Box::new(PatternFormatter::new(pattern)))
                .level_filter(spdlog::LevelFilter::Off)
                .std_stream(spdlog::sink::StdStream::Stdout)
                .build()
                .unwrap(),
        ))
        .build()
        .unwrap();

    // update logger
    spdlog::set_default_logger(Arc::new(new_logger));
}

/// Duplicate logging messages to `log_file`.
///
/// This does not overwrite previous configurations of the logger. It simply adds
/// `log_file` as an additional output for log messages without a level filter.
///
/// If the default logger was not initilized by `activate_logging()` before, this
/// will not initialize the logger, so no messages will be written to the file.
pub fn duplicate_log_to_file(log_file: &PathBuf) {
    let pattern = pattern!("[{date} {time}.{millisecond}] [{level}] {payload}{eol}");

    // create logger that logs to log_file
    let new_logger = spdlog::default_logger()
        .fork_with(|new| {
            let file_sink = Arc::new(
                FileSink::builder()
                    .formatter(Box::new(PatternFormatter::new(pattern)))
                    .level_filter(spdlog::LevelFilter::All)
                    .path(log_file)
                    .build()?,
            );
            new.sinks_mut().push(file_sink);
            Ok(())
        })
        .map_err(|e| Error::HarnessCreateError {
            entry: log_file.display().to_string(),
            reason: e.to_string(),
        })
        .expect("Could not create new logger: {e}");

    // update logger
    spdlog::set_default_logger(new_logger);
}

#[cfg(test)]
mod tests {
    use super::*;

    use log::{error, info, trace, warn};
    use rusty_fork::rusty_fork_test;
    use tempfile::NamedTempFile;

    rusty_fork_test! {

        // this is in here to that logging is not enables for all following tests
        #[test]
        fn log_to_file() {
            let log = NamedTempFile::with_suffix("log").unwrap();
            let log = log.path().to_path_buf();

            activate_logging(log::LevelFilter::Info);
            trace!("Trace on console");
            info!("Info on console");
            warn!("Warn on console");
            error!("Error on console");

            duplicate_log_to_file(&log);
            trace!("Trace in file");
            info!("Info in file");
            warn!("Warn in file");
            error!("Error in file");

            // simulate program ending
            spdlog::default_logger().flush();

            let file_content = std::fs::read_to_string(&log).unwrap();
            print!("log: {file_content}\n");

            assert!(file_content.contains("Trace in file"));
            assert!(file_content.contains("Info in file"));
            assert!(file_content.contains("Warn in file"));
            assert!(file_content.contains("Error in file"));
            assert!(!file_content.contains("on console"));
        }
    }

    #[test]
    fn collect_out_no_files() {
        // collect on dir without out_* files
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();

        std::fs::create_dir_all(&series_dir).unwrap();
        std::fs::File::create(series_dir.join("random_file")).unwrap();

        let res = collect_output(&series_dir).unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn collect_out_empty() {
        // collect on dir with out_* file, without content
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_dir = series_dir.join("runs/run_0_rep0");

        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::File::create(run_dir.join("out_empty")).unwrap();

        let res = collect_output(&series_dir).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res.get("out_empty").unwrap(), "");
    }

    #[test]
    fn collect_out_working() {
        // collect on dir with out_* files, with content
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_dir = series_dir.join("runs/run_0_rep0");

        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("out_full"), "foo bar").unwrap();

        let res = collect_output(&series_dir).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res.get("out_full").unwrap(), "foo bar");
    }

    #[test]
    #[should_panic]
    fn collect_out_too_many_runs() {
        // collect on dir with out_* files from multiple runs
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_dir_1 = series_dir.join("runs/run_0_rep0");
        let run_dir_2 = series_dir.join("runs/run_7_rep4");

        std::fs::create_dir_all(&run_dir_1).unwrap();
        std::fs::write(run_dir_1.join("out_1"), "foo bar").unwrap();

        std::fs::create_dir_all(&run_dir_2).unwrap();
        std::fs::write(run_dir_2.join("out_1"), "something else").unwrap();

        let _this_panics = collect_output(&series_dir);
    }
}
