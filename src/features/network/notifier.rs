use communication;
use error::*;
use wrapper::channel;
use wrapper::process;
use wrapper::thread;

pub(super) struct Notifier {
    id: usize,
    sender: channel::Sender<communication::Message>,
}

impl Notifier {
    pub(super) fn new(id: usize, sender: channel::Sender<communication::Message>) -> Self {
        Self { id, sender }
    }
}

impl thread::Runnable for Notifier {
    fn run(&self) -> Result<()> {
        let command = process::Command::new("ip", &["monitor", "address", "link"]);

        command.listen_stdout(|| {
            // check 2 times for updates with a 2 seconds delay
            for _ in 0..2 {
                thread::sleep_secs(2);
                communication::send_message(self.id, &self.sender)?;
            }

            Ok(())
        })?;

        Ok(())
    }
}
