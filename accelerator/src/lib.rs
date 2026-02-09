use pyo3::prelude::*;
use crate::matcher::EnhancedMatcher;
use crate::scanner::parallel::ParallelScanner;
use crate::types::{ScanConfig, HotFragment};
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::thread;
use std::time::Duration;
use std::sync::mpsc;

pub mod matcher;
pub mod scanner;
pub mod types;
pub mod exfat;
pub mod fragment_linker;
pub mod simd_search;

#[pyclass]
struct RustPatternMatcher {
    matcher: EnhancedMatcher,
}

#[pymethods]
impl RustPatternMatcher {
    #[new]
    fn new() -> Self {
        RustPatternMatcher {
            matcher: EnhancedMatcher::new(),
        }
    }

    fn scan_chunk(&mut self, py: Python, data: &[u8], offset: usize, deduplicate: bool) -> PyResult<Vec<PyObject>> {
        let results = self.matcher.scan_chunk(data, offset, deduplicate);
        
        let mut py_results = Vec::with_capacity(results.len());
        for link in results {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("url", link.url)?;
            dict.set_item("video_id", link.video_id)?;
            dict.set_item("title", link.title)?;
            dict.set_item("offset", link.offset)?;
            dict.set_item("pattern_name", link.pattern_name)?;
            dict.set_item("confidence", link.confidence)?;
            py_results.push(dict.to_object(py));
        }
        Ok(py_results)
    }
}

#[pyclass]
struct RustParallelScanner {
    scanner: ParallelScanner,
}

#[pymethods]
impl RustParallelScanner {
    #[new]
    #[pyo3(signature = (num_threads=0, chunk_size_mb=256, overlap_kb=64, deduplicate=true, min_confidence=0.1))]
    fn new(
        num_threads: usize, 
        chunk_size_mb: usize, 
        overlap_kb: usize, 
        deduplicate: bool, 
        min_confidence: f32
    ) -> Self {
        let config = ScanConfig {
            num_threads,
            chunk_size: chunk_size_mb * 1024 * 1024,
            overlap_size: overlap_kb * 1024,
            deduplicate,
            min_confidence,
        };
        RustParallelScanner {
            scanner: ParallelScanner::new(config),
        }
    }

    fn scan_streaming(
        &self, 
        py: Python, 
        path: String, 
        start_offset: usize, 
        reverse: bool,
        progress_cb: Option<PyObject>,
        hot_fragment_cb: Option<PyObject>
    ) -> PyResult<PyObject> {
        let path_buf = PathBuf::from(path);
        let progress = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel::<HotFragment>();
        let p_clone = progress.clone();
        
        let mut scan_result = None;
        let mut error = None;
        
        thread::scope(|s| {
            let handle = s.spawn(|| {
                let p_cb = |len: usize| {
                    p_clone.fetch_add(len, Ordering::Release);
                };
                let h_cb = |frag: HotFragment| {
                    let _ = tx.send(frag);
                };
                self.scanner.scan_file_streaming(
                    &path_buf,
                    start_offset,
                    reverse,
                    Some(&p_cb),
                    Some(&h_cb)
                )
            });

            let mut last_reported = 0;
            // Loop until thread is finished OR channel is not empty
            while !handle.is_finished() || rx.try_recv().is_ok() {
                // Process ALL available fragments to avoid race condition
                while let Ok(frag) = rx.try_recv() {
                    if let Some(ref cb) = hot_fragment_cb {
                       let dict = pyo3::types::PyDict::new(py);
                       let _ = dict.set_item("offset", frag.offset);
                       let _ = dict.set_item("size", frag.size);
                       let _ = dict.set_item("youtube_count", frag.youtube_count);
                       let _ = dict.set_item("confidence", frag.target_score / 10.0);
                       let _ = dict.set_item("score", frag.target_score);
                       let _ = dict.set_item("file_type", frag.file_type_guess);
                       if let Err(e) = cb.call1(py, (dict,)) {
                           eprintln!("Error in hot fragment callback: {}", e);
                       }
                    }
                }
                
                let current = progress.load(Ordering::Acquire);
                if current > last_reported + (5 * 1024 * 1024) {
                    if let Some(ref cb) = progress_cb {
                        if let Err(e) = cb.call1(py, (current,)) {
                             eprintln!("Error in progress callback: {}", e);
                        }
                    }
                    last_reported = current;
                }
                
                // Debug logging every 1GB or 5 seconds to track liveness
                let current_mb = current / 1024 / 1024;
                if current_mb % 1024 == 0 && current_mb > 0 {
                     eprintln!("[RUST DEBUG] Scanned {} MB", current_mb);
                }

                if !handle.is_finished() {
                     // Check if we are stuck?
                     // eprintln!("[RUST DEBUG] Waiting for thread...");
                     py.allow_threads(|| {
                        thread::sleep(Duration::from_millis(20));
                    });
                }
            }
            
            match handle.join() {
                Ok(res) => {
                    match res {
                        Ok(r) => scan_result = Some(r),
                        Err(e) => error = Some(e.to_string()),
                    }
                }
                Err(_) => error = Some("Scan thread panicked".to_string()),
            }
        });
        
        if let Some(err) = error {
            return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err));
        }
        
        let result = scan_result.unwrap();
        let dict = pyo3::types::PyDict::new(py);
        let links_list = pyo3::types::PyList::new(py, result.links.iter().map(|link| {
            let d = pyo3::types::PyDict::new(py);
            let _ = d.set_item("url", &link.url);
            let _ = d.set_item("video_id", &link.video_id);
            let _ = d.set_item("title", &link.title);
            let _ = d.set_item("offset", link.offset);
            let _ = d.set_item("pattern_name", &link.pattern_name);
            let _ = d.set_item("confidence", link.confidence);
            d
        }));
        
        dict.set_item("links", links_list)?;
        dict.set_item("bytes_scanned", result.bytes_scanned)?;
        dict.set_item("duration_secs", result.duration_secs)?;
            
        Ok(dict.to_object(py))
    }
}

pub mod clusterer;

#[pymodule]
fn rust_accelerator(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<RustPatternMatcher>()?;
    m.add_class::<RustParallelScanner>()?;
    m.add_class::<exfat::RustExFATScanner>()?;
    m.add_class::<exfat::ExFATEntry>()?;
    m.add_class::<fragment_linker::RustFragmentLinker>()?;
    m.add_class::<clusterer::FragmentClusterer>()?;
    Ok(())
}
