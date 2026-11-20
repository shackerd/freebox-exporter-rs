use std::error::Error;

/// The text of `error` followed by the text of each of its causes, separated by `: `.
///
/// `{error}` alone shows only the first level: for a request to the box,
/// `error sending request for url (…)`, without saying whether the connection was refused,
/// timed out or its certificate rejected. A cause the message already ends with is skipped,
/// since some errors repeat their cause at the end of their own text.
pub fn with_causes(error: &(dyn Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(current) = cause {
        let cause_text = current.to_string();
        if !text.ends_with(&cause_text) {
            text.push_str(": ");
            text.push_str(&cause_text);
        }
        cause = current.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::with_causes;
    use std::error::Error;
    use std::fmt;

    #[derive(Debug)]
    struct Layer {
        text: &'static str,
        cause: Option<Box<Layer>>,
    }

    impl Layer {
        fn new(text: &'static str, cause: Option<Layer>) -> Self {
            Self {
                text,
                cause: cause.map(Box::new),
            }
        }
    }

    impl fmt::Display for Layer {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.text)
        }
    }

    impl Error for Layer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.cause.as_deref().map(|c| c as &(dyn Error + 'static))
        }
    }

    #[test]
    fn every_cause_follows_the_error_in_order() {
        let error = Layer::new(
            "top",
            Some(Layer::new("middle", Some(Layer::new("bottom", None)))),
        );
        assert_eq!(with_causes(&error), "top: middle: bottom");
    }

    #[test]
    fn an_error_without_cause_is_unchanged() {
        assert_eq!(with_causes(&Layer::new("alone", None)), "alone");
    }

    #[test]
    fn a_cause_the_text_ends_with_is_not_repeated() {
        let error = Layer::new(
            "request failed: refused",
            Some(Layer::new(
                "refused",
                Some(Layer::new("os error 111", None)),
            )),
        );
        assert_eq!(with_causes(&error), "request failed: refused: os error 111");
    }

    #[test]
    fn a_cause_found_elsewhere_in_the_text_is_kept() {
        let error = Layer::new(
            "timed out waiting for the lock",
            Some(Layer::new("timed out", None)),
        );
        assert_eq!(
            with_causes(&error),
            "timed out waiting for the lock: timed out"
        );
    }

    #[tokio::test]
    async fn a_refused_connection_names_its_cause() {
        // a port that was just released: nothing listens on it
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);

        // without proxy: a proxy of the environment would answer instead of refusing
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let error = client
            .get(format!("http://{address}/"))
            .send()
            .await
            .unwrap_err();

        let mut cause = error.source();
        let refused = loop {
            let current = cause.expect("the refused connection is among the causes");
            if let Some(io) = current.downcast_ref::<std::io::Error>() {
                break io;
            }
            cause = current.source();
        };
        assert_eq!(refused.kind(), std::io::ErrorKind::ConnectionRefused);

        let text = with_causes(&error);
        assert!(text.starts_with(&error.to_string()), "{text}");
        assert!(text.contains(&refused.to_string()), "{text}");
    }
}
