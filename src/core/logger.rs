use flexi_logger::filter::{self, LogLineFilter};

pub struct IgnoreReqwest;

impl LogLineFilter for IgnoreReqwest {
    fn write(
        &self,
        now: &mut flexi_logger::DeferredNow,
        record: &log::Record,
        log_line_writer: &dyn filter::LogLineWriter,
    ) -> std::io::Result<()> {
        let path = record.module_path().unwrap_or_default();

        if path.starts_with("reqwest") || path.starts_with("prometheus_exporter") {
            return Ok(());
        }

        log_line_writer.write(now, record)
    }
}
