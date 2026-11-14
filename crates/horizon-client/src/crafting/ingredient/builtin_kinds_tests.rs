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
