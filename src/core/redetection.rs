use std::time::{Duration, Instant};

use log::{info, warn};

use crate::core::capabilities::Capabilities;

/// Outcome of one run of the collectors, as far as the detection of the capabilities is concerned
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CycleOutcome {
    /// The box no longer knew the session of the exporter: it restarted
    pub session_lost: bool,
    pub succeeded: usize,
    pub failed: usize,
}

/// What to do with the capabilities just detected
#[derive(Debug, PartialEq)]
pub enum Decision {
    /// Keep the current collectors
    Keep,
    /// Rebuild the collectors for these capabilities
    Rebuild(Capabilities),
}

/// When to detect the capabilities of the box again, and when to apply them.
///
/// The capabilities were detected at startup only, while a firmware update of the box can change
/// them, and a capability may not be detected at startup. A detection is pending:
/// - after a restart of the box: the session was lost, or every collector failed and then one
///   succeeded again (the clients may have logged in again without any failure);
/// - periodically, for a change without restart, e.g. the network mode;
/// - while a capability is unknown.
///
/// While pending, the box is asked at most once per delay, doubled after each failed or
/// incomplete detection up to `max_delay`. A change is applied once two consecutive complete
/// detections agree: right after its restart, the box may give a complete but transient answer.
pub struct Redetection {
    retry: Duration,
    max_delay: Duration,
    period: Duration,
    pending: bool,
    delay: Duration,
    next_attempt: Instant,
    last_detection: Instant,
    every_collector_failed: bool,
    /// A restart was announced since the last detection
    restart_announced: bool,
    candidate: Option<Capabilities>,
}

impl Redetection {
    /// `current` are the capabilities detected at startup, at `now`
    pub fn new(
        current: &Capabilities,
        now: Instant,
        retry: Duration,
        max_delay: Duration,
        period: Duration,
    ) -> Self {
        let pending = !current.is_complete();
        if pending {
            warn!("some capabilities of the box are unknown: they will be detected again");
        }
        Self {
            retry,
            max_delay,
            period,
            pending,
            delay: retry,
            next_attempt: now + retry,
            last_detection: now,
            every_collector_failed: false,
            restart_announced: false,
            candidate: None,
        }
    }

    /// Records the outcome of a run of the collectors. Returns whether it announced a restart of
    /// the box: once per restart, though several outcomes may show it (the session lost, then the
    /// collectors back after all of them failed)
    pub fn observe(&mut self, outcome: CycleOutcome, now: Instant) -> bool {
        let recovered = self.every_collector_failed && outcome.succeeded > 0;
        if outcome.succeeded + outcome.failed > 0 {
            self.every_collector_failed = outcome.succeeded == 0;
        }
        let periodic = now.duration_since(self.last_detection) >= self.period;

        let restarted = outcome.session_lost || recovered;
        let announced = restarted && !self.restart_announced;
        if announced {
            info!("the box restarted, or was unreachable: its capabilities will be detected again");
            self.restart_announced = true;
        }
        if restarted {
            // also while a detection is pending: right after a restart is the time to detect
            // them, rather than at the end of a long delay; leave the box time to start all its
            // services first
            self.pending = true;
            self.delay = self.retry;
            self.next_attempt = now + self.retry;
        } else if !self.pending && periodic {
            self.pending = true;
            self.delay = self.retry;
            self.next_attempt = now;
        }
        announced
    }

    /// Whether to detect the capabilities now; never while the box refuses the login
    pub fn due(&self, now: Instant, login_refused: bool) -> bool {
        self.pending && !login_refused && now >= self.next_attempt
    }

    /// Takes the result of a detection, None when it failed
    pub fn decide(
        &mut self,
        detected: Option<Capabilities>,
        current: &Capabilities,
        now: Instant,
    ) -> Decision {
        self.last_detection = now;
        self.restart_announced = false;

        let detected = match detected {
            Some(caps) if caps.is_complete() => caps,
            _ => {
                if self.candidate.take().is_some() || self.delay == self.retry {
                    warn!(
                        "the capabilities of the box could not be detected: attempting again later"
                    );
                }
                self.next_attempt = now + self.delay;
                self.delay = (self.delay * 2).min(self.max_delay);
                return Decision::Keep;
            }
        };

        if &detected == current {
            if self.candidate.take().is_some() || self.pending {
                info!("capabilities checked again: unchanged");
            }
            self.settle(now);
            return Decision::Keep;
        }

        if self.candidate.as_ref() == Some(&detected) {
            self.settle(now);
            return Decision::Rebuild(detected);
        }

        match &self.candidate {
            // the box answers differently at each detection: keep the delay growing
            Some(previous) => {
                info!(
                    "capabilities still changing ({}), confirming later",
                    previous.changes(&detected)
                );
                self.next_attempt = now + self.delay;
                self.delay = (self.delay * 2).min(self.max_delay);
            }
            // confirm it once more, a retry later
            None => {
                info!(
                    "capabilities changed ({}), confirming",
                    current.changes(&detected)
                );
                self.delay = self.retry;
                self.next_attempt = now + self.retry;
            }
        }
        self.candidate = Some(detected);
        Decision::Keep
    }

    /// The rebuild for the capabilities just decided failed: detect them again later
    pub fn rebuild_failed(&mut self, now: Instant) {
        self.pending = true;
        self.next_attempt = now + self.delay;
        self.delay = (self.delay * 2).min(self.max_delay);
    }

    fn settle(&mut self, now: Instant) {
        self.pending = false;
        self.candidate = None;
        self.delay = self.retry;
        self.last_detection = now;
    }

    #[cfg(test)]
    fn pending(&self) -> bool {
        self.pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RETRY: Duration = Duration::from_secs(60);
    const MAX: Duration = Duration::from_secs(15 * 60);
    const PERIOD: Duration = Duration::from_secs(3600);

    /// Capabilities of a box in router mode; `api` None when its API version is unknown
    fn caps(api: Option<u32>) -> Capabilities {
        let at_least = |v: u32| api.map(|a| a >= v);
        Capabilities {
            connection: Some(true),
            system: Some(true),
            lan: Some(true),
            lan_browser: Some(true),
            switch: Some(true),
            wifi: Some(true),
            dhcp: Some(true),
            nas: Some(true),
            vm: at_least(7),
            raid: at_least(8),
            vpn: at_least(4),
            ddns: at_least(4),
            v6_system: at_least(6),
            network_mode: Some("router".to_string()),
            api_version: api.map(|a| format!("{a}.0")),
            model: None,
            device_name: None,
        }
    }

    fn outcome(session_lost: bool, succeeded: usize, failed: usize) -> CycleOutcome {
        CycleOutcome {
            session_lost,
            succeeded,
            failed,
        }
    }

    /// Time from `now` until a detection is due, by steps of a second; fails beyond two hours
    /// rather than waiting forever
    fn until_due(r: &Redetection, now: Instant) -> Duration {
        let mut wait = Duration::ZERO;
        while !r.due(now + wait, false) {
            wait += Duration::from_secs(1);
            assert!(wait <= Duration::from_secs(7200), "never due");
        }
        wait
    }

    fn settled(now: Instant) -> Redetection {
        Redetection::new(&caps(Some(7)), now, RETRY, MAX, PERIOD)
    }

    #[test]
    fn complete_capabilities_at_startup_are_not_detected_again() {
        let t0 = Instant::now();
        let r = settled(t0);
        assert!(!r.due(t0 + RETRY * 10, false));
    }

    #[test]
    fn unknown_capabilities_at_startup_are_detected_again_after_the_retry_delay() {
        let t0 = Instant::now();
        let r = Redetection::new(&caps(None), t0, RETRY, MAX, PERIOD);
        assert!(!r.due(t0, false));
        assert!(r.due(t0 + RETRY, false));
    }

    #[test]
    fn a_lost_session_starts_a_detection_once_the_box_had_time_to_start() {
        let t0 = Instant::now();
        let mut r = settled(t0);
        r.observe(outcome(true, 0, 13), t0);
        assert!(!r.due(t0 + RETRY / 2, false));
        assert!(r.due(t0 + RETRY, false));
    }

    #[test]
    fn collectors_back_after_all_of_them_failed_start_a_detection() {
        let t0 = Instant::now();
        let mut r = settled(t0);
        r.observe(outcome(false, 0, 13), t0);
        assert!(
            !r.due(t0 + RETRY, false),
            "failures alone are not a restart"
        );
        r.observe(outcome(false, 13, 0), t0 + RETRY);
        assert!(r.due(t0 + RETRY * 2, false));
    }

    #[test]
    fn a_restart_seen_twice_before_the_detection_is_announced_once() {
        let t0 = Instant::now();
        let mut r = settled(t0);
        assert!(r.observe(outcome(true, 0, 13), t0), "the lost session");
        assert!(
            !r.observe(outcome(false, 13, 0), t0 + RETRY / 4),
            "the collectors back after the lost session: the same restart"
        );
        assert!(
            r.due(t0 + RETRY / 4 + RETRY, false),
            "the second sign still leaves the box time to start"
        );
        assert!(!r.due(t0 + RETRY / 4 + RETRY / 2, false));

        let t1 = t0 + RETRY * 2;
        assert_eq!(
            r.decide(Some(caps(Some(7))), &caps(Some(7)), t1),
            Decision::Keep
        );
        assert!(
            r.observe(outcome(true, 0, 13), t1 + RETRY),
            "a restart after the detection is a new one"
        );
    }

    #[test]
    fn a_restart_after_a_failed_detection_is_announced_again() {
        let t0 = Instant::now();
        let mut r = settled(t0);
        assert!(r.observe(outcome(true, 0, 13), t0));
        assert_eq!(r.decide(None, &caps(Some(7)), t0 + RETRY), Decision::Keep);
        assert!(r.observe(outcome(true, 0, 13), t0 + RETRY * 5));
    }

    #[test]
    fn a_partial_failure_is_not_a_restart() {
        let t0 = Instant::now();
        let mut r = settled(t0);
        r.observe(outcome(false, 12, 1), t0);
        r.observe(outcome(false, 13, 0), t0);
        assert!(!r.due(t0 + RETRY * 10, false));
    }

    #[test]
    fn the_capabilities_are_detected_again_periodically() {
        let t0 = Instant::now();
        let mut r = settled(t0);
        r.observe(outcome(false, 13, 0), t0 + PERIOD / 2);
        assert!(!r.due(t0 + PERIOD / 2, false));
        r.observe(outcome(false, 13, 0), t0 + PERIOD);
        assert!(r.due(t0 + PERIOD, false));
    }

    #[test]
    fn nothing_is_detected_while_the_box_refuses_the_login() {
        let t0 = Instant::now();
        let r = Redetection::new(&caps(None), t0, RETRY, MAX, PERIOD);
        assert!(!r.due(t0 + RETRY, true));
    }

    #[test]
    fn a_change_is_applied_once_confirmed() {
        let t0 = Instant::now();
        let current = caps(Some(7));
        let mut r = settled(t0);
        r.observe(outcome(true, 0, 13), t0);

        assert_eq!(
            Decision::Keep,
            r.decide(Some(caps(Some(8))), &current, t0 + RETRY)
        );
        assert!(
            r.due(t0 + RETRY * 2, false),
            "the change is confirmed a retry later"
        );
        assert_eq!(
            Decision::Rebuild(caps(Some(8))),
            r.decide(Some(caps(Some(8))), &current, t0 + RETRY * 2)
        );
        assert!(!r.pending());
    }

    /// e.g. the box replaced by another model: its model metric is rebuilt like any capability
    #[test]
    fn a_new_model_of_the_box_is_applied_once_confirmed() {
        let t0 = Instant::now();
        let current = caps(Some(7));
        let mut other = caps(Some(7));
        other.model = Some(crate::mappers::system::BoxModel {
            model: Some("fbxgw9-r1/full".to_string()),
            model_name: Some("Freebox v9 (r1)".to_string()),
        });
        let mut r = settled(t0);
        r.observe(outcome(true, 0, 13), t0);

        assert_eq!(
            Decision::Keep,
            r.decide(Some(other.clone()), &current, t0 + RETRY)
        );
        assert_eq!(
            Decision::Rebuild(other.clone()),
            r.decide(Some(other), &current, t0 + RETRY * 2)
        );
    }

    #[test]
    fn a_transient_answer_is_not_applied() {
        let t0 = Instant::now();
        let current = caps(Some(7));
        let mut r = settled(t0);
        r.observe(outcome(true, 0, 13), t0);

        // right after its restart, the box does not list its sensors yet
        let mut transient = caps(Some(8));
        transient.v6_system = Some(false);
        assert_eq!(
            Decision::Keep,
            r.decide(Some(transient), &current, t0 + RETRY)
        );
        assert_eq!(
            Decision::Keep,
            r.decide(Some(caps(Some(8))), &current, t0 + RETRY * 2)
        );
        assert_eq!(
            Decision::Rebuild(caps(Some(8))),
            r.decide(Some(caps(Some(8))), &current, t0 + RETRY * 3)
        );
    }

    #[test]
    fn unchanged_capabilities_end_the_detection() {
        let t0 = Instant::now();
        let current = caps(Some(7));
        let mut r = settled(t0);
        r.observe(outcome(true, 0, 13), t0);

        assert_eq!(
            Decision::Keep,
            r.decide(Some(caps(Some(7))), &current, t0 + RETRY)
        );
        assert!(!r.pending());
    }

    #[test]
    fn a_failed_or_incomplete_detection_is_attempted_again_less_and_less_often() {
        let t0 = Instant::now();
        let current = caps(None);
        let mut r = Redetection::new(&current, t0, RETRY, MAX, PERIOD);

        let mut now = t0 + RETRY;
        let mut waits = vec![];
        for attempt in 0..8 {
            assert!(r.due(now, false), "attempt {attempt}");
            // a failure, or an answer still missing the API version
            let detected = (attempt % 2 == 0).then(|| caps(None));
            assert_eq!(Decision::Keep, r.decide(detected, &current, now));
            assert!(r.pending());
            let wait = until_due(&r, now);
            waits.push(wait.as_secs());
            now += wait;
        }
        assert_eq!(vec![60, 120, 240, 480, 900, 900, 900, 900], waits);
    }

    #[test]
    fn a_restart_during_a_long_delay_attempts_again_soon() {
        let t0 = Instant::now();
        let current = caps(None);
        let mut r = Redetection::new(&current, t0, RETRY, MAX, PERIOD);
        let mut now = t0 + RETRY;
        // the delay grows while the API version stays unknown
        for _ in 0..6 {
            now += until_due(&r, now);
            r.decide(None, &current, now);
        }

        r.observe(outcome(true, 0, 13), now);
        assert!(r.due(now + RETRY, false));
    }

    #[test]
    fn answers_changing_at_each_detection_keep_the_delay_growing() {
        let t0 = Instant::now();
        let current = caps(Some(7));
        let mut r = settled(t0);
        r.observe(outcome(true, 0, 13), t0);

        let mut flipped = caps(Some(8));
        flipped.wifi = Some(false);
        let mut now = t0 + RETRY;
        let mut waits = vec![];
        for attempt in 0..5 {
            let detected = if attempt % 2 == 0 {
                caps(Some(8))
            } else {
                flipped.clone()
            };
            assert_eq!(Decision::Keep, r.decide(Some(detected), &current, now));
            let wait = until_due(&r, now);
            waits.push(wait.as_secs());
            now += wait;
        }
        assert_eq!(vec![60, 60, 120, 240, 480], waits);
    }

    #[test]
    fn a_failed_rebuild_is_attempted_again() {
        let t0 = Instant::now();
        let current = caps(Some(7));
        let mut r = settled(t0);
        r.observe(outcome(true, 0, 13), t0);
        r.decide(Some(caps(Some(8))), &current, t0 + RETRY);
        r.decide(Some(caps(Some(8))), &current, t0 + RETRY * 2);

        r.rebuild_failed(t0 + RETRY * 2);
        assert!(r.pending());
        assert!(r.due(t0 + RETRY * 3, false));
    }
}
