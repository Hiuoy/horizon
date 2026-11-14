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
