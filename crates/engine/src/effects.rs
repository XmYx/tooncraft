//! Layer effect stacks as commands. Effects live on the layer, so each change is one undo step
//! that swaps the layer stack (no pixels are copied).

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

fn record(e: &mut Engine, label: &str, before: Vec<tooncraft_doc::Layer>) {
    e.push(Entry { label: label.into(), parts: vec![Part::Layers(before)] });
}

pub(crate) fn add(e: &mut Engine, layer: usize, effect: tooncraft_fx::Effect, index: Option<usize>) -> Result<()> {
    let before = e.project.layers.clone();
    let l = e.project.layer_mut(layer)?;
    if l.effects.len() >= tooncraft_fx::MAX_EFFECTS {
        return Err(EngineError::TooManyEffects);
    }
    let at = index.unwrap_or(l.effects.len()).min(l.effects.len());
    l.effects.insert(at, effect.sanitized());
    record(e, "Add effect", before);
    Ok(())
}

/// Change the effect at `index` with `f`; `f` returning false refuses the change.
pub(crate) fn edit(e: &mut Engine, layer: usize, index: usize, label: &str, f: impl FnOnce(&mut tooncraft_fx::Effect) -> bool) -> Result<()> {
    let before = e.project.layers.clone();
    let fx = e.project.layer_mut(layer)?.effects.get_mut(index).ok_or(EngineError::NoEffect(index))?;
    let old = fx.clone();
    if !f(fx) {
        *fx = old;
        return Err(EngineError::EffectMismatch);
    }
    if *fx != old {
        record(e, label, before);
    }
    Ok(())
}

pub(crate) fn remove(e: &mut Engine, layer: usize, index: usize) -> Result<()> {
    let before = e.project.layers.clone();
    let l = e.project.layer_mut(layer)?;
    if index >= l.effects.len() {
        return Err(EngineError::NoEffect(index));
    }
    l.effects.remove(index);
    record(e, "Remove effect", before);
    Ok(())
}

pub(crate) fn move_to(e: &mut Engine, layer: usize, from: usize, to: usize) -> Result<()> {
    let before = e.project.layers.clone();
    let l = e.project.layer_mut(layer)?;
    if from >= l.effects.len() {
        return Err(EngineError::NoEffect(from));
    }
    let fx = l.effects.remove(from);
    let to = to.min(l.effects.len());
    l.effects.insert(to, fx);
    if from != to {
        record(e, "Move effect", before);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use tooncraft_doc::{Project, RenderOptions, render_frame};
    use tooncraft_fx::{Effect, EffectKind};

    use crate::{Command, Engine, EngineError};

    fn engine() -> Engine {
        let mut e = Engine::new(Project::new("fx", 64, 48, 24, 10).unwrap());
        let brush = tooncraft_brush::Brush::default();
        let samples =
            (0..20).map(|i| tooncraft_brush::Sample { x: 10.0 + i as f32, y: 20.0, pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0, time: i as f64 * 0.01 }).collect();
        e.apply(Command::Stroke { layer: 0, frame: 0, brush, samples }).unwrap();
        e.apply(Command::ExtendHold { layer: 0, frame: 0 }).unwrap();
        e
    }

    #[test]
    fn stack_edits_are_undoable_and_reach_the_render() {
        let mut e = engine();
        let plain = render_frame(&e.project, 0, RenderOptions::default());
        let start = e.project.clone();
        e.apply(Command::AddEffect { layer: 0, effect: Effect::new(EffectKind::Blur { radius: 3.0 }), index: None }).unwrap();
        e.apply(Command::AddEffect { layer: 0, effect: Effect::new(EffectKind::Outline { width: 2.0, colour: [255, 0, 0] }), index: Some(0) }).unwrap();
        assert_eq!(e.project.layers[0].effects[0].kind.id(), "outline");
        assert_ne!(render_frame(&e.project, 0, RenderOptions::default()), plain);
        e.apply(Command::MoveEffect { layer: 0, from: 0, to: 5 }).unwrap();
        assert_eq!(e.project.layers[0].effects[1].kind.id(), "outline");
        e.apply(Command::EnableEffect { layer: 0, index: 0, enabled: false }).unwrap();
        e.apply(Command::EditEffect { layer: 0, index: 0, frame: 0, value: EffectKind::Blur { radius: 9.0 } }).unwrap();
        assert_eq!(e.project.layers[0].effects[0].kind, EffectKind::Blur { radius: 9.0 });
        assert_eq!(e.apply(Command::EditEffect { layer: 0, index: 0, frame: 0, value: EffectKind::Thicken { amount: 1.0 } }), Err(EngineError::EffectMismatch));
        assert_eq!(e.apply(Command::RemoveEffect { layer: 0, index: 7 }), Err(EngineError::NoEffect(7)));
        e.apply(Command::RemoveEffect { layer: 0, index: 1 }).unwrap();
        while e.can_undo() && e.project.layers[0].effects != start.layers[0].effects {
            e.undo().unwrap();
        }
        assert_eq!(e.project, start, "every effect edit undoes");
    }

    #[test]
    fn keys_animate_parameters() {
        let mut e = engine();
        e.apply(Command::AddEffect { layer: 0, effect: Effect::new(EffectKind::Blur { radius: 0.0 }), index: None }).unwrap();
        e.apply(Command::SetEffectKey { layer: 0, index: 0, frame: 0, value: None }).unwrap();
        e.apply(Command::SetEffectKey { layer: 0, index: 0, frame: 1, value: Some(EffectKind::Blur { radius: 6.0 }) }).unwrap();
        let f0 = render_frame(&e.project, 0, RenderOptions::default());
        let f1 = render_frame(&e.project, 1, RenderOptions::default());
        assert_ne!(f0, f1, "the same drawing renders differently on the two frames");
        // with keys, an edit sets a key at that frame
        e.apply(Command::EditEffect { layer: 0, index: 0, frame: 5, value: EffectKind::Blur { radius: 2.0 } }).unwrap();
        assert_eq!(e.project.layers[0].effects[0].keys.len(), 3);
        e.apply(Command::RemoveEffectKey { layer: 0, index: 0, frame: 5 }).unwrap();
        assert_eq!(e.project.layers[0].effects[0].keys.len(), 2);
        assert!(e.apply(Command::RemoveEffectKey { layer: 0, index: 0, frame: 5 }).is_err(), "no key there any more");
    }

    #[test]
    fn json_commands() {
        let mut e = engine();
        let cmd: Command = serde_json::from_str(
            r#"{"cmd":"add_effect","layer":0,"effect":{"kind":{"type":"drop_shadow","dx":4,"dy":4,"blur":2,"colour":[0,0,0],"opacity":0.6}}}"#,
        )
        .unwrap();
        e.apply(cmd).unwrap();
        assert!(e.project.layers[0].effects[0].enabled);
        let cmd: Command = serde_json::from_str(r#"{"cmd":"set_effect_key","layer":0,"index":0,"frame":3}"#).unwrap();
        e.apply(cmd).unwrap();
        assert_eq!(e.project.layers[0].effects[0].keys.len(), 1);
    }

    #[test]
    fn opacity_keys_and_mattes_as_commands() {
        let mut e = engine();
        let start = e.project.clone();
        e.apply(Command::SetOpacityKey { layer: 0, frame: 0, opacity: 0.0 }).unwrap();
        e.apply(Command::SetOpacityKey { layer: 0, frame: 1, opacity: 2.0 }).unwrap();
        assert_eq!(e.project.layers[0].opacity_at(1), 1.0, "clamped");
        assert!(e.apply(Command::SetOpacityKey { layer: 0, frame: 99, opacity: 1.0 }).is_err(), "past the end");
        e.apply(Command::RemoveOpacityKey { layer: 0, frame: 1 }).unwrap();
        assert!(e.apply(Command::RemoveOpacityKey { layer: 0, frame: 1 }).is_err());
        e.apply(Command::AddLayer { name: "Matte".into(), index: None }).unwrap();
        let cmd: Command = serde_json::from_str(r#"{"cmd":"set_layer_matte","layer":1,"matte":"inside"}"#).unwrap();
        e.apply(cmd).unwrap();
        assert_eq!(e.project.layers[1].matte, Some(tooncraft_doc::Matte::Inside));
        let blank = render_frame(&e.project, 0, RenderOptions { paper: false, ..Default::default() });
        assert!(blank.data.iter().all(|v| *v == 0), "an empty matte hides what it clips");
        for _ in 0..5 {
            e.undo().unwrap();
        }
        assert_eq!(e.project, start);
    }
}
