//! Event queue for passing work items between automation and OCR threads.
//!
//! Uses std::sync::mpsc channel for single-producer, single-consumer communication.
//! The automation thread sends screenshot paths, the OCR worker receives and processes them.

use chrono::{DateTime, Local};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

/// A work item for the OCR worker thread.
#[derive(Debug, Clone)]
pub struct OcrWorkItem {
    /// Path to the screenshot file
    pub screenshot_path: PathBuf,
    /// Iteration number (1-based)
    pub iteration: u32,
    /// Timestamp when screenshot was captured
    pub captured_at: DateTime<Local>,
}

impl OcrWorkItem {
    /// Creates a new work item.
    pub fn new(screenshot_path: PathBuf, iteration: u32) -> Self {
        Self {
            screenshot_path,
            iteration,
            captured_at: Local::now(),
        }
    }
}

/// Creates a new work queue.
///
/// Returns a tuple of (sender, receiver):
/// - The sender is used by the automation thread to queue screenshots
/// - The receiver is used by the OCR worker thread to process them
///
/// The channel is unbounded - items will queue up if OCR is slower than automation.
pub fn create_work_queue() -> (Sender<OcrWorkItem>, Receiver<OcrWorkItem>) {
    channel()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_work_queue_send_receive() {
        let (sender, receiver) = create_work_queue();

        // Send a work item
        let item = OcrWorkItem::new(PathBuf::from("test/screenshot.png"), 1);
        sender.send(item).expect("Failed to send");

        // Receive it
        let received = receiver.recv().expect("Failed to receive");
        assert_eq!(received.iteration, 1);
        assert_eq!(
            received.screenshot_path,
            PathBuf::from("test/screenshot.png")
        );
    }

    #[test]
    fn test_work_queue_multiple_items() {
        let (sender, receiver) = create_work_queue();

        // Send multiple items
        for i in 1..=5 {
            let item = OcrWorkItem::new(PathBuf::from(format!("screenshot_{}.png", i)), i);
            sender.send(item).expect("Failed to send");
        }

        // Receive all items in order
        for i in 1..=5 {
            let received = receiver.recv().expect("Failed to receive");
            assert_eq!(received.iteration, i);
        }
    }

    #[test]
    fn test_channel_closes_when_sender_dropped() {
        let (sender, receiver) = create_work_queue();

        sender
            .send(OcrWorkItem::new(PathBuf::from("test.png"), 1))
            .unwrap();

        // Drop the sender
        drop(sender);

        // First recv should succeed
        assert!(receiver.recv().is_ok());

        // Second recv should fail (channel closed)
        assert!(receiver.recv().is_err());
    }
}
