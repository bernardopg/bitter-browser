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
