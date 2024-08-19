use std::collections::HashMap;
use std::sync::mpsc;

#[derive(Debug)]
enum ThreadMessage {
    Kill,
}

struct Thread {
    sender: mpsc::Sender<ThreadMessage>,
}

impl Thread {
    fn signal(&self, msg: ThreadMessage) {
        self.sender.send(msg);
    }
}

pub struct ThreadManager {
    threads: HashMap<String, Thread>,
}

impl ThreadManager {
    pub fn new() -> Self {
        ThreadManager {
            threads: HashMap::new(),
        }
    }

    pub fn spawn<F>(&mut self, key: &str, f: F)
        where
            F: Fn() + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel::<ThreadMessage>();
        let thread = Thread { sender: sender };
        let name = String::from(key);
        self.threads.insert(String::from(key), thread);

        std::thread::spawn(move || {
            loop {
                match receiver.try_recv() {
                    Ok(ThreadMessage::Kill) => {
                        println!("killing {}!", name);
                        break;
                    },
                    _ => f(),
                }
            }
        });
    }

    pub fn kill(&self, key: &str) {
        if let Some(thread) = self.threads.get(key) {
            thread.signal(ThreadMessage::Kill);
        }
    }
}
