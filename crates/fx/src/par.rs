//! Row-parallel loops over flat RGBA buffers on scoped threads (no thread pool, no `unsafe`).

/// Buffers with fewer elements than this run on the calling thread (thread start-up would cost more).
const MIN_PARALLEL_ELEMENTS: usize = 1 << 18;
/// Most threads one effect uses.
const MAX_THREADS: usize = 16;

fn threads_for(elements: usize) -> usize {
    if elements < MIN_PARALLEL_ELEMENTS {
        return 1;
    }
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, MAX_THREADS)
}

/// Run `f(y, row)` over every row of a buffer whose rows are `stride` elements, split over
/// threads. A thread that cannot be started leaves its rows to the calling thread.
pub(crate) fn rows<T: Send, F: Fn(usize, &mut [T]) + Sync>(data: &mut [T], stride: usize, f: F) {
    if stride == 0 {
        return;
    }
    let h = data.len() / stride;
    let threads = threads_for(data.len());
    if threads <= 1 || h < 2 {
        for (y, row) in data.chunks_exact_mut(stride).enumerate() {
            f(y, row);
        }
        return;
    }
    let per = h.div_ceil(threads).max(1);
    let mut failed = Vec::new();
    std::thread::scope(|s| {
        for (ci, chunk) in data.chunks_mut(per * stride).enumerate() {
            let f = &f;
            let job = move || {
                for (i, row) in chunk.chunks_exact_mut(stride).enumerate() {
                    f(ci * per + i, row);
                }
            };
            if std::thread::Builder::new().spawn_scoped(s, job).is_err() {
                failed.push(ci);
            }
        }
    });
    for ci in failed {
        let start = ci * per * stride;
        let end = (start + per * stride).min(data.len());
        if let Some(chunk) = data.get_mut(start..end) {
            for (i, row) in chunk.chunks_exact_mut(stride).enumerate() {
                f(ci * per + i, row);
            }
        }
    }
}
