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
