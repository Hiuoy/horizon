use super::*;
#[test]
fn container_loot_component_recipe_decodes_validates_and_matches() {
    use serde_json::json;
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:stick".into(),
        crate::json_nbt::json_to_compound(
            json!({"minecraft:container_loot":{"loot_table":"fixture:unregistered"}})
                .as_object()
                .unwrap(),
        )
        .unwrap(),
    )]));
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let root = tempfile::tempdir().unwrap();
    let ingredient_json = |payload: serde_json::Value, strict| {
        json!({"type":"neoforge:components","items":"minecraft:stick",
            "components":{"minecraft:container_loot":payload},"strict":strict})
    };
    let recipe_json = |ingredient: serde_json::Value| {
        json!({"type":"minecraft:crafting_shapeless","ingredients":[ingredient],
            "result":{"id":"minecraft:diamond"}})
    };
    let recipes = |ingredient: serde_json::Value| {
        crate::crafting::Recipes::from_sources(
            root.path(),
            &items,
            &horizon_world::loot::PackOverlay::NONE,
            vec![
                ("fixture:loot".into(), recipe_json(ingredient)),
                (
                    "fixture:control".into(),
                    recipe_json(json!("minecraft:stone")),
                ),
            ],
            &load,
        )
        .unwrap()
    };
    for (payload, seed, table) in [
        (
            json!({"loot_table":"fixture:unregistered"}),
            0_i64,
            "fixture:unregistered",
        ),
        (
            json!({"loot_table":"unregistered","seed":0,"ignored":[1]}),
            0,
            "minecraft:unregistered",
        ),
        (
            json!({"loot_table":":unregistered","seed":-2.75}),
            -2,
            "minecraft:unregistered",
        ),
        (
            json!({"loot_table":"fixture:unregistered","seed":i64::MAX}),
            i64::MAX,
            "fixture:unregistered",
        ),
        (
            json!({"loot_table":"fixture:unregistered","seed":i64::MIN}),
            i64::MIN,
            "fixture:unregistered",
        ),
        (json!({"loot_table":""}), 0, "minecraft:"),
    ] {
        for strict in [false, true] {
            let source = ingredient_json(payload.clone(), strict);
            let ingredient = Ingredient::parse(&source, &items).unwrap();
            let loaded = recipes(source);
            let shown = ingredient.shown.as_ref().unwrap()[0].data().unwrap();
            assert_eq!(
                shown.other("minecraft:container_loot"),
                Some(&Tag::compound([
                    ("loot_table", Tag::String(table.into())),
                    ("seed", Tag::Long(seed)),
                ]))
            );
            let mut stack = ItemStack::new("minecraft:stick", 1);
            stack.data_mut().set_loot_component(
                "minecraft:container_loot",
                &Some(horizon_core::json_nbt::json_to_tag(&payload).unwrap()),
            );
            assert!(loaded.recipe_matches(&context, "fixture:loot", &[Some(stack.clone())], 1));
            let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
                assert!(ingredient.test_in(&context, &stack));
            });
            assert_eq!(allocations.calls, 0, "{allocations:?}");
            stack.data_mut().set_loot_component(
                "minecraft:container_loot",
                &Some(Tag::compound([
                    ("loot_table", Tag::String(table.into())),
                    ("seed", Tag::Long(seed.wrapping_add(1))),
                ])),
            );
            assert!(!loaded.recipe_matches(&context, "fixture:loot", &[Some(stack.clone())], 1));
            stack.data_mut().set_loot_component(
                "minecraft:container_loot",
                &Some(Tag::compound([(
                    "loot_table",
                    Tag::String("fixture:other".into()),
                )])),
            );
            assert!(!ingredient.test_in(&context, &stack));
        }
    }
    let source = ingredient_json(json!({"loot_table":"fixture:unregistered","seed":0}), false);
    let ingredient = Ingredient::parse(&source, &items).unwrap();
    let loaded = recipes(source);
    let mut stack = ItemStack::new("minecraft:stick", 1);
    assert!(loaded.recipe_matches(&context, "fixture:loot", &[Some(stack.clone())], 1));
    stack
        .data_mut()
        .set_loot_component("minecraft:custom_data", &Some(Tag::Int(1)));
    assert!(ingredient.test_in(&context, &stack));
    let strict = recipes(ingredient_json(
        json!({"loot_table":"fixture:unregistered"}),
        true,
    ));
    assert!(!strict.recipe_matches(&context, "fixture:loot", &[Some(stack.clone())], 1));
    stack
        .data_mut()
        .set_loot_component("minecraft:custom_data", &None);
    stack
        .data_mut()
        .set_loot_component("minecraft:container_loot", &None);
    stack.data_mut().set_loot_component(
        "!minecraft:container_loot",
        &Some(Tag::compound([] as [(&str, Tag); 0])),
    );
    assert!(!loaded.recipe_matches(&context, "fixture:loot", &[Some(stack.clone())], 1));
    let removed = recipes(
        json!({"type":"neoforge:components","items":"minecraft:stick",
        "components":{"!minecraft:container_loot":{}}}),
    );
    assert!(removed.recipe_matches(&context, "fixture:loot", &[Some(stack)], 1));
    assert!(!removed.recipe_matches(
        &context,
        "fixture:loot",
        &[Some(ItemStack::new("minecraft:stick", 1))],
        1
    ));

    for payload in [
        json!({}),
        json!("fixture:unregistered"),
        json!([]),
        json!({"loot_table":1}),
        json!({"loot_table":{"type":"minecraft:empty"}}),
        json!({"loot_table":"Bad:table"}),
        json!({"loot_table":"..:table"}),
        json!({"loot_table":"a:b:c"}),
        json!({"loot_table":"bad path"}),
        json!({"loot_table":"fixture:unregistered","seed":"0"}),
        json!({"loot_table":"fixture:unregistered","seed":{}}),
    ] {
        let source = ingredient_json(payload, false);
        assert!(Ingredient::parse(&source, &items).is_none());
        match crate::json_nbt::json_to_compound(source["components"].as_object().unwrap()) {
            Ok(patch) => {
                let rule = IngredientRule::Components {
                    patch,
                    removed: Vec::new(),
                    strict: false,
                };
                let error = components::validate(&rule, &items).unwrap_err();
                assert!(
                    error.contains("component minecraft:container_loot:"),
                    "{error}"
                );
            }
            // Nested null is refused before a component payload reaches NbtOps.
            Err(error) => assert_eq!(error, horizon_core::json_nbt::JsonNbtError::NestedNull),
        }
        let loaded = recipes(source);
        assert!(loaded.recipe("fixture:loot").is_none());
        assert!(!loaded.recipe_matches(
            &context,
            "fixture:loot",
            &[Some(ItemStack::new("minecraft:stick", 1))],
            1
        ));
        assert!(loaded.recipe_matches(
            &context,
            "fixture:control",
            &[Some(ItemStack::new("minecraft:stone", 1))],
            1
        ));
    }
    for (raw, expected) in [
        (Tag::Byte(-1), -1),
        (Tag::Short(2), 2),
        (Tag::Int(3), 3),
        (Tag::Long(i64::MIN), i64::MIN),
        (Tag::Float((-0.0_f32).to_bits()), 0),
        (Tag::Double(f64::NAN.to_bits()), 0),
        (Tag::Double(f64::INFINITY.to_bits()), i64::MAX),
        (Tag::Double(f64::NEG_INFINITY.to_bits()), i64::MIN),
        (Tag::Double((-2.75_f64).to_bits()), -2),
    ] {
        let value = Tag::compound([
            ("loot_table", Tag::String(":unregistered".into())),
            ("seed", raw),
        ]);
        let decoded = horizon_command::item::decoded_component_value(
            "minecraft:container_loot",
            &value,
            &items.component_registry,
        )
        .unwrap();
        assert_eq!(decoded.get("seed"), Some(&Tag::Long(expected)));
    }
    for units in [vec![0xd800], vec![0xdc00], vec![0xd83d, 0xde00]] {
        let value = Tag::compound([(
            "loot_table",
            Tag::String(horizon_core::nbt::JavaString(units)),
        )]);
        assert!(
            horizon_command::item::decoded_component_value(
                "minecraft:container_loot",
                &value,
                &items.component_registry,
            )
            .is_err()
        );
    }
}

#[test]
fn lodestone_tracker_component_recipes_decode_validate_and_match() {
    const ID: &str = "minecraft:lodestone_tracker";
    fn ingredient(patch: Value, strict: bool) -> Value {
        serde_json::json!({
            "type":"neoforge:components", "items":"minecraft:compass",
            "components":patch, "strict":strict
        })
    }
    fn recipe(ingredient: Value) -> Value {
        serde_json::json!({
            "type":"minecraft:smelting", "ingredient":ingredient,
            "result":{"id":"minecraft:diamond"}, "experience":0.0, "cookingtime":20
        })
    }
    fn book(items: &Items, sources: Vec<(&str, Value)>) -> crate::crafting::Recipes {
        let root = tempfile::tempdir().unwrap();
        crate::crafting::Recipes::from_sources(
            root.path(),
            items,
            &horizon_world::loot::PackOverlay::NONE,
            sources
                .into_iter()
                .map(|(id, value)| (id.into(), value))
                .collect(),
            &load(),
        )
        .unwrap()
    }
    fn stack(value: Tag) -> ItemStack {
        let mut stack = ItemStack::new("minecraft:compass", 1);
        stack.data_mut().set_loot_component(ID, &Some(value));
        stack
    }
    fn target(dimension: Tag, pos: Tag) -> Tag {
        Tag::compound([(
            "target",
            Tag::compound([("dimension", dimension), ("pos", pos)]),
        )])
    }
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:compass".into(),
        crate::json_nbt::json_to_compound(
            serde_json::json!({
                "minecraft:lodestone_tracker":{
                    "target":{"dimension":":unregistered/dimension","pos":[1,-2,3]},
                    "tracked":-2
                },
                "minecraft:max_stack_size":64
            })
            .as_object()
            .unwrap(),
        )
        .unwrap(),
    )]));
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let check =
        |recipes: &crate::crafting::Recipes, candidate: &ItemStack, expected: Option<&str>| {
            // Build the cooking index outside the candidate-allocation measurement.
            let _ = recipes.cook(&context, "minecraft:smelting", candidate);
            let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
                let cooked = recipes.cook(&context, "minecraft:smelting", candidate);
                assert_eq!(cooked.map(|recipe| recipe.id.as_str()), expected);
                if let Some(cooked) = cooked {
                    assert_eq!(cooked.result.item.as_ref(), "minecraft:diamond");
                }
            });
            assert_eq!(allocations.calls, 0, "{allocations:?}");
        };
    let literal: Value = serde_json::from_str(
        r#"{"target":{"dimension":"unregistered/dimension","pos":[1.9,-2.9,4294967299],"ignored":false},"ignored":{"future":true}}"#,
    )
    .unwrap();
    let patch = serde_json::json!({ID:literal});
    let recipes = book(
        &items,
        vec![("fixture:target", recipe(ingredient(patch.clone(), false)))],
    );
    let shown = Ingredient::parse(&ingredient(patch.clone(), false), &items).unwrap();
    assert_eq!(
        shown.shown.as_ref().unwrap()[0].data().unwrap().other(ID),
        Some(&target(
            Tag::String("minecraft:unregistered/dimension".into()),
            Tag::IntArray(vec![1, -2, 3]),
        )),
    );
    let plain = ItemStack::new("minecraft:compass", 1);
    check(&recipes, &plain, Some("fixture:target"));
    for dimension in [
        "unregistered/dimension",
        ":unregistered/dimension",
        "minecraft:unregistered/dimension",
    ] {
        for pos in [
            Tag::IntArray(vec![1, -2, 3]),
            Tag::ByteArray(vec![1, 254, 3]),
            Tag::LongArray(vec![4294967297, -2, 3]),
            Tag::List(vec![
                Tag::Short(1),
                Tag::Float((-2.9f32).to_bits()),
                Tag::Long(4294967299),
            ]),
        ] {
            check(
                &recipes,
                &stack(target(Tag::String(dimension.into()), pos)),
                Some("fixture:target"),
            );
        }
    }
    for payload in [
        r#"{"target":{"dimension":"fixture:unregistered/dimension","pos":[1,-2,3]}}"#,
        r#"{"target":{"dimension":"Minecraft:unregistered/dimension","pos":[1,-2,3]}}"#,
        r#"{"target":{"dimension":"unregistered/dimension","pos":[1,-2,4]}}"#,
        r#"{"target":{"dimension":"unregistered/dimension","pos":[1,-2]}}"#,
        r#"{"target":{"dimension":"unregistered/dimension","pos":[1,-2,3]},"tracked":false}"#,
        r#"{"target":{"dimension":"unregistered/dimension","pos":[1,-2,3]},"tracked":"true"}"#,
        r#"{}"#,
    ] {
        let value: Value = serde_json::from_str(payload).unwrap();
        check(
            &recipes,
            &stack(crate::json_nbt::json_to_tag(&value).unwrap()),
            None,
        );
    }
    let mut empty = plain.clone();
    empty.count = 0;
    check(&recipes, &empty, None);
    check(&recipes, &ItemStack::new("minecraft:stone", 1), None);

    // Strictness counts prototype components, but not unknown record fields.
    let strict = book(
        &items,
        vec![("fixture:strict", recipe(ingredient(patch.clone(), true)))],
    );
    check(&strict, &plain, None);
    let mut complete_patch = patch.clone();
    complete_patch["minecraft:max_stack_size"] = serde_json::json!(64);
    let complete = book(
        &items,
        vec![("fixture:complete", recipe(ingredient(complete_patch, true)))],
    );
    check(&complete, &plain, Some("fixture:complete"));
    let mut extra = plain.clone();
    extra.data_mut().set_loot_component(
        "minecraft:custom_data",
        &Some(Tag::compound([("opaque", Tag::Int(7))])),
    );
    check(&complete, &extra, None);
    check(&recipes, &extra, Some("fixture:target"));
    let mut without_max = plain.clone();
    (_, without_max.extra) = crate::item_data::decode_components(Some(&Tag::compound([(
        "!minecraft:max_stack_size",
        Tag::Compound(Default::default()),
    )])));
    check(&strict, &without_max, Some("fixture:strict"));

    let removal = book(
        &items,
        vec![(
            "fixture:removed",
            recipe(ingredient(
                serde_json::json!({"!minecraft:lodestone_tracker":{}}),
                false,
            )),
        )],
    );
    check(&removal, &plain, None);
    let mut removed = plain.clone();
    (_, removed.extra) = crate::item_data::decode_components(Some(&Tag::compound([(
        "!minecraft:lodestone_tracker",
        Tag::Compound(Default::default()),
    )])));
    check(&removal, &removed, Some("fixture:removed"));
    check(&recipes, &removed, None);
    let mut reset = stack(Tag::Compound(Default::default()));
    reset.data_mut().set_loot_component(ID, &None);
    check(&recipes, &reset, Some("fixture:target"));
    check(&removal, &reset, None);

    // Optional defaults are not defaults for malformed, present fields.
    for (literal, tracked) in [
        (r#"{}"#, true),
        (r#"{"tracked":true,"ignored":5}"#, true),
        (r#"{"tracked":-0.25}"#, true),
        (r#"{"tracked":false}"#, false),
        (r#"{"tracked":0}"#, false),
    ] {
        let value: Value = serde_json::from_str(literal).unwrap();
        let recipes = book(
            &items,
            vec![(
                "fixture:targetless",
                recipe(ingredient(serde_json::json!({ID:value}), false)),
            )],
        );
        let matching = stack(Tag::compound([("tracked", Tag::Byte(i8::from(tracked)))]));
        check(&recipes, &matching, Some("fixture:targetless"));
        check(
            &recipes,
            &stack(Tag::compound([("tracked", Tag::Byte(i8::from(!tracked)))])),
            None,
        );
        check(&recipes, &plain, None);
        if !tracked {
            for zero in [
                Tag::Float((-0.0f32).to_bits()),
                Tag::Double((-0.0f64).to_bits()),
            ] {
                check(
                    &recipes,
                    &stack(Tag::compound([("tracked", zero)])),
                    Some("fixture:targetless"),
                );
            }
        } else {
            check(
                &recipes,
                &stack(Tag::Compound(Default::default())),
                Some("fixture:targetless"),
            );
            check(
                &recipes,
                &stack(Tag::compound([(
                    "tracked",
                    Tag::Double(f64::NAN.to_bits()),
                )])),
                Some("fixture:targetless"),
            );
        }
    }
    // ResourceKey.codec accepts pack-defined keys, even empty identifier paths.
    for dimension in ["fixture:pack_only", "", ":", "minecraft:"] {
        let recipes = book(
            &items,
            vec![(
                "fixture:dimension",
                recipe(ingredient(
                    serde_json::json!({ID:{
                        "target":{"dimension":dimension,"pos":[2147483647,-2147483648,0]}
                    }}),
                    false,
                )),
            )],
        );
        let candidate = stack(target(
            Tag::String(dimension.into()),
            Tag::List(vec![
                Tag::Float(f32::INFINITY.to_bits()),
                Tag::Double(f64::NEG_INFINITY.to_bits()),
                Tag::Float(f32::NAN.to_bits()),
            ]),
        ));
        check(&recipes, &candidate, Some("fixture:dimension"));
        check(&recipes, &plain, None);
    }
    for units in [vec![0xd800], vec![0xdc00], vec![0xd83d, 0xde00]] {
        let invalid = target(
            Tag::String(horizon_core::nbt::JavaString(units)),
            Tag::IntArray(vec![1, -2, 3]),
        );
        assert!(
            horizon_command::item::decoded_component_value(ID, &invalid, &items.component_registry)
                .is_err()
        );
        assert!(
            horizon_command::item::decode_component(ID, &invalid, &items.component_registry)
                .is_err()
        );
        check(&recipes, &stack(invalid), None);
    }
    for literal in [
        r#"5"#,
        r#"true"#,
        r#""tracker""#,
        r#"[]"#,
        r#"{"target":5}"#,
        r#"{"target":{}}"#,
        r#"{"target":{"dimension":"overworld"}}"#,
        r#"{"target":{"pos":[1,2,3]}}"#,
        r#"{"target":{"dimension":5,"pos":[1,2,3]}}"#,
        r#"{"target":{"dimension":"Minecraft:overworld","pos":[1,2,3]}}"#,
        r#"{"target":{"dimension":"minecraft:Overworld","pos":[1,2,3]}}"#,
        r#"{"target":{"dimension":"bad namespace:path","pos":[1,2,3]}}"#,
        r#"{"target":{"dimension":"..:path","pos":[1,2,3]}}"#,
        r#"{"target":{"dimension":"a:b:c","pos":[1,2,3]}}"#,
        r#"{"target":{"dimension":"overworld","pos":[]}}"#,
        r#"{"target":{"dimension":"overworld","pos":[1,2]}}"#,
        r#"{"target":{"dimension":"overworld","pos":[1,2,3,4]}}"#,
        r#"{"target":{"dimension":"overworld","pos":[1,"2",3]}}"#,
        r#"{"target":{"dimension":"overworld","pos":5}}"#,
        r#"{"tracked":"true"}"#,
        r#"{"tracked":{}}"#,
        r#"{"tracked":[]}"#,
    ] {
        let value: Value = serde_json::from_str(literal).unwrap();
        let invalid = ingredient(serde_json::json!({ID:value}), false);
        assert!(Ingredient::parse(&invalid, &items).is_none(), "{literal}");
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(invalid["components"].as_object().unwrap())
                .unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        let error = components::validate(&rule, &items).unwrap_err();
        assert!(
            error.contains("component minecraft:lodestone_tracker:"),
            "{error}"
        );
        let loaded = book(
            &items,
            vec![
                ("fixture:refused", recipe(invalid)),
                (
                    "fixture:kept",
                    recipe(ingredient(serde_json::json!({ID:{}}), false)),
                ),
            ],
        );
        assert!(
            loaded
                .cooking
                .iter()
                .all(|recipe| recipe.id != "fixture:refused")
        );
        check(
            &loaded,
            &stack(Tag::Compound(Default::default())),
            Some("fixture:kept"),
        );
        check(&loaded, &plain, None);
    }
    // Raw NBT can contain End; JSON deliberately refuses nested null first.
    for invalid in [Tag::End] {
        assert!(
            horizon_command::item::decoded_component_value(ID, &invalid, &items.component_registry)
                .is_err()
        );
        check(&recipes, &stack(invalid), None);
    }
}

#[test]
fn tooltip_display_recipe_uses_registered_component_sets_and_numeric_booleans() {
    use crate::crafting::{Recipe, Recipes};
    use horizon_world::loot::PackOverlay;
    use serde_json::json;
    use std::sync::Arc;

    let mut items = Items::empty();
    let registry = Arc::make_mut(&mut items.component_registry);
    registry
        .by_registry
        .entry("minecraft:data_component_type".into())
        .or_default()
        .ids = vec![
        "minecraft:tooltip_display".into(),
        "minecraft:damage".into(),
        "minecraft:unbreakable".into(),
        "minecraft:banner_patterns".into(),
        "fixture:transient".into(),
    ];
    // A declared persistent mod component need not have a built-in payload decoder.
    registry
        .persistent_components
        .push("fixture:declared".into());
    // Synthetic test prototype, not a claim about vanilla stone's defaults.
    items.component_defaults = Arc::new(std::collections::HashMap::from([(
        "minecraft:stone".into(),
        crate::json_nbt::json_to_compound(json!({
            "minecraft:tooltip_display": {
                "hide_tooltip": -0.0,
                "hidden_components": ["fixture:transient", "damage", "fixture:declared", "unbreakable"]
            }
        }).as_object().unwrap()).unwrap(),
    )]));
    let root = tempfile::tempdir().unwrap();
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let compile = |item: &str, patch: serde_json::Value, strict: bool| {
        Recipes::from_sources(root.path(), &items, &PackOverlay::NONE, vec![(
            "fixture:tooltip".into(),
            json!({
                "type":"minecraft:crafting_shaped", "pattern":["X"],
                "key":{"X":{"type":"neoforge:components","items":item,"components":patch,"strict":strict}},
                "result":{"id":"minecraft:diamond"}
            }),
        )], &load).unwrap()
    };
    let patch = json!({"minecraft:tooltip_display":{
        "hide_tooltip":0,
        "hidden_components":["unbreakable","fixture:declared","damage","fixture:transient","damage"]
    }});
    let recipe = compile("minecraft:stone", patch.clone(), false);
    let strict = compile("minecraft:stone", patch, true);
    let stone = ItemStack::new("minecraft:stone", 1);
    let outcome = |recipes: &Recipes, stack: &ItemStack| {
        recipes
            .craft(&context, &[Some(stack.clone())], 1)
            .map(|stack| stack.item)
    };
    assert_eq!(
        outcome(&recipe, &stone).as_deref(),
        Some("minecraft:diamond")
    );
    assert_eq!(
        outcome(&strict, &stone).as_deref(),
        Some("minecraft:diamond")
    );
    assert!(outcome(&recipe, &ItemStack::new("minecraft:dirt", 1)).is_none());
    let mut explicit = stone.clone();
    explicit.data_mut().set_other(
        "minecraft:tooltip_display",
        Some(Tag::compound([
            ("hide_tooltip", Tag::Double((-0.0_f64).to_bits())),
            (
                "hidden_components",
                Tag::List(vec![
                    Tag::String("fixture:declared".into()),
                    Tag::String("minecraft:damage".into()),
                    Tag::String("minecraft:unbreakable".into()),
                    Tag::String("fixture:transient".into()),
                    Tag::String(":damage".into()),
                ]),
            ),
            ("ignored", Tag::Int(17)),
        ])),
    );
    assert_eq!(
        outcome(&strict, &explicit).as_deref(),
        Some("minecraft:diamond")
    );
    let Recipe::Shaped { cells, .. } = &recipe.crafting[0].1 else {
        panic!("shaped recipe");
    };
    let predicate = cells[0].as_ref().unwrap();
    let (_, counts) = horizon_threads::allocation_probe::measure(|| {
        for _ in 0..32 {
            assert!(predicate.test_in(&context, std::hint::black_box(&explicit)));
            assert!(predicate.test_in(&context, std::hint::black_box(&stone)));
        }
    });
    assert_eq!(counts.calls, 0, "{counts:?}");
    assert_eq!(counts.bytes, 0);
    explicit.damage = 3;
    assert!(outcome(&recipe, &explicit).is_some());
    assert!(
        outcome(&strict, &explicit).is_none(),
        "strictness counts extra components"
    );
    explicit.damage = 0;
    for value in [
        Tag::compound([("hide_tooltip", Tag::Byte(1))]),
        Tag::compound([(
            "hidden_components",
            Tag::List(vec![Tag::String("damage".into())]),
        )]),
        Tag::compound([(
            "hidden_components",
            Tag::List(vec![
                Tag::String("damage".into()),
                Tag::String("fixture:unknown".into()),
            ]),
        )]),
    ] {
        explicit
            .data_mut()
            .set_other("minecraft:tooltip_display", Some(value));
        assert!(outcome(&recipe, &explicit).is_none());
    }
    let empty = compile(
        "minecraft:stone",
        json!({"minecraft:tooltip_display":{}}),
        false,
    );
    for collection in [
        Tag::List(vec![]),
        Tag::ByteArray(vec![]),
        Tag::IntArray(vec![]),
        Tag::LongArray(vec![]),
    ] {
        explicit.data_mut().set_other(
            "minecraft:tooltip_display",
            Some(Tag::compound([
                ("hide_tooltip", Tag::Float((-0.0_f32).to_bits())),
                ("hidden_components", collection),
            ])),
        );
        assert_eq!(
            outcome(&empty, &explicit).as_deref(),
            Some("minecraft:diamond")
        );
    }
    let hidden = compile(
        "minecraft:stone",
        json!({"minecraft:tooltip_display":{"hide_tooltip":0.5}}),
        false,
    );
    explicit.data_mut().set_other(
        "minecraft:tooltip_display",
        Some(Tag::compound([("hide_tooltip", Tag::Long(1_i64 << 32))])),
    );
    assert_eq!(
        outcome(&hidden, &explicit).as_deref(),
        Some("minecraft:diamond")
    );
    explicit.data_mut().set_other(
        "minecraft:tooltip_display",
        Some(Tag::compound([(
            "hide_tooltip",
            Tag::Double(f64::NAN.to_bits()),
        )])),
    );
    assert_eq!(
        outcome(&hidden, &explicit).as_deref(),
        Some("minecraft:diamond")
    );
    let removal = compile(
        "minecraft:stone",
        json!({"!minecraft:tooltip_display":{}}),
        true,
    );
    assert!(
        outcome(&removal, &stone).is_none(),
        "the prototype is present"
    );
    let mut removed = stone.clone();
    removed.data_mut().set_other(
        "!minecraft:tooltip_display",
        Some(Tag::compound([] as [(&str, Tag); 0])),
    );
    assert!(
        outcome(&recipe, &removed).is_none(),
        "removal wins over the prototype"
    );
    assert_eq!(
        outcome(&removal, &removed).as_deref(),
        Some("minecraft:diamond")
    );
    let banner = compile(
        "minecraft:white_banner",
        json!({"minecraft:tooltip_display":{
            "hidden_components":["banner_patterns","minecraft:banner_patterns"]
        }}),
        false,
    );
    let mut ominous = ItemStack::new("minecraft:white_banner", 1);
    ominous.data_mut().ominous_banner = true;
    assert_eq!(
        outcome(&banner, &ominous).as_deref(),
        Some("minecraft:diamond")
    );

    for invalid in [
        json!(true),
        json!({"hide_tooltip":"true"}),
        json!({"hide_tooltip":[]}),
        json!({"hidden_components":"damage"}),
        json!({"hidden_components":[1]}),
        json!({"hidden_components":["damage","fixture:unknown"]}),
        json!({"hidden_components":["Bad:identifier"]}),
    ] {
        let rejected = compile(
            "minecraft:stone",
            json!({"minecraft:tooltip_display":invalid}),
            false,
        );
        assert!(rejected.crafting.is_empty());
        assert!(rejected.by_type.values().all(|recipes| recipes.is_empty()));
        assert!(outcome(&rejected, &stone).is_none());
    }
}

#[test]
fn attack_animation_codec_matches_recipes_and_rejects_malformed_siblings() {
    use horizon_core::nbt::Compound;
    use serde_json::json;
    use std::{collections::HashMap, sync::Arc};

    const ID: &str = "minecraft:attack_animation";
    fn record(kind: &str, duration: Tag) -> Tag {
        Tag::Compound(
            [
                ("type".into(), Tag::String(kind.into())),
                ("duration".into(), duration),
            ]
            .into_iter()
            .collect(),
        )
    }
    fn stack(value: Tag) -> ItemStack {
        let mut stack = ItemStack::new("minecraft:diamond_sword", 3);
        stack.data_mut().other.push((ID.into(), value));
        stack
    }
    let ingredient = |patch: Value, strict| {
        json!({"type":"neoforge:components","items":"minecraft:diamond_sword",
            "components":patch,"strict":strict})
    };
    let recipe = |ingredient: Value| {
        json!({"type":"minecraft:crafting_shapeless","ingredients":[ingredient],
            "result":{"id":"minecraft:diamond"}})
    };
    let mut items = Items::empty();
    // Synthetic test prototype, not a claim about vanilla sword defaults.
    items.component_defaults = Arc::new(HashMap::from([(
        "minecraft:diamond_sword".into(),
        [(ID.into(), record("whack", Tag::Short(6)))]
            .into_iter()
            .collect::<Compound>(),
    )]));
    let valid = [
        (json!({}), "whack", 6),
        (json!({"type":"whack"}), "whack", 6),
        (json!({"duration":6}), "whack", 6),
        (json!({"type":"none"}), "none", 6),
        (json!({"type":"stab"}), "stab", 6),
        (json!({"duration":0}), "whack", 0),
        (json!({"duration":-0.75}), "whack", 0),
        (json!({"duration":6.75}), "whack", 6),
        (json!({"duration":4294967302_i64}), "whack", 6),
        (json!({"duration":2147483647}), "whack", i32::MAX),
        (json!({"duration":1e100}), "whack", i32::MAX),
        (json!({"duration":-0.0}), "whack", 0),
        (json!({"duration":true}), "whack", 1),
        (json!({"duration":false}), "whack", 0),
        (
            json!({"type":"stab","duration":1,"ignored":[1,2]}),
            "stab",
            1,
        ),
    ];
    let invalid = [
        json!([]),
        json!("whack"),
        json!(6),
        json!(null),
        json!(true),
        json!({"type":"WHACK"}),
        json!({"type":"minecraft:whack"}),
        json!({"type":"unknown"}),
        json!({"type":1}),
        json!({"type":{}}),
        json!({"duration":-1}),
        json!({"duration":-1.75}),
        json!({"duration":2147483648_i64}),
        json!({"duration":-1e100}),
        json!({"duration":"6"}),
        json!({"duration":"fixture:duration"}),
        json!({"duration":null}),
        json!({"type":null}),
        json!({"duration":[]}),
        json!({"duration":{}}),
    ];
    let mut sources = Vec::new();
    for (index, (value, _, _)) in valid.iter().enumerate() {
        sources.push((
            format!("fixture:valid_{index}"),
            recipe(ingredient(json!({(ID):value}), false)),
        ));
    }
    for (index, value) in invalid.iter().enumerate() {
        sources.push((
            format!("fixture:invalid_{index}"),
            recipe(ingredient(json!({(ID):value}), false)),
        ));
    }
    sources.extend([
        ("fixture:control".into(), recipe(json!("minecraft:apple"))),
        (
            "fixture:strict".into(),
            recipe(ingredient(json!({(ID):{}}), true)),
        ),
        (
            "fixture:removed".into(),
            recipe(ingredient(json!({"!minecraft:attack_animation":{}}), false)),
        ),
    ]);
    let load = load();
    let ctx = load.loop_context(LoopSide::Server, []);
    let root = tempfile::tempdir().unwrap();
    let recipes = crate::crafting::Recipes::from_sources(
        root.path(),
        &items,
        &horizon_world::loot::PackOverlay::NONE,
        sources,
        &load,
    )
    .unwrap();
    let matches = |id: &str, stack: &ItemStack| {
        recipes.recipe_matches(&ctx, id, &[Some(stack.clone()), None, None, None], 2)
    };
    for (index, (_, kind, duration)) in valid.iter().enumerate() {
        let id = format!("fixture:valid_{index}");
        assert!(
            matches(&id, &stack(record(kind, Tag::Int(*duration)))),
            "{id}"
        );
        assert!(
            !matches(
                &id,
                &stack(record(kind, Tag::Int(duration.wrapping_sub(1))))
            ),
            "{id}"
        );
        let different = if *kind == "stab" { "whack" } else { "stab" };
        assert!(
            !matches(&id, &stack(record(different, Tag::Int(*duration)))),
            "{id}"
        );
    }
    for index in 0..invalid.len() {
        let id = format!("fixture:invalid_{index}");
        assert!(
            !recipes.crafting.iter().any(|(name, _)| name == &id),
            "{id}"
        );
        assert!(
            recipes
                .by_type
                .values()
                .all(|entries| entries.iter().all(|entry| entry.id != id)),
            "{id}"
        );
    }
    let (_, result) = recipes
        .find(
            &ctx,
            &[Some(ItemStack::new("minecraft:apple", 1)), None, None, None],
            2,
        )
        .unwrap();
    assert_eq!(result.item.as_ref(), "minecraft:diamond");
    let inherited = ItemStack::new("minecraft:diamond_sword", 1);
    assert!(matches("fixture:valid_0", &inherited));
    assert!(matches("fixture:strict", &inherited));
    assert!(!matches("fixture:removed", &inherited));
    let mut extra = inherited.clone();
    extra
        .data_mut()
        .other
        .push(("minecraft:rarity".into(), Tag::String("rare".into())));
    assert!(matches("fixture:valid_0", &extra));
    assert!(!matches("fixture:strict", &extra));
    let mut removed = inherited.clone();
    removed.data_mut().other.push((
        "!minecraft:attack_animation".into(),
        Tag::Compound(Compound::new()),
    ));
    assert!(!matches("fixture:valid_0", &removed));
    assert!(matches("fixture:removed", &removed));
    assert!(!matches(
        "fixture:valid_0",
        &ItemStack::new("minecraft:apple", 1)
    ));
    assert!(!matches(
        "fixture:valid_0",
        &ItemStack::new("minecraft:diamond_sword", 0)
    ));
    let numeric = [
        (Tag::Byte(6), 6),
        (Tag::Short(6), 6),
        (Tag::Int(6), 6),
        (Tag::Long(4294967302), 6),
        (Tag::Float(6.75_f32.to_bits()), 6),
        (Tag::Double(6.75_f64.to_bits()), 6),
        (Tag::Float((-0.0_f32).to_bits()), 0),
        (Tag::Double((-0.75_f64).to_bits()), 0),
        (Tag::Float(f32::NAN.to_bits()), 0),
        (Tag::Double(f64::NAN.to_bits()), 0),
        (Tag::Float(f32::INFINITY.to_bits()), i32::MAX),
        (Tag::Double(f64::INFINITY.to_bits()), i32::MAX),
    ];
    for (number, expected) in numeric {
        let value = record("whack", number);
        assert_eq!(
            horizon_command::item::decoded_component_value(ID, &value, &items.component_registry)
                .unwrap(),
            record("whack", Tag::Int(expected)),
        );
        let id = match expected {
            0 => "fixture:valid_5",
            6 => "fixture:valid_0",
            _ => "fixture:valid_9",
        };
        assert!(matches(id, &stack(value)));
    }
    for number in [
        Tag::Byte(-1),
        Tag::Short(-1),
        Tag::Int(-1),
        Tag::Long(2147483648),
        Tag::Float(f32::NEG_INFINITY.to_bits()),
        Tag::Double(f64::NEG_INFINITY.to_bits()),
    ] {
        let value = record("whack", number);
        assert!(
            horizon_command::item::decoded_component_value(ID, &value, &items.component_registry)
                .is_err()
        );
        assert!(!matches("fixture:valid_0", &stack(value)));
    }
    let compiled = Ingredient::parse(&ingredient(json!({(ID):{}}), false), &items).unwrap();
    let same = stack(record("whack", Tag::Double(6.9_f64.to_bits())));
    let different = stack(record("stab", Tag::Int(6)));
    let (_, counts) = horizon_threads::allocation_probe::measure(|| {
        for _ in 0..100 {
            assert!(compiled.test_in(&ctx, std::hint::black_box(&same)));
            assert!(compiled.test_in(&ctx, std::hint::black_box(&inherited)));
            assert!(!compiled.test_in(&ctx, std::hint::black_box(&different)));
            assert!(!compiled.test_in(&ctx, std::hint::black_box(&removed)));
        }
    });
    assert_eq!(counts.calls, 0, "{counts:?}");
    assert_eq!(counts.bytes, 0, "{counts:?}");
}

#[test]
fn damage_resistant_component_recipes_use_loaded_holder_sets() {
    use horizon_world::damage_source::Catalog;
    use horizon_world::loot::PackOverlay;
    use serde_json::json;

    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("items.json"),
        r##"{"minecraft:stick":{"minecraft:damage_resistant":{"types":"#fixture:hot"}}}"##,
    )
    .unwrap();
    std::fs::write(
        root.path().join("damage-catalog.json"),
        json!({"data_version":horizon_world::DATA_VERSION,"types":[]}).to_string(),
    )
    .unwrap();
    let definition = json!({"message_id":"heat","scaling":"never","exhaustion":0});
    let files = vec![
        (
            "damage_type/fixture:heat".into(),
            vec![definition.to_string().into_bytes()],
        ),
        (
            "damage_type/minecraft:ember".into(),
            vec![definition.to_string().into_bytes()],
        ),
        (
            "tags/fixture:damage_type/hot".into(),
            vec![br#"{"values":["fixture:heat"]}"#.to_vec()],
        ),
        (
            "tags/fixture:damage_type/same_members".into(),
            vec![br#"{"values":["fixture:heat"]}"#.to_vec()],
        ),
        (
            "tags/minecraft:damage_type/is_fire".into(),
            vec![br#"{"values":["fixture:heat"]}"#.to_vec()],
        ),
    ];
    let overlay = PackOverlay::new(&files);
    let defaults = horizon_world::item_defaults::read(root.path()).unwrap();
    let mut items = Items::catalog(root.path(), &overlay, &defaults).unwrap();
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let recipes = |patch: Value, strict: bool, items: &Items| {
        crate::crafting::Recipes::from_sources(
            root.path(),
            items,
            &overlay,
            vec![
                ("fixture:resistant".into(), json!({
                    "type":"minecraft:smelting",
                    "ingredient":{"type":"neoforge:components","items":"minecraft:stick","components":patch,"strict":strict},
                    "result":{"id":"minecraft:diamond"}
                })),
                ("fixture:sibling".into(), json!({
                    "type":"minecraft:smelting","ingredient":"minecraft:stone",
                    "result":{"id":"minecraft:gold_ingot"}
                })),
            ],
            &load,
        )
        .unwrap()
    };
    let set = |stack: &mut ItemStack, value: Tag| {
        stack
            .data_mut()
            .set_loot_component("minecraft:damage_resistant", &Some(value));
    };
    let tag = |value: Value| crate::json_nbt::json_to_tag(&value).unwrap();
    for (literal, matching, different) in [
        (
            json!({"types":"#fixture:hot","ignored":{"field":7}}),
            vec![
                json!({"types":"#fixture:hot"}),
                json!({"types":"#fixture:hot","extra":false}),
            ],
            vec![
                json!({"types":"#fixture:same_members"}),
                json!({"types":["fixture:heat"]}),
            ],
        ),
        (
            json!({"types":"#is_fire"}),
            vec![
                json!({"types":"#is_fire"}),
                json!({"types":"#:is_fire"}),
                json!({"types":"#minecraft:is_fire"}),
            ],
            vec![
                json!({"types":"#fixture:hot"}),
                json!({"types":"fixture:heat"}),
            ],
        ),
        (
            json!({"types":":ember","ignored":5}),
            vec![
                json!({"types":"ember"}),
                json!({"types":"minecraft:ember"}),
                json!({"types":[":ember"]}),
            ],
            vec![
                json!({"types":"fixture:heat"}),
                json!({"types":["ember","ember"]}),
                json!({"types":[]}),
            ],
        ),
        (
            json!({"types":["fixture:heat","ember","fixture:heat"]}),
            vec![json!({"types":["fixture:heat",":ember","fixture:heat"]})],
            vec![
                json!({"types":["ember","fixture:heat","fixture:heat"]}),
                json!({"types":["fixture:heat","ember"]}),
            ],
        ),
        (
            json!({"types":[]}),
            vec![json!({"types":[]})],
            vec![json!({"types":"ember"})],
        ),
    ] {
        let book = recipes(json!({"minecraft:damage_resistant":literal}), false, &items);
        let mut stack = ItemStack::new("minecraft:stick", 1);
        for value in matching {
            set(&mut stack, tag(value));
            let cooked = book.cook(&context, "minecraft:smelting", &stack).unwrap();
            assert_eq!(cooked.id, "fixture:resistant");
            assert_eq!(&*cooked.result.item, "minecraft:diamond");
            let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
                for _ in 0..20 {
                    assert_eq!(
                        book.cook(&context, "minecraft:smelting", &stack)
                            .map(|recipe| recipe.id.as_str()),
                        Some("fixture:resistant"),
                    );
                }
            });
            assert_eq!(allocations.calls, 0, "{allocations:?}");
            assert_eq!(allocations.bytes, 0);
        }
        for value in different {
            set(&mut stack, tag(value));
            assert!(book.cook(&context, "minecraft:smelting", &stack).is_none());
        }
        assert_eq!(
            book.cook(
                &context,
                "minecraft:smelting",
                &ItemStack::new("minecraft:stone", 1)
            )
            .unwrap()
            .id,
            "fixture:sibling",
        );
    }
    let named = recipes(
        json!({"minecraft:damage_resistant":{"types":"#fixture:hot"}}),
        false,
        &items,
    );
    let strict = recipes(
        json!({"minecraft:damage_resistant":{"types":"#fixture:hot"}}),
        true,
        &items,
    );
    let mut stack = ItemStack::new("minecraft:stick", 1);
    assert!(named.cook(&context, "minecraft:smelting", &stack).is_some());
    assert!(
        strict
            .cook(&context, "minecraft:smelting", &stack)
            .is_some()
    );
    stack
        .data_mut()
        .set_loot_component("minecraft:repair_cost", &Some(Tag::Int(1)));
    assert!(named.cook(&context, "minecraft:smelting", &stack).is_some());
    assert!(
        strict
            .cook(&context, "minecraft:smelting", &stack)
            .is_none()
    );
    let absent = recipes(json!({"!minecraft:damage_resistant":{}}), false, &items);
    assert!(
        absent
            .cook(&context, "minecraft:smelting", &stack)
            .is_none()
    );
    let removal = Tag::compound([(
        "!minecraft:damage_resistant",
        Tag::Compound(Default::default()),
    )]);
    (_, stack.extra) = crate::item_data::decode_components(Some(&removal));
    assert!(
        absent
            .cook(&context, "minecraft:smelting", &stack)
            .is_some()
    );
    assert!(named.cook(&context, "minecraft:smelting", &stack).is_none());
    stack.count = 0;
    assert!(
        absent
            .cook(&context, "minecraft:smelting", &stack)
            .is_none()
    );

    for value in [
        json!(5),
        json!([]),
        json!({}),
        json!({"types":5}),
        json!({"types":[5]}),
        json!({"types":{"message_id":"heat","scaling":"never","exhaustion":0}}),
        json!({"types":["#fixture:hot"]}),
        json!({"types":"fixture:missing"}),
        json!({"types":"#fixture:missing"}),
        json!({"types":["fixture:heat","fixture:missing"]}),
        json!({"types":"Minecraft:ember"}),
        json!({"types":"#Fixture:hot"}),
        json!({"types":"..:ember"}),
        json!({"types":"#..:hot"}),
        json!({"types":"a:b:c"}),
    ] {
        let patch = json!({"minecraft:damage_resistant":value});
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(patch.as_object().unwrap()).unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        let error = components::validate(&rule, &items).unwrap_err();
        assert!(
            error.contains("component minecraft:damage_resistant:"),
            "{error}"
        );
        let book = recipes(patch, false, &items);
        assert!(
            book.cook(
                &context,
                "minecraft:smelting",
                &ItemStack::new("minecraft:stick", 1)
            )
            .is_none()
        );
        assert_eq!(
            book.cook(
                &context,
                "minecraft:smelting",
                &ItemStack::new("minecraft:stone", 1)
            )
            .unwrap()
            .id,
            "fixture:sibling",
        );
    }
    let empty = recipes(
        json!({"minecraft:damage_resistant":{"types":[]}}),
        false,
        &items,
    );
    for value in [
        Tag::ByteArray(vec![]),
        Tag::IntArray(vec![]),
        Tag::LongArray(vec![]),
    ] {
        let value = Tag::compound([("types", value)]);
        assert_eq!(
            horizon_command::item::decoded_component_value(
                "minecraft:damage_resistant",
                &value,
                &items.component_registry
            )
            .unwrap(),
            Tag::compound([("types", Tag::List(vec![]))]),
        );
        let mut stack = ItemStack::new("minecraft:stick", 1);
        set(&mut stack, value);
        assert!(empty.cook(&context, "minecraft:smelting", &stack).is_some());
    }
    for value in [
        Tag::End,
        Tag::Byte(0),
        Tag::Float((-0.0_f32).to_bits()),
        Tag::Double(f64::NAN.to_bits()),
        Tag::ByteArray(vec![0]),
        Tag::IntArray(vec![0]),
        Tag::LongArray(vec![0]),
        Tag::String(horizon_core::nbt::JavaString(vec![0xd800])),
        Tag::String(horizon_core::nbt::JavaString(vec![u16::from(b'#'), 0xdc00])),
        Tag::String(horizon_core::nbt::JavaString(vec![0xd83d, 0xde00])),
    ] {
        let value = Tag::compound([("types", value)]);
        assert!(
            horizon_command::item::decoded_component_value(
                "minecraft:damage_resistant",
                &value,
                &items.component_registry
            )
            .is_err()
        );
        let mut stack = ItemStack::new("minecraft:stick", 1);
        set(&mut stack, value);
        let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
            assert!(named.cook(&context, "minecraft:smelting", &stack).is_none());
        });
        assert_eq!(allocations.calls, 0, "{allocations:?}");
    }
    // Reload only rebinds tags: changed/removed definitions remain frozen and new
    // definitions do not appear until a fresh world load.
    let catalog = Catalog::load(root.path(), &overlay).unwrap();
    let reloaded_files = vec![
        (
            "damage_type/fixture:heat".into(),
            vec![br#"{"invalid":true}"#.to_vec()],
        ),
        (
            "damage_type/fixture:new".into(),
            vec![definition.to_string().into_bytes()],
        ),
        (
            "tags/fixture:damage_type/rebound".into(),
            vec![br#"{"values":["fixture:heat"]}"#.to_vec()],
        ),
    ];
    let reloaded_overlay = PackOverlay::new(&reloaded_files);
    let reloaded = catalog.reload_tags(root.path(), &reloaded_overlay).unwrap();
    items.bind_component_damage_catalog(&reloaded);
    let rebound = recipes(
        json!({"minecraft:damage_resistant":{"types":"#fixture:rebound"}}),
        false,
        &items,
    );
    let mut stack = ItemStack::new("minecraft:stick", 1);
    set(&mut stack, tag(json!({"types":"#fixture:rebound"})));
    assert_eq!(
        rebound
            .cook(&context, "minecraft:smelting", &stack)
            .unwrap()
            .id,
        "fixture:resistant"
    );
    let frozen = recipes(
        json!({"minecraft:damage_resistant":{"types":"fixture:heat"}}),
        false,
        &items,
    );
    set(&mut stack, tag(json!({"types":["fixture:heat"]})));
    assert!(
        frozen
            .cook(&context, "minecraft:smelting", &stack)
            .is_some()
    );
    for types in ["#fixture:hot", "fixture:new"] {
        let refused = recipes(
            json!({"minecraft:damage_resistant":{"types":types}}),
            false,
            &items,
        );
        assert!(
            refused
                .cook(&context, "minecraft:smelting", &stack)
                .is_none()
        );
        assert_eq!(
            refused
                .cook(
                    &context,
                    "minecraft:smelting",
                    &ItemStack::new("minecraft:stone", 1)
                )
                .unwrap()
                .id,
            "fixture:sibling",
        );
    }
}

#[test]
fn use_effects_component_recipe_decodes_defaults_bounds_and_matches() {
    use crate::crafting::{Recipes, ResolvedRecipe, ResolvedSources};
    use horizon_world::loot::PackOverlay;
    use std::sync::Arc;

    let mut items = Items::empty();
    items.component_defaults = Arc::new(std::collections::HashMap::from([(
        "minecraft:stick".into(),
        crate::json_nbt::json_to_compound(
            serde_json::json!({"minecraft:use_effects":{}})
                .as_object()
                .unwrap(),
        )
        .unwrap(),
    )]));
    let valid = [
        ("defaults", r#"{"use_effects":{}}"#, false),
        (
            "explicit",
            r#"{"minecraft:use_effects":{"can_sprint":false,"interact_vibrations":true,"speed_multiplier":0.2,"unknown":"ignored"}}"#,
            true,
        ),
        (
            "zero",
            r#"{"minecraft:use_effects":{"speed_multiplier":0}}"#,
            false,
        ),
        (
            "one",
            r#"{"minecraft:use_effects":{"can_sprint":0.5,"interact_vibrations":0,"speed_multiplier":1}}"#,
            false,
        ),
        ("removed", r#"{"!minecraft:use_effects":{}}"#, false),
    ];
    let invalid = [
        "false",
        "[]",
        r#""text""#,
        r#"{"can_sprint":"true"}"#,
        r#"{"interact_vibrations":{}}"#,
        r#"{"speed_multiplier":"0.2"}"#,
        r#"{"speed_multiplier":-0.01}"#,
        r#"{"speed_multiplier":1.001}"#,
    ];
    let recipe = |name: &str, patch: &str, strict: bool| {
        let file = Arc::new(
            serde_json::from_str::<serde_json::Value>(&format!(
                r#"{{"type":"minecraft:crafting_shaped","pattern":["A"],"key":{{"A":{{"type":"neoforge:components","items":"minecraft:stick","components":{patch},"strict":{strict}}}}},"result":{{"id":"minecraft:diamond"}}}}"#,
            ))
            .unwrap(),
        );
        ResolvedRecipe {
            name: format!("fixture:{name}"),
            resolved: file.clone(),
            file,
        }
    };
    let mut sources = ResolvedSources::default();
    for (name, patch, strict) in valid {
        sources.recipes.push(recipe(name, patch, strict));
    }
    for (index, payload) in invalid.iter().enumerate() {
        let patch = format!(r#"{{"minecraft:use_effects":{payload}}}"#);
        sources
            .recipes
            .push(recipe(&format!("invalid_{index}"), &patch, false));
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(
                serde_json::from_str::<serde_json::Value>(&patch)
                    .unwrap()
                    .as_object()
                    .unwrap(),
            )
            .unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        let error = components::validate(&rule, &items).unwrap_err();
        assert!(
            error.contains("component minecraft:use_effects:"),
            "{error}"
        );
    }
    let recipes =
        Recipes::from_resolved(&std::env::temp_dir(), &items, &PackOverlay::NONE, &sources)
            .unwrap();
    for (index, _) in invalid.iter().enumerate() {
        assert!(
            recipes
                .recipe(&format!("fixture:invalid_{index}"))
                .is_none()
        );
    }
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let matches = |name: &str, stack: &ItemStack| {
        recipes.recipe_matches(
            &context,
            &format!("fixture:{name}"),
            &[Some(stack.clone())],
            1,
        )
    };
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"use_effects":{}}}"#,
        &items,
    );
    let decoded = ingredient.shown.as_ref().unwrap()[0]
        .data()
        .unwrap()
        .other("minecraft:use_effects")
        .unwrap();
    assert_eq!(decoded.get("can_sprint"), Some(&Tag::Byte(0)));
    assert_eq!(decoded.get("interact_vibrations"), Some(&Tag::Byte(1)));
    assert_eq!(
        decoded.get("speed_multiplier"),
        Some(&Tag::Float(0.2_f32.to_bits())),
    );
    let mut stack = ItemStack::new("minecraft:stick", 1);
    for payload in [
        None,
        Some(serde_json::json!({})),
        Some(serde_json::json!({
            "can_sprint":0, "interact_vibrations":-2.5,
            "speed_multiplier":0.200000001, "unused":[1,2,3]
        })),
    ] {
        let tag = payload.map(|value| crate::json_nbt::json_to_tag(&value).unwrap());
        stack
            .data_mut()
            .set_loot_component("minecraft:use_effects", &tag);
        assert!(matches("defaults", &stack));
        assert!(matches("explicit", &stack));
        assert!(!matches("removed", &stack));
        let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
            assert!(ingredient.test_in(&context, &stack));
        });
        assert_eq!(allocations.calls, 0, "{allocations:?}");
    }
    for payload in [
        serde_json::json!({"can_sprint":true}),
        serde_json::json!({"interact_vibrations":false}),
        serde_json::json!({"speed_multiplier":0.3}),
        serde_json::json!({"can_sprint":"invalid"}),
    ] {
        let tag = crate::json_nbt::json_to_tag(&payload).unwrap();
        stack
            .data_mut()
            .set_loot_component("minecraft:use_effects", &Some(tag));
        assert!(!matches("defaults", &stack));
    }
    for (name, payload) in [
        ("zero", serde_json::json!({"speed_multiplier":0})),
        (
            "one",
            serde_json::json!({"can_sprint":2,"interact_vibrations":0,"speed_multiplier":1.0}),
        ),
    ] {
        let tag = crate::json_nbt::json_to_tag(&payload).unwrap();
        stack
            .data_mut()
            .set_loot_component("minecraft:use_effects", &Some(tag));
        assert!(matches(name, &stack));
        assert!(!matches("defaults", &stack));
    }
    stack
        .data_mut()
        .set_loot_component("minecraft:use_effects", &None);
    stack
        .data_mut()
        .set_loot_component("minecraft:repair_cost", &Some(Tag::Int(1)));
    assert!(matches("defaults", &stack));
    assert!(
        !matches("explicit", &stack),
        "strict rejects extra components"
    );
    let (damage, extra) = crate::item_data::decode_components(Some(&Tag::Compound(
        [(
            "!minecraft:use_effects".into(),
            Tag::Compound(Default::default()),
        )]
        .into_iter()
        .collect(),
    )));
    let removed = ItemStack {
        damage,
        extra,
        ..ItemStack::new("minecraft:stick", 1)
    };
    assert!(matches("removed", &removed));
    assert!(!matches("defaults", &removed));
    assert!(!matches("defaults", &ItemStack::new("minecraft:stick", 0)));
    for speed in [
        Tag::Float((-0.0_f32).to_bits()),
        Tag::Double((-0.0_f64).to_bits()),
        Tag::Float(f32::NAN.to_bits()),
        Tag::Float(f32::INFINITY.to_bits()),
        Tag::Float(f32::NEG_INFINITY.to_bits()),
    ] {
        let tag = Tag::Compound([("speed_multiplier".into(), speed)].into_iter().collect());
        assert!(
            horizon_command::item::decoded_component_value(
                "minecraft:use_effects",
                &tag,
                &items.component_registry,
            )
            .is_err()
        );
        let mut invalid_stack = ItemStack::new("minecraft:stick", 1);
        invalid_stack
            .data_mut()
            .set_loot_component("minecraft:use_effects", &Some(tag));
        assert!(!matches("zero", &invalid_stack));
    }
    // Unknown Java string keys are ignored, not lossy-decoded into known fields.
    let tag = Tag::Compound(
        [(horizon_core::nbt::JavaString(vec![0xd800]), Tag::Int(1))]
            .into_iter()
            .collect(),
    );
    stack
        .data_mut()
        .set_loot_component("minecraft:use_effects", &Some(tag));
    assert!(matches("defaults", &stack));
    for speed in [
        Tag::Byte(0),
        Tag::Short(0),
        Tag::Int(0),
        Tag::Long(0),
        Tag::Float(0.0_f32.to_bits()),
        Tag::Double(0.0_f64.to_bits()),
    ] {
        let tag = Tag::Compound([("speed_multiplier".into(), speed)].into_iter().collect());
        stack
            .data_mut()
            .set_loot_component("minecraft:use_effects", &Some(tag));
        assert!(matches("zero", &stack));
    }
    let rounded = Tag::Compound(
        [
            ("can_sprint".into(), Tag::Float(f32::NAN.to_bits())),
            (
                "interact_vibrations".into(),
                Tag::Double((-0.0_f64).to_bits()),
            ),
            (
                "speed_multiplier".into(),
                Tag::Double(1.0000000001_f64.to_bits()),
            ),
        ]
        .into_iter()
        .collect(),
    );
    stack
        .data_mut()
        .set_loot_component("minecraft:use_effects", &Some(rounded));
    assert!(
        matches("one", &stack),
        "bounds apply after float conversion"
    );
}
