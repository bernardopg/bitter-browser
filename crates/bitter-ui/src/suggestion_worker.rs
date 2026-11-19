use bitter_core::{bookmarks::Bookmarks, history::History};
use futures_channel::oneshot;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};

pub type Matches = Vec<(String, String)>;

pub struct ResultSet {
    pub bookmarks: Matches,
    pub history: Matches,
}

pub struct SuggestionWorker {
    requests: Sender<(String, oneshot::Sender<ResultSet>)>,
}

impl SuggestionWorker {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        let (requests, incoming) = mpsc::channel::<(String, oneshot::Sender<ResultSet>)>();
        std::thread::Builder::new()
            .name("bitter-suggestions".into())
            .spawn(move || {
                let bookmarks = Bookmarks::open_for_search(&path).ok();
                let history = History::open_for_search(&path).ok();
                while let Ok(first) = incoming.recv() {
                    let (query, outgoing) = incoming.try_iter().last().unwrap_or(first);
                    if outgoing.is_canceled() {
                        continue;
                    }
                    let result = ResultSet {
                        bookmarks: bookmarks
                            .as_ref()
                            .and_then(|db| db.search(&query).ok())
                            .unwrap_or_default(),
                        history: history
                            .as_ref()
                            .and_then(|db| db.search(&query).ok())
                            .unwrap_or_default(),
                    };
                    let _ = outgoing.send(result);
                }
            })?;
        Ok(Self { requests })
    }

    pub fn submit(&self, input: String) -> oneshot::Receiver<ResultSet> {
        let (reply, receiver) = oneshot::channel();
        let _ = self.requests.send((input, reply));
        receiver
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_channel::oneshot::Receiver;
    use std::time::Duration;

    fn wait_for(mut receiver: Receiver<ResultSet>) -> ResultSet {
        for _ in 0..500 {
            if let Ok(Some(result)) = receiver.try_recv() {
                return result;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the suggestion worker did not reply in time");
    }

    #[test]
    fn submit_returns_bookmarks_and_history_matches() {
        let dir = tempfile::tempdir().unwrap();
        let bookmarks = Bookmarks::open(dir.path()).unwrap();
        bookmarks
            .add_bookmark("https://example.test/", Some("Example Site"))
            .unwrap();
        bookmarks.add_bookmark("https://other.test/", None).unwrap();
        let history = History::open(dir.path()).unwrap();
        history.add_visit("https://example.test/", None).unwrap();
        drop(bookmarks);
        drop(history);

        let worker = SuggestionWorker::new(dir.path().to_owned()).unwrap();
        let result = wait_for(worker.submit("example".into()));
        assert_eq!(
            result.bookmarks,
            [(
                "https://example.test/".to_owned(),
                "Example Site".to_owned()
            )]
        );
        assert_eq!(
            result.history,
            [("https://example.test/".to_owned(), "".to_owned())]
        );
        let result = wait_for(worker.submit("nothing".into()));
        assert!(result.bookmarks.is_empty() && result.history.is_empty());
    }
}
