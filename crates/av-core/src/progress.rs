//! Thread-safe cooperative cancellation and stage progress reporting.

use crate::CoreError;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Stable names for pipeline progress and session cache boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Detect,
    Preprocess,
    Quantize,
    ResolveAa,
    Label,
    MergeSmall,
    ExtractBoundaries,
    FitEdges,
    Assemble,
}

/// Caller-owned cancellation flag and nonblocking callback, safe to share with workers.
#[derive(Clone)]
pub struct Control {
    pub cancel: Arc<AtomicBool>,
    pub progress: Arc<dyn Fn(Stage, f32) + Send + Sync>,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(|_, _| {}),
        }
    }
}

impl Control {
    /// Check the shared flag at bounded intervals, including before each stage.
    pub fn check(&self) -> Result<(), CoreError> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(CoreError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Report a finite fraction in [0,1], checking cancellation first.
    pub fn report(&self, stage: Stage, fraction: f32) -> Result<(), CoreError> {
        self.check()?;
        if !fraction.is_finite() {
            return Err(CoreError::Internal("non-finite progress".into()));
        }
        (self.progress)(stage, fraction.clamp(0.0, 1.0));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn shared_cancellation_and_progress() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let sink = output.clone();
        let ctl = Control {
            progress: Arc::new(move |s, f| sink.lock().unwrap().push((s, f))),
            ..Control::default()
        };
        ctl.report(Stage::Quantize, 2.0).unwrap();
        assert_eq!(*output.lock().unwrap(), vec![(Stage::Quantize, 1.0)]);
        assert!(ctl.report(Stage::Label, f32::NAN).is_err());
        let cloned = ctl.clone();
        std::thread::spawn(move || cloned.cancel.store(true, Ordering::Relaxed))
            .join()
            .unwrap();
        assert_eq!(ctl.report(Stage::Label, 0.0), Err(CoreError::Cancelled));
        assert_eq!(output.lock().unwrap().len(), 1);
    }
}
