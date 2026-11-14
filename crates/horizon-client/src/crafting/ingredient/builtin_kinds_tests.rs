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
fn attack_range_persistent_codec_matches_recipes_without_candidate_allocations() {
    use crate::crafting::{Recipe, Recipes};
    use horizon_world::loot::PackOverlay;
    use serde_json::{Value, json};
    use std::sync::Arc;

    fn file(patch: Value, strict: bool, item: &str) -> Value {
        json!({
            "type":"minecraft:crafting_shapeless",
            "ingredients":[{"type":"neoforge:components","items":item,
                "components":patch,"strict":strict}],
            "result":{"id":"minecraft:diamond"}
        })
    }
    fn ingredient(recipes: &Recipes) -> &Ingredient {
        let Recipe::Shapeless { ingredients, .. } = recipes.recipe("attack_range:recipe").unwrap()
        else {
            panic!("expected shapeless recipe");
        };
        &ingredients[0]
    }
    fn stack(item: &str, value: Tag) -> ItemStack {
        let mut stack = ItemStack::new(item, 3);
        stack
            .data_mut()
            .set_other("minecraft:attack_range", Some(value));
        stack
    }
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("data/minecraft/recipe")).unwrap();
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut items = Items::empty();
    // Synthetic test prototypes, not vanilla defaults: stone inherits an empty
    // range record and damage; dirt inherits a distinct six-field range.
    items.component_defaults = Arc::new(std::collections::HashMap::from([
        (
            "minecraft:stone".into(),
            horizon_core::nbt::Compound::from([
                (
                    "minecraft:attack_range".into(),
                    Tag::compound([("ignored", Tag::Int(9))]),
                ),
                ("minecraft:damage".into(), Tag::Int(0)),
            ]),
        ),
        (
            "minecraft:dirt".into(),
            horizon_core::nbt::Compound::from([(
                "minecraft:attack_range".into(),
                Tag::compound([
                    ("min_reach", Tag::Byte(1)),
                    ("max_reach", Tag::Short(2)),
                    ("min_creative_reach", Tag::Int(3)),
                    ("max_creative_reach", Tag::Long(4)),
                    ("hitbox_margin", Tag::Float(0.5f32.to_bits())),
                    ("mob_factor", Tag::Double(1.5f64.to_bits())),
                ]),
            )]),
        ),
    ]));
    let compile = |patch, strict, item| {
        Recipes::from_sources(
            root.path(),
            &items,
            &PackOverlay::NONE,
            vec![("attack_range:recipe".into(), file(patch, strict, item))],
            &load,
        )
        .unwrap()
    };
    let matches = |recipes: &Recipes, stack: &ItemStack| {
        recipes.recipe_matches(&context, "attack_range:recipe", &[Some(stack.clone())], 1)
    };
    let defaults = compile(
        json!({"minecraft:attack_range":{}}),
        false,
        "minecraft:stone",
    );
    let stone = ItemStack::new("minecraft:stone", 3);
    assert!(matches(&defaults, &stone));
    assert!(!matches(&defaults, &ItemStack::new("minecraft:dirt", 1)));
    assert!(!matches(&defaults, &ItemStack::new("minecraft:stone", 0)));
    assert_eq!(
        ingredient(&defaults).shown.as_ref().unwrap()[0]
            .data()
            .unwrap()
            .other("minecraft:attack_range"),
        Some(&Tag::compound([
            ("min_reach", Tag::Float(0.0f32.to_bits())),
            ("max_reach", Tag::Float(3.0f32.to_bits())),
            ("min_creative_reach", Tag::Float(0.0f32.to_bits())),
            ("max_creative_reach", Tag::Float(5.0f32.to_bits())),
            ("hitbox_margin", Tag::Float(0.3f32.to_bits())),
            ("mob_factor", Tag::Float(1.0f32.to_bits())),
        ]))
    );
    let full = compile(
        json!({"minecraft:attack_range":{"min_reach":1,"max_reach":2,
            "min_creative_reach":3,"max_creative_reach":4,
            "hitbox_margin":0.5,"mob_factor":1.5,"unknown":{"ignored":true}}}),
        true,
        "minecraft:dirt",
    );
    let dirt = ItemStack::new("minecraft:dirt", 1);
    assert!(matches(&full, &dirt));
    let mut override_dirt = dirt.clone();
    override_dirt.data_mut().set_other(
        "minecraft:attack_range",
        Some(Tag::compound([] as [(&str, Tag); 0])),
    );
    assert!(
        !matches(&full, &override_dirt),
        "explicit values replace the prototype"
    );

    // Every field is independent, including min > max: the codec has no
    // cross-field ordering validation. All NBT numeric widths become f32.
    for (name, maximum) in [
        ("min_reach", 64.0f64),
        ("max_reach", 64.0),
        ("min_creative_reach", 64.0),
        ("max_creative_reach", 64.0),
        ("hitbox_margin", 1.0),
        ("mob_factor", 2.0),
    ] {
        for number in [0.0, maximum] {
            let query = compile(
                json!({"minecraft:attack_range":{(name):number}}),
                false,
                "minecraft:stone",
            );
            for numeric in [
                Tag::Byte(number as i8),
                Tag::Short(number as i16),
                Tag::Int(number as i32),
                Tag::Long(number as i64),
                Tag::Float((number as f32).to_bits()),
                Tag::Double(number.to_bits()),
            ] {
                let current = stack(
                    "minecraft:stone",
                    Tag::compound([(name, numeric), ("ignored", Tag::Int(3))]),
                );
                assert!(matches(&query, &current), "{name}={number}");
            }
        }
        for boolean in [false, true] {
            let query = compile(
                json!({"minecraft:attack_range":{(name):boolean}}),
                false,
                "minecraft:stone",
            );
            let current = stack(
                "minecraft:stone",
                Tag::compound([(name, Tag::Int(i32::from(boolean)))]),
            );
            assert!(
                matches(&query, &current),
                "NbtOps byte booleans are numeric"
            );
        }
        for invalid in [
            Tag::Float((-0.0f32).to_bits()),
            Tag::Double((-0.0f64).to_bits()),
            Tag::Float(f32::NAN.to_bits()),
            Tag::Double(f64::NAN.to_bits()),
            Tag::Float(f32::INFINITY.to_bits()),
            Tag::Float(f32::NEG_INFINITY.to_bits()),
            Tag::Long(i64::MAX),
            Tag::Double(f64::INFINITY.to_bits()),
        ] {
            let current = stack("minecraft:stone", Tag::compound([(name, invalid.clone())]));
            assert!(!matches(&defaults, &current), "{name}: {invalid:?}");
            assert!(
                horizon_command::item::decoded_component_value(
                    "minecraft:attack_range",
                    &Tag::compound([(name, invalid)]),
                    &items.component_registry,
                )
                .is_err()
            );
        }
        for invalid in [
            json!(-0.1),
            json!(maximum + 0.1),
            json!("fixture:provider"),
            json!([]),
            json!({}),
            Value::Null,
        ] {
            let valid = file(
                json!({"minecraft:attack_range":{}}),
                false,
                "minecraft:stone",
            );
            let invalid = file(
                json!({"minecraft:attack_range":{(name):invalid}}),
                false,
                "minecraft:stone",
            );
            let files = [(
                "recipe/attack_range:recipe".into(),
                vec![
                    serde_json::to_vec(&valid).unwrap(),
                    serde_json::to_vec(&invalid).unwrap(),
                ],
            )];
            let omitted =
                Recipes::load(root.path(), &items, &PackOverlay::new(&files), &load).unwrap();
            assert!(omitted.crafting.is_empty(), "{name}: {invalid}");
            assert!(omitted.by_type.values().all(|recipes| recipes.is_empty()));
        }
    }
    for invalid in [json!(1), json!("fixture:provider"), json!([]), Value::Null] {
        let omitted = compile(
            json!({"minecraft:attack_range":invalid}),
            false,
            "minecraft:stone",
        );
        assert!(omitted.crafting.is_empty());
        assert!(omitted.by_type.values().all(|recipes| recipes.is_empty()));
    }
    let crossed = compile(
        json!({"minecraft:attack_range":{"min_reach":64,"max_reach":0,
            "min_creative_reach":64,"max_creative_reach":0}}),
        false,
        "minecraft:stone",
    );
    assert!(matches(
        &crossed,
        &stack(
            "minecraft:stone",
            Tag::compound([
                ("min_reach", Tag::Int(64)),
                ("max_reach", Tag::Int(0)),
                ("min_creative_reach", Tag::Int(64)),
                ("max_creative_reach", Tag::Int(0)),
            ])
        )
    ));
    // JSON's integral negative zero is intentionally converted to NBT +0.
    let json_zero = compile(
        json!({"minecraft:attack_range":{"min_reach":-0.0}}),
        false,
        "minecraft:stone",
    );
    assert!(matches(&json_zero, &stone));
    let rounded = stack(
        "minecraft:stone",
        Tag::compound([("max_reach", Tag::Double(3.00000001f64.to_bits()))]),
    );
    assert!(
        matches(&defaults, &rounded),
        "comparison uses boxed f32, not f64 or raw NBT"
    );
    let strict = compile(
        json!({"minecraft:attack_range":{}}),
        true,
        "minecraft:stone",
    );
    assert!(
        !matches(&strict, &stone),
        "strict components include inherited damage"
    );
    let strict_all = compile(
        json!({"minecraft:attack_range":{},"minecraft:damage":0}),
        true,
        "minecraft:stone",
    );
    assert!(matches(&strict_all, &stone));
    let strict_remove = compile(
        json!({"minecraft:attack_range":{},"!minecraft:damage":{}}),
        true,
        "minecraft:stone",
    );
    let mut removed_damage = stone.clone();
    removed_damage.data_mut().set_other(
        "!minecraft:damage",
        Some(Tag::compound([] as [(&str, Tag); 0])),
    );
    assert!(matches(&strict_remove, &removed_damage));
    let mut removed_range = stone.clone();
    removed_range.data_mut().set_other(
        "!minecraft:attack_range",
        Some(Tag::compound([] as [(&str, Tag); 0])),
    );
    assert!(!matches(&defaults, &removed_range));
    let removal = compile(
        json!({"!minecraft:attack_range":{"min_reach":"not decoded"}}),
        false,
        "minecraft:stone",
    );
    assert!(!matches(&removal, &stone));
    assert!(matches(&removal, &removed_range));
    let type_only = compile(json!({}), false, "minecraft:stone");
    assert!(matches(&type_only, &stone));
    assert!(matches(&type_only, &removed_range));
    let changed = stack(
        "minecraft:stone",
        Tag::compound([("mob_factor", Tag::Double(1.5f64.to_bits()))]),
    );
    assert!(!matches(&defaults, &changed));
    assert!(matches(&type_only, &changed));
    let candidate = ingredient(&defaults);
    let full_candidate = ingredient(&full);
    let (_, counts) = horizon_threads::allocation_probe::measure(|| {
        for _ in 0..100 {
            assert!(std::hint::black_box(candidate.test_in(&context, &stone)));
            assert!(std::hint::black_box(candidate.test_in(&context, &rounded)));
            assert!(!std::hint::black_box(candidate.test_in(&context, &changed)));
            assert!(std::hint::black_box(
                full_candidate.test_in(&context, &dirt)
            ));
        }
    });
    assert_eq!(counts.calls, 0, "{counts:?}");
    assert_eq!(counts.bytes, 0, "{counts:?}");
}
