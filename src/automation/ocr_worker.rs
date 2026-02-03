//! OCR worker thread for processing screenshots.
//!
//! Runs in a separate thread, receiving screenshot paths from the work queue
//! and processing them with OCR. Results are written to a CSV file.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crate::automation::csv_writer::{append_to_csv, append_to_raw_csv};
use crate::automation::queue::OcrWorkItem;
use crate::ocr::ocr_screenshot;

/// Runs the OCR worker loop.
///
/// Processes items from the queue until the channel is closed (sender dropped).
/// Each screenshot is loaded, processed with OCR, and results appended to CSV.
///
/// This function blocks until the channel closes, so it should be run in a
/// dedicated thread.
pub fn run_ocr_worker(receiver: Receiver<OcrWorkItem>, csv_path: PathBuf, ocr_threshold: u8) {
    crate::log("OCR worker started");

    loop {
        match receiver.recv() {
            Ok(work_item) => {
                crate::log(&format!(
                    "OCR worker: processing iteration {} ({})",
                    work_item.iteration,
                    work_item.screenshot_path.display()
                ));

                // Load screenshot from disk
                let img = match image::open(&work_item.screenshot_path) {
                    Ok(img) => img.to_rgba8(),
                    Err(e) => {
                        crate::log(&format!(
                            "OCR worker: failed to load {}: {}",
                            work_item.screenshot_path.display(),
                            e
                        ));
                        continue; // Skip this item, continue with next
                    }
                };

                // Run OCR
                let scores = match ocr_screenshot(&img, ocr_threshold) {
                    Ok(scores) => scores,
                    Err(e) => {
                        crate::log(&format!(
                            "OCR worker: OCR failed for iteration {}: {}",
                            work_item.iteration, e
                        ));
                        continue; // Skip this item, continue with next
                    }
                };

                // Log the extracted scores
                crate::log(&format!(
                    "OCR complete for iteration {}: Stage1={:?}, Stage2={:?}, Stage3={:?}",
                    work_item.iteration, scores[0], scores[1], scores[2]
                ));

                // Append to CSV
                if let Err(e) = append_to_csv(&csv_path, &work_item, &scores) {
                    crate::log(&format!(
                        "OCR worker: failed to write CSV for iteration {}: {}",
                        work_item.iteration, e
                    ));
                    // Continue anyway - the screenshot is saved for manual retry
                }

                // Append to raw CSV (just scores, no header)
                let raw_csv_path = csv_path.with_file_name("rehearsal_data.csv");
                if let Err(e) = append_to_raw_csv(&raw_csv_path, &scores) {
                    crate::log(&format!(
                        "OCR worker: failed to write raw CSV for iteration {}: {}",
                        work_item.iteration, e
                    ));
                }
            }
            Err(_) => {
                // Channel closed, sender was dropped
                crate::log("OCR worker: channel closed, exiting");
                break;
            }
        }
    }

    crate::log("OCR worker finished");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::csv_writer::init_csv;
    use crate::automation::queue::create_work_queue;
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn test_worker_exits_when_channel_closes() {
        let dir = tempdir().unwrap();
        let csv_path = dir.path().join("test.csv");
        init_csv(&csv_path).unwrap();

        let (sender, receiver) = create_work_queue();

        // Spawn worker
        let csv_path_clone = csv_path.clone();
        let handle = thread::spawn(move || {
            run_ocr_worker(receiver, csv_path_clone, 190);
        });

        // Drop sender to close channel
        drop(sender);

        // Worker should exit
        handle.join().expect("Worker thread panicked");
    }
}
