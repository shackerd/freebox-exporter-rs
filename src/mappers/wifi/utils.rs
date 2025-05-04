use super::models::ChannelSurveyHistory;

pub fn calculate_avg_channel_survey_history(
    histories: &[ChannelSurveyHistory],
) -> ChannelSurveyHistory {
    let len = histories.len() as u32;

    let busy_avg = histories
        .iter()
        .map(|f| f.busy_percent.unwrap_or(0) as u32)
        .sum::<u32>()
        / len;
    let tx_avg = histories
        .iter()
        .map(|f| f.tx_percent.unwrap_or(0) as u32)
        .sum::<u32>()
        / len;
    let rx_bss_avg = histories
        .iter()
        .map(|f| f.rx_bss_percent.unwrap_or(0) as u32)
        .sum::<u32>()
        / len;
    let rx_avg = histories
        .iter()
        .map(|f| f.rx_percent.unwrap_or(0) as u32)
        .sum::<u32>()
        / len;

    ChannelSurveyHistory {
        timestamp: Some(chrono::offset::Local::now().timestamp() as u64),
        busy_percent: Some(busy_avg as u8),
        tx_percent: Some(tx_avg as u8),
        rx_bss_percent: Some(rx_bss_avg as u8),
        rx_percent: Some(rx_avg as u8),
    }
}

pub fn get_recent_channel_entries(
    histories: &[ChannelSurveyHistory],
    max_range: usize,
) -> Vec<ChannelSurveyHistory> {
    let len = histories.len() as u32;

    match len {
        0 => vec![ChannelSurveyHistory {
            timestamp: Some(chrono::offset::Local::now().timestamp() as u64),
            busy_percent: Some(0),
            tx_percent: Some(0),
            rx_bss_percent: Some(0),
            rx_percent: Some(0),
        }],
        l => {
            if l <= max_range as u32 {
                histories.to_vec()
            } else {
                // take only the last x entries
                let mut hist = histories.to_vec();
                hist.sort_by(|a, b| b.timestamp.unwrap().cmp(&a.timestamp.unwrap()));
                hist.split_at(len as usize - max_range).1.to_vec()
            }
        }
    }
}