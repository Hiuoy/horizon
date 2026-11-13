use super::*;
use horizon_core::nbt::Tag;
use horizon_world::native_load_context::{DataLoadOrigin, LoadContext, LoadInputs, LoopSide};

fn load() -> LoadContext {
    LoadContext::new(
        DataLoadOrigin::ServerDataLoad,
        std::sync::Arc::new(LoadInputs::new(Default::default(), Default::default())),
        [],
    )
}
fn parse(source: &str, items: &Items) -> Ingredient {
    Ingredient::parse(&serde_json::from_str(source).unwrap(), items).unwrap()
}
fn resources() -> Items {
    let root = std::path::PathBuf::from(
        std::env::var_os("HORIZON_TEST_RESOURCES").expect("prepared resources"),
    );
    let defaults = horizon_world::item_defaults::read(&root).unwrap();
    Items::catalog(&root, &horizon_world::loot::PackOverlay::NONE, &defaults).unwrap()
}
#[test]
#[ignore = "requires prepared resources"]
fn components_match_damage_ignore_count_and_refuse_empty() {
    let items = resources();
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5}}"#,
        &items,
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut sword = ItemStack::new("minecraft:diamond_sword", 3);
    sword.damage = 5;
    assert!(ingredient.test_in(&context, &sword));
    sword.damage = 0;
    assert!(!ingredient.test_in(&context, &sword));
    sword.damage = 5;
    sword.count = 0;
    assert!(!ingredient.test_in(&context, &sword));
    assert!(!ingredient.test_in(&context, &ItemStack::new("minecraft:air", 1)));
    assert!(!ingredient.is_simple());
}
#[test]
#[ignore = "requires prepared resources"]
fn components_strict_counts_max_damage_default_and_custom_name() {
    let items = resources();
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5},"strict":true}"#,
        &items,
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
    sword.damage = 5;
    // MAX_DAMAGE is a real prototype component, absent from the literal patch.
    assert!(
        items.component_defaults["minecraft:diamond_sword"]
            .contains_key(&"minecraft:max_damage".into())
    );
    assert!(!ingredient.test_in(&context, &sword));
    sword.data_mut().set_loot_component(
        "minecraft:custom_name",
        &Some(Tag::String("Named sword".into())),
    );
    assert!(!ingredient.test_in(&context, &sword));

    // A literal supported-only prototype isolates strict extra-component behavior.
    let mut supported = Items::empty();
    supported.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([
        ("minecraft:diamond_sword".into(), crate::json_nbt::json_to_compound(
            serde_json::json!({"minecraft:damage":0,"minecraft:max_damage":1561,"minecraft:max_stack_size":1}).as_object().unwrap()).unwrap())
    ]));
    let complete = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5,"minecraft:max_damage":1561,"minecraft:max_stack_size":1},"strict":true}"#,
        &supported,
    );
    let mut plain = ItemStack::new("minecraft:diamond_sword", 1);
    plain.damage = 5;
    assert!(complete.test_in(&context, &plain));
    assert!(!complete.test_in(&context, &sword));
    let missing_max = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5,"minecraft:max_stack_size":1},"strict":true}"#,
        &supported,
    );
    assert!(
        !missing_max.test_in(&context, &plain),
        "the literal max_damage default must be named"
    );
}
#[test]
#[ignore = "requires prepared resources"]
fn components_removal_requires_absence_including_defaults() {
    let items = resources();
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"!minecraft:enchantments":{}}}"#,
        &items,
    );
    assert!(!ingredient.shown.as_ref().unwrap()[0].carries_component(
        items.get("minecraft:diamond_sword"),
        "minecraft:enchantments"
    ));
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
    assert!(!ingredient.test_in(&context, &sword));
    let patch = Tag::compound([("!minecraft:enchantments", Tag::Compound(Default::default()))]);
    (_, sword.extra) = crate::item_data::decode_components(Some(&patch));
    assert!(ingredient.test_in(&context, &sword));
    sword = ItemStack::new("minecraft:diamond_sword", 1);
    sword.data_mut().set_loot_component(
        "minecraft:enchantments",
        &Some(Tag::Compound(Default::default())),
    );
    assert!(!ingredient.test_in(&context, &sword));
}
#[test]
#[ignore = "requires prepared resources"]
fn block_tag_logs_uses_block_items_and_rejects_hash() {
    let items = resources();
    let ingredient = parse(
        r#"{"type":"neoforge:block_tag","tag":"minecraft:logs"}"#,
        &items,
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    assert!(ingredient.test_in(&context, &ItemStack::new("minecraft:oak_log", 1)));
    assert!(!ingredient.test_in(&context, &ItemStack::new("minecraft:stone", 1)));
    assert!(ingredient.is_simple());
    let mut alias_items = items.clone();
    alias_items.block_tag_order.insert(
        "fixture:wall_signs".into(),
        vec!["minecraft:oak_wall_sign".into()],
    );
    let wall = parse(
        r#"{"type":"neoforge:block_tag","tag":"fixture:wall_signs"}"#,
        &alias_items,
    );
    assert_eq!(wall.order, ["minecraft:oak_sign"]);
    assert!(
        Ingredient::parse(
            &serde_json::json!({"type":"neoforge:block_tag","tag":"#minecraft:logs"}),
            &items
        )
        .is_none()
    );
}
#[test]
fn custom_display_matches_base_but_shows_another_item() {
    let items = Items::empty();
    let ingredient = parse(
        r#"{"type":"neoforge:custom_display","base":"minecraft:apple","display":{"type":"minecraft:item","item":"minecraft:diamond"}}"#,
        &items,
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    assert!(ingredient.test_in(&context, &ItemStack::new("minecraft:apple", 1)));
    assert!(!ingredient.test_in(&context, &ItemStack::new("minecraft:diamond", 1)));
    assert!(ingredient.is_simple());
    assert_eq!(
        crate::recipe_book::Book::default().items_of(&ingredient),
        ["minecraft:diamond"]
    );
    assert_eq!(ingredient.order, ["minecraft:apple"]);
    let placement = crate::crafting::Placement::of(vec![Some(ingredient)], None);
    assert_eq!(placement.ingredients[0].order, ["minecraft:apple"]);
    // 26.3 TagSlotDisplay carries an item HolderSet, including the # prefix.
    let tags = Items::for_tests(std::collections::HashMap::from([(
        "minecraft:planks".into(),
        std::collections::HashSet::from(["minecraft:oak_planks".into()]),
    )]));
    let tagged = parse(
        r##"{"type":"neoforge:custom_display","base":"minecraft:apple","display":{"type":"minecraft:tag","tag":"#minecraft:planks"}}"##,
        &tags,
    );
    assert_eq!(
        crate::recipe_book::Book::default().items_of(&tagged),
        ["minecraft:oak_planks"]
    );
}
#[test]
fn simplicity_follows_all_children_and_custom_display_base() {
    let items = Items::empty();
    for source in [
        r#"{"type":"neoforge:compound","children":["minecraft:apple","minecraft:stone"]}"#,
        r#"{"type":"neoforge:intersection","children":["minecraft:apple","minecraft:apple"]}"#,
        r#"{"type":"neoforge:difference","base":["minecraft:apple","minecraft:stone"],"subtracted":"minecraft:stone"}"#,
    ] {
        assert!(parse(source, &items).is_simple());
    }
    let source = r#"{"type":"neoforge:custom_display","base":{"type":"neoforge:components","items":"minecraft:apple","components":{}},"display":{"type":"minecraft:item","item":"minecraft:diamond"}}"#;
    assert!(!parse(source, &items).is_simple());
    let mut custom = Ingredient::of(vec!["minecraft:apple".into()]);
    custom.custom = Some(("fixture:custom".into(), Default::default()));
    assert!(!custom.is_simple());
}

#[test]
fn builtin_kinds_bypass_mod_resolution_and_honor_type_key_priority() {
    let load = load();
    for source in [
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5}}"#,
        r#"{"type":"neoforge:block_tag","tag":"minecraft:logs"}"#,
        r#"{"type":"fixture:wrong","neoforge:ingredient_type":"neoforge:custom_display","base":"minecraft:apple","display":{"type":"minecraft:item","item":"minecraft:diamond"}}"#,
    ] {
        let mut json: Value = serde_json::from_str(source).unwrap();
        let original = json.clone();
        assert!(!Ingredient::resolve(&mut json, &load));
        assert_eq!(json, original);
    }
}

#[test]
fn recipe_book_and_placement_use_component_ingredient_item_lists() {
    use crate::recipe_book::{Book, Category, Display, Entry};
    let items = Items::empty();
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5}}"#,
        &items,
    );
    let entry = Entry {
        id: "fixture:components".into(),
        display_id: -1,
        category: Category::CraftingEquipment,
        group: None,
        display: Display::Shapeless(vec![ingredient.clone()]),
        result: ItemStack::new("minecraft:diamond", 1),
        variants: Vec::new(),
        stacks: Vec::new(),
    };
    let mut book = Book::default();
    let placement = crate::crafting::Placement::of(vec![Some(ingredient)], None);
    book.placements.insert(entry.id.clone(), placement.clone());
    let mut stock = crate::menu::place::Stacked::default();
    // An undamaged sword cannot satisfy the concrete recipe, but the book and
    // placement intentionally account only for its item holder.
    stock.account(&std::sync::Arc::from("minecraft:diamond_sword"), 1);
    assert!(book.can_craft(&entry, &stock));
    assert!(stock.can_craft(&placement.ingredients.iter().collect::<Vec<_>>()));
    assert_eq!(
        book.items_of(&placement.ingredients[0]),
        ["minecraft:diamond_sword"]
    );
}

#[test]
fn item_model_component_literals_decode_validate_and_match() {
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:stick".into(),
        crate::json_nbt::json_to_compound(
            serde_json::json!({"minecraft:item_model":"unregistered/model"})
                .as_object()
                .unwrap(),
        )
        .unwrap(),
    )]));
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    for source in [
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"unregistered/model"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"minecraft:unregistered/model"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":":unregistered/model"}}"#,
    ] {
        let ingredient = parse(source, &items);
        assert_eq!(
            ingredient.shown.as_ref().unwrap()[0]
                .data()
                .unwrap()
                .other("minecraft:item_model"),
            Some(&Tag::String("minecraft:unregistered/model".into())),
        );
        let mut stack = ItemStack::new("minecraft:stick", 1);
        let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
            assert!(ingredient.test_in(&context, &stack));
        });
        assert_eq!(allocations.calls, 0, "{allocations:?}");
        for model in [
            "unregistered/model",
            "minecraft:unregistered/model",
            ":unregistered/model",
        ] {
            stack
                .data_mut()
                .set_loot_component("minecraft:item_model", &Some(Tag::String(model.into())));
            let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
                assert!(ingredient.test_in(&context, &stack));
            });
            assert_eq!(allocations.calls, 0, "{allocations:?}");
        }
        for model in [
            "minecraft:other",
            "fixture:unregistered/model",
            "Minecraft:unregistered/model",
        ] {
            stack
                .data_mut()
                .set_loot_component("minecraft:item_model", &Some(Tag::String(model.into())));
            assert!(!ingredient.test_in(&context, &stack));
        }
    }
    for source in [
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"Minecraft:stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"minecraft:Stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"bad namespace:stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"..:stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"a:b:c"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":5}}"#,
    ] {
        let value: serde_json::Value = serde_json::from_str(source).unwrap();
        assert!(Ingredient::parse(&value, &items).is_none());
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(value["components"].as_object().unwrap())
                .unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        let error = components::validate(&rule, &items).unwrap_err();
        assert!(error.contains("component minecraft:item_model:"), "{error}");
    }
    for model in ["", ":", "minecraft:"] {
        assert_eq!(
            horizon_command::item::decoded_component_value(
                "minecraft:item_model",
                &Tag::String(model.into()),
                &items.component_registry,
            )
            .unwrap(),
            Tag::String("minecraft:".into()),
        );
    }
    for units in [vec![0xd800], vec![0xdc00], vec![0xd83d, 0xde00]] {
        let invalid_utf16 = Tag::String(horizon_core::nbt::JavaString(units));
        assert!(
            horizon_command::item::decoded_component_value(
                "minecraft:item_model",
                &invalid_utf16,
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
    for invalid in [
        Tag::End,
        Tag::compound([("target", Tag::End)]),
        Tag::compound([("tracked", Tag::End)]),
    ] {
        assert!(
            horizon_command::item::decoded_component_value(ID, &invalid, &items.component_registry)
                .is_err()
        );
        check(&recipes, &stack(invalid), None);
    }
}
