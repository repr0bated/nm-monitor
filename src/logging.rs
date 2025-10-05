use log::LevelFilter;

pub fn init_logging() {
    if systemd_journal_logger::JournalLog::new()
        .and_then(|j| Ok(j.install()))
        .is_ok()
    {
        log::set_max_level(LevelFilter::Info);
        return;
    }
    // Fallback to env_logger if journald is not available (e.g., dev containers)
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
    log::set_max_level(LevelFilter::Info);
}
