use flexi_logger::filter::{self, LogLineFilter};

pub struct CustomLogFilter;

impl LogLineFilter for CustomLogFilter {
    fn write(
        &self,
        now: &mut flexi_logger::DeferredNow,
        record: &log::Record,
        log_line_writer: &dyn filter::LogLineWriter,
    ) -> std::io::Result<()> {
        let path = record.module_path().unwrap_or_default();

        if record.level() != log::Level::Debug
            && (path.starts_with("reqwest") || path.starts_with("prometheus_exporter"))
        {
            return Ok(());
        }

        log_line_writer.write(now, record)
    }
}
