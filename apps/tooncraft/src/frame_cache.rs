//! Composited frames for the canvas when a layer needs the CPU compositor (blend modes, effects,
//! mattes). Frames are cached per project version and frame, bounded by a memory budget, and a
//! worker thread renders the frames after the playhead ahead of time, so playback with effects
//! shows finished frames instead of computing them in the UI thread. A frame that is not ready
//! (scrubbing, a cold start) is rendered on the spot.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};

/// Frames someone is rendering or has rendered, per project version: the UI thread and the
/// worker never render the same frame twice.
type Claims = Arc<Mutex<HashSet<(u64, u32)>>>;

fn claim(c: &Claims, generation: u64, frame: u32) -> bool {
    let mut set = c.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if set.len() > 100_000 {
        set.clear();
    }
    set.insert((generation, frame))
}

use tooncraft_doc::{Project, RenderOptions};
use tooncraft_raster::{RgbaImage, Surface};

/// Most bytes of cached frames (sparse tiles).
pub const DEFAULT_BUDGET: u64 = 1536 << 20;
/// Frames rendered ahead of the playhead.
pub const AHEAD: u32 = 48;

struct Job {
    generation: u64,
    project: Project,
    frames: Vec<u32>,
}

struct Worker {
    jobs: Sender<Job>,
    done: Receiver<(u64, u32, RgbaImage)>,
}

fn spawn(claims: Claims) -> Option<Worker> {
    let (jobs, rx) = channel::<Job>();
    let (tx, done) = channel();
    let run = move || {
        // a job that arrived while the last one was being rendered
        let mut pending: Option<Job> = None;
        loop {
            let mut job = match pending.take() {
                Some(j) => j,
                None => match rx.recv() {
                    Ok(j) => j,
                    Err(_) => return,
                },
            };
            // only the newest job matters
            while let Ok(newer) = rx.try_recv() {
                job = newer;
            }
            for f in &job.frames {
                match rx.try_recv() {
                    Ok(newer) => {
                        pending = Some(newer);
                        break;
                    }
                    Err(TryRecvError::Disconnected) => return,
                    Err(TryRecvError::Empty) => {}
                }
                if !claim(&claims, job.generation, *f) {
                    continue;
                }
                let img = tooncraft_doc::render_frame(&job.project, *f, RenderOptions { paper: false, ..Default::default() });
                if tx.send((job.generation, *f, img)).is_err() {
                    return;
                }
            }
        }
    };
    std::thread::Builder::new().name("tooncraft-frames".into()).spawn(run).ok().map(|_| Worker { jobs, done })
}

pub struct FrameCache {
    generation: u64,
    frames: HashMap<u32, (Surface, u64)>,
    bytes: u64,
    pub budget: u64,
    clock: u64,
    worker: Option<Worker>,
    worker_tried: bool,
    claims: Claims,
    /// The frames the worker was last asked for (generation, first frame).
    asked: Option<(u64, u32)>,
    /// Frames rendered in the UI thread / by the worker (statistics).
    pub rendered_here: u64,
    pub rendered_ahead: u64,
    /// Milliseconds spent in the UI thread compositing frames and turning images into tiles.
    pub render_ms: f64,
    pub tiles_ms: f64,
}

impl Default for FrameCache {
    fn default() -> Self {
        Self {
            generation: u64::MAX,
            frames: HashMap::new(),
            bytes: 0,
            budget: DEFAULT_BUDGET,
            clock: 0,
            worker: None,
            worker_tried: false,
            claims: Arc::new(Mutex::new(HashSet::new())),
            asked: None,
            rendered_here: 0,
            rendered_ahead: 0,
            render_ms: 0.0,
            tiles_ms: 0.0,
        }
    }
}

fn to_surface(img: &RgbaImage) -> Option<Surface> {
    let mut s = Surface::new(img.width, img.height).ok()?;
    s.load_image(img).ok()?;
    Some(s)
}

impl FrameCache {
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    fn reset(&mut self, generation: u64) {
        if generation != self.generation {
            self.generation = generation;
            self.frames.clear();
            self.bytes = 0;
            self.asked = None;
            self.claims.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
        }
    }

    fn insert(&mut self, frame: u32, s: Surface) {
        self.clock += 1;
        self.bytes += s.memory_bytes();
        if let Some((old, _)) = self.frames.insert(frame, (s, self.clock)) {
            self.bytes = self.bytes.saturating_sub(old.memory_bytes());
        }
        while self.bytes > self.budget && self.frames.len() > 1 {
            let Some(oldest) = self.frames.iter().filter(|(f, _)| **f != frame).min_by_key(|(_, (_, used))| *used).map(|(f, _)| *f) else { break };
            if let Some((s, _)) = self.frames.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(s.memory_bytes());
            }
            self.claims.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&(self.generation, oldest));
        }
    }

    /// Take the frames the worker finished.
    fn collect(&mut self) {
        let mut got = Vec::new();
        if let Some(w) = &self.worker {
            while let Ok(r) = w.done.try_recv() {
                got.push(r);
            }
        }
        for (g, f, img) in got {
            let t0 = std::time::Instant::now();
            let s = (g == self.generation && !self.frames.contains_key(&f)).then(|| to_surface(&img)).flatten();
            self.tiles_ms += t0.elapsed().as_secs_f64() * 1000.0;
            if let Some(s) = s {
                self.rendered_ahead += 1;
                self.insert(f, s);
            }
        }
    }

    /// Ask the worker for the frames after `frame` that are not cached yet (wrapping at the end
    /// of the shot, as playback loops).
    pub fn prefetch(&mut self, p: &Project, generation: u64, frame: u32) {
        self.reset(generation);
        if !self.worker_tried {
            self.worker_tried = true;
            self.worker = spawn(self.claims.clone());
        }
        let Some(w) = &self.worker else { return };
        // ask again only when the playhead moved well past the last request
        if let Some((g, from)) = self.asked
            && g == generation
            && frame >= from
            && frame < from + AHEAD / 2
        {
            return;
        }
        let len = p.length().max(1);
        let budget_frames = (self.budget / (p.width() as u64 * p.height() as u64 * 4).max(1)).max(1) as u32;
        let n = AHEAD.min(len).min(budget_frames);
        let frames: Vec<u32> = (1..=n).map(|k| (frame + k) % len).filter(|f| !self.frames.contains_key(f)).collect();
        if !frames.is_empty() && w.jobs.send(Job { generation, project: p.clone(), frames }).is_err() {
            self.worker = None;
        }
        self.asked = Some((generation, frame));
    }

    /// The composited frame (no paper), from the cache or rendered now.
    pub fn get(&mut self, p: &Project, generation: u64, frame: u32) -> Option<&Surface> {
        self.reset(generation);
        self.collect();
        if !self.frames.contains_key(&frame) {
            // tell the worker this one is done here
            claim(&self.claims, generation, frame);
            let t0 = std::time::Instant::now();
            let img = tooncraft_doc::render_frame(p, frame, RenderOptions { paper: false, ..Default::default() });
            let t1 = std::time::Instant::now();
            let s = to_surface(&img)?;
            self.render_ms += (t1 - t0).as_secs_f64() * 1000.0;
            self.tiles_ms += t1.elapsed().as_secs_f64() * 1000.0;
            self.rendered_here += 1;
            self.insert(frame, s);
        }
        self.clock += 1;
        let clock = self.clock;
        self.frames.get_mut(&frame).map(|(s, used)| {
            *used = clock;
            &*s
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caches_per_version_and_renders_ahead() {
        let mut e = tooncraft_engine::demo::demo(320, 180).unwrap();
        let p = e.project.clone();
        let mut c = FrameCache::default();
        assert!(c.get(&p, 1, 0).is_some());
        assert_eq!((c.len(), c.rendered_here), (1, 1));
        assert!(c.get(&p, 1, 0).is_some());
        assert_eq!(c.rendered_here, 1, "the second look is cached");
        c.prefetch(&p, 1, 0);
        let start = std::time::Instant::now();
        while c.rendered_ahead < 3 && start.elapsed().as_secs() < 20 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            c.collect();
        }
        assert!(c.rendered_ahead >= 3, "the worker rendered frames after the playhead");
        let before = c.rendered_here;
        assert!(c.get(&p, 1, 1).is_some());
        assert_eq!(c.rendered_here, before, "frame 2 came from the worker");
        // an edit is a new version: nothing old is shown
        let _ = e.apply(tooncraft_engine::Command::SetPaper { color: [1, 2, 3] });
        assert!(c.get(&e.project, 2, 1).is_some());
        assert_eq!(c.len(), 1);
        // the budget bounds memory
        c.budget = 1;
        for f in 0..4 {
            let _ = c.get(&p, 3, f);
        }
        assert_eq!(c.len(), 1);
    }
}
