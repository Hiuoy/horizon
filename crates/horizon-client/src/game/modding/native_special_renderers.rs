//! Special item models drawn by mods' renderers on the client:
//! `native_register_special_model_renderer` (NeoForge's `RegisterSpecialModelRendererEvent`) is
//! asked once at start, before resources load, and its types are what a `minecraft:special` model
//! may name besides vanilla's, or in place of one. When an item of such a model is resolved for
//! drawing, on the main thread, the type's renderer answers the resource models to draw
//! (`ItemEnv::special_model_parts`), checked here before the resolver bakes them. The answer is
//! kept by the stack's equality for each model node, so an unchanged stack is asked once across
//! frames and every display context draws the same answer.
use super::native_entity_renderers::Resolved;
use horizon_assets::{
    item_model::{SpecialPart, StackView},
    math::Mat4,
};
use horizon_client::fast_hash::FxHashMap;
use horizon_core::nbt::Tag;
use mod_api::{ClientHost, MAX_SPECIAL_MODEL_PARTS, SpecialModelPart, SpecialModelRenderer};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, OnceLock, PoisonError, atomic::Ordering},
};

/// The registrations, by special model type: kept once at start.
static REGISTERED: OnceLock<BTreeMap<String, Resolved<dyn SpecialModelRenderer>>> =
    OnceLock::new();

/// The answers kept for each model node (by its `config`'s address, stable until resources
/// reload), newest last: the stacks asked about and what they draw.
static ANSWERS: Mutex<Option<FxHashMap<usize, VecDeque<(StackView, Arc<[SpecialPart]>)>>>> =
    Mutex::new(None);

/// The most stacks a model node keeps answers for: the oldest goes first.
const KEPT_PER_NODE: usize = 64;

/// Keeps the mods' registrations, once per process (false when they already were).
pub(super) fn register(renderers: BTreeMap<String, Arc<dyn SpecialModelRenderer>>) -> bool {
    REGISTERED
        .set(
            renderers
                .into_iter()
                .map(|(kind, renderer)| (kind, Resolved::new(renderer)))
                .collect(),
        )
        .is_ok()
}

/// On every resource reload: the renderers that failed are asked again, and no answer is kept
/// (the model nodes they belonged to are gone).
pub(in crate::game) fn reload() {
    for resolved in REGISTERED.get().into_iter().flat_map(BTreeMap::values) {
        resolved.failed.store(false, Ordering::Relaxed);
    }
    *ANSWERS.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

/// The models the renderer of `kind` draws for `stack` in the model node of `config`: the kept
/// answer when this node was asked about an equal stack, else the renderer's, checked and kept.
/// `None` when no mod registered it, when it failed since resources last loaded, or when it fails
/// now.
pub(in crate::game) fn parts(
    host: &dyn ClientHost,
    kind: &str,
    config: &Arc<Tag>,
    stack: &StackView,
) -> Option<Arc<[SpecialPart]>> {
    let resolved = REGISTERED.get()?.get(kind)?;
    if resolved.failed.load(Ordering::Relaxed) {
        return None;
    }
    answer(kind, resolved, Arc::as_ptr(config) as usize, stack, || {
        use super::api_values::TryIntoApi;
        let stack = stack
            .try_into_api()
            .map_err(|e| format!("the stack can't be read: {e:?}"))?;
        resolved.inner.extract(host, &stack, config)
    })
}

/// The answer for `stack` in the model node `node`: the one kept for an equal stack, else what
/// `extract` answers, checked and kept.
fn answer(
    kind: &str,
    resolved: &Resolved<dyn SpecialModelRenderer>,
    node: usize,
    stack: &StackView,
    extract: impl FnOnce() -> Result<Vec<SpecialModelPart>, String>,
) -> Option<Arc<[SpecialPart]>> {
    if let Some(kept) = kept(node, stack) {
        return Some(kept);
    }
    let parts: Arc<[SpecialPart]> = guarded(kind, resolved, extract)?.into();
    let mut answers = ANSWERS.lock().unwrap_or_else(PoisonError::into_inner);
    let kept = answers
        .get_or_insert_with(FxHashMap::default)
        .entry(node)
        .or_default();
    if kept.len() == KEPT_PER_NODE {
        kept.pop_front();
    }
    kept.push_back((stack.clone(), parts.clone()));
    Some(parts)
}

/// The answer kept for an equal stack in the model node `node`.
fn kept(node: usize, stack: &StackView) -> Option<Arc<[SpecialPart]>> {
    let answers = ANSWERS.lock().unwrap_or_else(PoisonError::into_inner);
    let kept = answers.as_ref()?.get(&node)?;
    kept.iter()
        .rev()
        .find(|(asked, _)| asked == stack)
        .map(|(_, parts)| parts.clone())
}

/// Runs `extract` for the renderer `resolved` of `kind` and checks its answer: `None`, with
/// `failed` set and logged once, when it returns an `Err`, panics or answers parts the resolver
/// refuses.
fn guarded(
    kind: &str,
    resolved: &Resolved<dyn SpecialModelRenderer>,
    extract: impl FnOnce() -> Result<Vec<SpecialModelPart>, String>,
) -> Option<Vec<SpecialPart>> {
    if resolved.failed.load(Ordering::Relaxed) {
        return None;
    }
    let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(extract))
        .unwrap_or_else(|_| Err("it panicked".into()))
        .and_then(|parts| checked(&parts));
    match answer {
        Ok(parts) => Some(parts),
        Err(error) => {
            if !resolved.failed.swap(true, Ordering::Relaxed) {
                log::warn!(
                    "Horizon: the special model renderer of {kind} failed ({error}); its models draw the missing model until resources reload"
                );
            }
            None
        }
    }
}

/// `parts` as the resolver takes them, or why they are refused: more than
/// [`MAX_SPECIAL_MODEL_PARTS`], or a transform that isn't finite. Nothing is kept of a refusal.
fn checked(parts: &[SpecialModelPart]) -> Result<Vec<SpecialPart>, String> {
    if parts.len() > MAX_SPECIAL_MODEL_PARTS {
        return Err(format!(
            "{} parts, more than {MAX_SPECIAL_MODEL_PARTS}",
            parts.len()
        ));
    }
    parts
        .iter()
        .map(|part| {
            let [a, b, c, d, e, f, g, h, i, j, k, l] = part.transform;
            let pose = Mat4::from_row_major([
                a, b, c, d, e, f, g, h, i, j, k, l, 0.0, 0.0, 0.0, 1.0,
            ]);
            if !pose.is_finite() {
                return Err(format!("{}'s transform isn't finite", part.model));
            }
            Ok(SpecialPart {
                model: part.model.clone(),
                pose,
                tints: part.tints.iter().map(|&argb| argb as i32).collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Unused;
    impl SpecialModelRenderer for Unused {
        fn extract(
            &self,
            _: &dyn ClientHost,
            _: &mod_api::ItemStack,
            _: &Tag,
        ) -> Result<Vec<SpecialModelPart>, String> {
            unreachable!("the tests answer through `guarded`'s closure")
        }
    }

    fn renderer() -> Resolved<dyn SpecialModelRenderer> {
        Resolved::new(Arc::new(Unused))
    }

    /// A renderer's parts reach the resolver posed by their row-major transform and tinted.
    #[test]
    fn parts_reach_the_resolver_posed_and_tinted() {
        let parts = guarded("t:pan", &renderer(), || {
            Ok(vec![
                SpecialModelPart::new("t:item/a")
                    .translated([1.0, 2.0, 3.0])
                    .with_tints(vec![0xFF11_2233]),
            ])
        })
        .unwrap();
        assert_eq!(parts[0].model, "t:item/a");
        assert_eq!(parts[0].pose.0, Mat4::translation([1.0, 2.0, 3.0]).0);
        assert_eq!(parts[0].tints, [0xFF11_2233_u32 as i32]);
    }

    /// An unchanged stack is asked once across frames in a model node; another stack, or the same
    /// stack in another node, is asked anew, and a reload asks again.
    #[test]
    fn an_unchanged_stack_is_extracted_once_across_frames() {
        let resolved = renderer();
        let asked = std::cell::Cell::new(0);
        let draw = |node: usize, count: i32| {
            answer("t:pan", &resolved, node, &StackView::new("t:pan", count), || {
                asked.set(asked.get() + 1);
                Ok(vec![SpecialModelPart::new(format!("t:item/{count}"))])
            })
            .unwrap()
        };
        // Nodes far from any real `config` address.
        let (pan, pot) = (1, 2);
        let first = draw(pan, 1);
        for _frame in 0..10 {
            assert!(Arc::ptr_eq(&draw(pan, 1), &first));
        }
        assert_eq!(asked.get(), 1);
        assert_eq!(draw(pan, 2)[0].model, "t:item/2");
        assert_eq!(draw(pot, 1)[0].model, "t:item/1");
        assert_eq!(asked.get(), 3);
        assert!(Arc::ptr_eq(&draw(pan, 1), &first));
        assert_eq!(asked.get(), 3);
        reload();
        draw(pan, 1);
        assert_eq!(asked.get(), 4);
    }

    /// An `Err`, a panic, 257 parts and a transform that isn't finite each refuse the answer
    /// whole, and the renderer isn't asked again until resources reload.
    #[test]
    fn a_failing_renderer_is_refused_whole_and_asked_no_more_until_reload() {
        let nan = SpecialModelPart::new("t:item/a").translated([f32::NAN, 0.0, 0.0]);
        let failures: [Box<dyn Fn() -> Result<Vec<SpecialModelPart>, String>>; 4] = [
            Box::new(|| Err("no pan".into())),
            Box::new(|| panic!("the pan fell")),
            Box::new(|| {
                Ok(vec![
                    SpecialModelPart::new("t:item/a");
                    MAX_SPECIAL_MODEL_PARTS + 1
                ])
            }),
            Box::new(move || Ok(vec![SpecialModelPart::new("t:item/b"), nan.clone()])),
        ];
        for fail in failures {
            let resolved = renderer();
            assert_eq!(guarded("t:pan", &resolved, fail), None);
            let mut asked = false;
            assert_eq!(
                guarded("t:pan", &resolved, || {
                    asked = true;
                    Ok(Vec::new())
                }),
                None
            );
            assert!(!asked, "a failed renderer is asked again before reload");
            resolved.failed.store(false, Ordering::Relaxed);
            assert_eq!(
                guarded("t:pan", &resolved, || Ok(Vec::new())),
                Some(Vec::new())
            );
        }
        let full = vec![SpecialModelPart::new("t:item/a"); MAX_SPECIAL_MODEL_PARTS];
        assert_eq!(
            guarded("t:pan", &renderer(), || Ok(full)).map(|parts| parts.len()),
            Some(MAX_SPECIAL_MODEL_PARTS)
        );
    }
}
