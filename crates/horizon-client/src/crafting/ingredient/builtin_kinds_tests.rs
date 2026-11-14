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
