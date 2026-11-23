//! Edit decisions out as OpenTimelineIO JSON (`.otio`), from OTIO's published file schema: the
//! shot is a timeline whose stack holds one video track per layer (bottom layer first, as OTIO
//! stacks run), each exposure a clip named after its drawing and each blank stretch a gap, all
//! at the shot's frame rate; frame markers become markers on the stack. The drawings themselves
//! are not referenced as media (they live in the shot), so clips carry a missing reference and
//! the drawing's id in their metadata.

use serde_json::{Value, json};
use tooncraft_doc::{Cell, Project};

fn time(frames: u32, rate: f64) -> Value {
    json!({ "OTIO_SCHEMA": "RationalTime.1", "rate": rate, "value": frames as f64 })
}

fn range(start: u32, duration: u32, rate: f64) -> Value {
    json!({ "OTIO_SCHEMA": "TimeRange.1", "start_time": time(start, rate), "duration": time(duration, rate) })
}

/// The shot as an OpenTimelineIO timeline, pretty-printed JSON.
pub fn to_json(p: &Project) -> String {
    let rate = p.fps().max(1) as f64;
    let len = p.length();
    let mut tracks = Vec::new();
    for layer in &p.layers {
        let mut children = Vec::new();
        let mut f = 0u32;
        while f < len {
            match layer.cells.get(f as usize) {
                Some(Cell::Key(d)) => {
                    let (start, end) = layer.exposure_range(f).unwrap_or((f, f + 1));
                    let name = p.drawing_names.get(d).cloned().unwrap_or_else(|| format!("drawing {}", d.0));
                    children.push(json!({
                        "OTIO_SCHEMA": "Clip.1",
                        "name": name,
                        "source_range": range(0, end - start, rate),
                        "media_reference": { "OTIO_SCHEMA": "MissingReference.1", "name": "", "metadata": {} },
                        "effects": [],
                        "markers": [],
                        "metadata": { "tooncraft": { "drawing": d.0, "frame": start } }
                    }));
                    f = end.max(f + 1);
                }
                _ => {
                    let start = f;
                    while f < len && !matches!(layer.cells.get(f as usize), Some(Cell::Key(_))) {
                        f += 1;
                    }
                    children.push(
                        json!({ "OTIO_SCHEMA": "Gap.1", "name": "", "source_range": range(0, f - start, rate), "effects": [], "markers": [], "metadata": {} }),
                    );
                }
            }
        }
        tracks.push(json!({
            "OTIO_SCHEMA": "Track.1",
            "name": layer.name,
            "kind": "Video",
            "source_range": null,
            "effects": [],
            "markers": [],
            "metadata": { "tooncraft": { "visible": layer.visible, "opacity": layer.opacity } },
            "children": children
        }));
    }
    let markers: Vec<Value> = p
        .markers
        .iter()
        .map(|(frame, text)| json!({ "OTIO_SCHEMA": "Marker.2", "name": text, "marked_range": range(*frame, 0, rate), "color": "RED", "metadata": {} }))
        .collect();
    let timeline = json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": p.name,
        "global_start_time": time(0, rate),
        "metadata": { "tooncraft": { "width": p.width(), "height": p.height() } },
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "name": "tracks",
            "source_range": null,
            "effects": [],
            "markers": markers,
            "metadata": {},
            "children": tracks
        }
    });
    serde_json::to_string_pretty(&timeline).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposures_become_clips_and_blanks_gaps() {
        let mut p = Project::new("Walk", 64, 48, 24, 6).unwrap();
        let a = p.add_drawing(None).unwrap();
        let b = p.add_drawing(None).unwrap();
        p.layers[0].cells = vec![Cell::Key(a), Cell::Hold, Cell::Hold, Cell::Blank, Cell::Key(b), Cell::Hold];
        p.set_marker(4, "pose").unwrap();
        let v: Value = serde_json::from_str(&to_json(&p)).unwrap();
        assert_eq!(v["OTIO_SCHEMA"], "Timeline.1");
        let kids = v["tracks"]["children"][0]["children"].as_array().unwrap().clone();
        let schema: Vec<&str> = kids.iter().map(|k| k["OTIO_SCHEMA"].as_str().unwrap()).collect();
        assert_eq!(schema, ["Clip.1", "Gap.1", "Clip.1"]);
        assert_eq!(kids[0]["source_range"]["duration"]["value"], 3.0);
        assert_eq!(kids[0]["source_range"]["duration"]["rate"], 24.0);
        assert_eq!(kids[2]["metadata"]["tooncraft"]["drawing"], b.0);
        assert_eq!(v["tracks"]["markers"][0]["name"], "pose");
        // the durations add up to the shot
        let total: f64 = kids.iter().map(|k| k["source_range"]["duration"]["value"].as_f64().unwrap()).sum();
        assert_eq!(total, 6.0);
    }
}
