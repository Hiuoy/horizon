//! The content mods' mob effects and potions in the content root: their
//! network ids (`registries.json`), the ids commands check
//! (`command-registries.json`) and each potion's stacks in the creative tabs
//! that list vanilla's (`creative-inventory.json`).
use super::{ContentMod, J, Plan, Result};
use horizon_mods::content::Potion;

/// The mods' potions in registry order, by id as effects are. Each takes a
/// network id after vanilla's.
pub fn potions(mods: &[ContentMod]) -> Vec<&Potion> {
    let mut potions: Vec<&Potion> = mods.iter().flat_map(|m| &m.content.potions).collect();
    potions.sort_by(|a, b| a.id.cmp(&b.id));
    potions
}

/// The items a potion fills, as vanilla's creative tabs list them
/// (`CreativeModeTabs.generatePotionEffectTypes`), with their name keys' part.
const POTION_ITEMS: [(&str, &str); 4] = [
    ("minecraft:potion", "potion"),
    ("minecraft:splash_potion", "splash_potion"),
    ("minecraft:lingering_potion", "lingering_potion"),
    ("minecraft:tipped_arrow", "tipped_arrow"),
];

impl Plan<'_> {
    /// The new mob effects and potions for `registries.json`, each registry
    /// that has any: `PotionContents` carries a potion by its id.
    pub(super) fn effect_registries(&self) -> Vec<(&'static str, Vec<String>)> {
        let effects = self.effects.iter().map(|e| e.id.clone()).collect();
        let potions = self.potions.iter().map(|p| p.id.clone()).collect();
        [
            ("minecraft:mob_effect", effects),
            ("minecraft:potion", potions),
        ]
        .into_iter()
        .filter(|(_, ids): &(_, Vec<String>)| !ids.is_empty())
        .collect()
    }

    /// The ids `command-registries.json` checks: `/effect` and the mob_effect
    /// resource arguments take the mods' effects, a stack's `potion_contents`
    /// in `/give` and the item arguments their potions.
    pub(super) fn effect_command_ids(&self, doc: &mut J) {
        let new_effects = || self.effects.iter().map(|e| J::Str(e.id.clone()));
        if let Some(J::Arr(ids)) = doc.at("effects") {
            ids.extend(new_effects());
        }
        if let Some(J::Arr(ids)) = doc.at("by_registry/minecraft:mob_effect/ids") {
            ids.extend(new_effects());
        }
        if let Some(J::Arr(ids)) = doc.at("by_registry/minecraft:potion/ids") {
            ids.extend(self.potions.iter().map(|p| J::Str(p.id.clone())));
        }
    }

    /// The creative inventory (`creative-inventory.json`): the mods' potions
    /// in the tabs that list vanilla's, then the mods' tabs and items
    /// (`creative_tabs`).
    pub(super) fn creative_inventory(&self, doc: &mut J) -> Result<()> {
        if let Some(J::Arr(tabs)) = doc.at("tabs") {
            tabs.iter_mut().for_each(|tab| self.potion_stacks(tab));
        }
        self.creative_tabs(doc)
    }

    /// The mods' potions in a vanilla tab that lists potions: each item's
    /// (bottle, splash, lingering, tipped arrow) right after vanilla's last
    /// one, as `generatePotionEffectTypes` walks the registry and the mods'
    /// potions come after vanilla's. In the search tab each takes its English
    /// name (the mod's `item.minecraft.<item>.effect.<name>`, else the name
    /// itself; shortcut: no effect lines, as a mod item has only its name)
    /// and the tags of the vanilla stack before it.
    pub(super) fn potion_stacks(&self, tab: &mut J) {
        if self.potions.is_empty() {
            return;
        }
        let last = |items: &[J], item: &str| {
            let end = format!("id:\"{item}\"}}");
            items.iter().rposition(|s| {
                matches!(s, J::Str(s) if s.contains("\"minecraft:potion_contents\"") && s.ends_with(&end))
            })
        };
        let Some(J::Arr(items)) = tab.get("items") else {
            return;
        };
        let mut at: Vec<(usize, &str, &str)> = POTION_ITEMS
            .iter()
            .filter_map(|&(item, key)| Some((last(items, item)?, item, key)))
            .collect();
        // From the end, so an earlier index still points where it did.
        at.sort_by_key(|a| std::cmp::Reverse(a.0));
        for (index, item, key) in at {
            let stacks = self.potions.iter().map(|p| {
                J::Str(format!(
                    "{{components:{{\"minecraft:potion_contents\":{{potion:\"{}\"}}}},count:1,id:\"{item}\"}}",
                    p.id
                ))
            });
            let names = self.potions.iter().map(|p| {
                let name = self
                    .names
                    .get(&format!("item.minecraft.{key}.effect.{}", p.name));
                J::Arr(vec![J::Str(
                    name.cloned().unwrap_or_else(|| p.name.clone()),
                )])
            });
            let tags = match tab.get("tags") {
                Some(J::Arr(tags)) => tags.get(index).cloned(),
                _ => None,
            };
            let tags = self
                .potions
                .iter()
                .map(|_| tags.clone().unwrap_or(J::Arr(Vec::new())));
            for (list, add) in [
                ("items", stacks.collect::<Vec<_>>()),
                ("lines", names.collect()),
                ("tags", tags.collect()),
            ] {
                if let Some(J::Arr(list)) = tab.get_mut(list)
                    && list.len() > index
                {
                    list.splice(index + 1..index + 1, add);
                }
            }
        }
    }
}

#[cfg(test)]
impl Plan<'_> {
    /// A plan that adds nothing, for tests to fill the parts they need.
    pub(super) fn empty() -> Self {
        Plan {
            blocks: Vec::new(),
            items: Vec::new(),
            vanilla_items: Default::default(),
            effects: Vec::new(),
            potions: Vec::new(),
            fluids: Vec::new(),
            entity_types: Vec::new(),
            menu_types: Vec::new(),
            states: 0,
            defaults: Default::default(),
            sounds: Default::default(),
            tabs: Vec::new(),
            names: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_mods::content::{EffectCategory, MobEffect};

    fn potion(id: &str, name: &str) -> Potion {
        Potion {
            id: id.into(),
            name: name.into(),
            effects: Vec::new(),
        }
    }

    /// The mods' potions get network ids after vanilla's, by id, and join the
    /// ids `/give` checks, beside their effects.
    #[test]
    fn mod_potions_get_ids_after_vanillas() {
        let glow = MobEffect {
            id: "ruby:glow".into(),
            category: EffectCategory::Beneficial,
            color: 1,
        };
        let (base, zap, shine) = (
            potion("alpha:base", "base"),
            potion("alpha:zap", "zap"),
            potion("ruby:glow", "glow"),
        );
        let mut plan = Plan::empty();
        plan.effects = vec![&glow];
        plan.potions = vec![&base, &zap, &shine];
        let mut registry: J = serde_json::from_str(
            r#"{"minecraft:item":{"entries":{}},"minecraft:block":{"entries":{}},
                "minecraft:block_entity_type":{"entries":{}},
                "minecraft:mob_effect":{"entries":{"minecraft:luck":{"protocol_id":25}}},
                "minecraft:potion":{"entries":{"minecraft:water":{"protocol_id":0},
                "minecraft:awkward":{"protocol_id":3}}}}"#,
        )
        .unwrap();
        plan.json("registries.json", &mut registry).unwrap();
        let registry = serde_json::to_value(registry).unwrap();
        let id = |kind: &str, name: &str| registry[kind]["entries"][name]["protocol_id"].clone();
        assert_eq!(id("minecraft:mob_effect", "ruby:glow"), 26);
        assert_eq!(id("minecraft:potion", "minecraft:awkward"), 3);
        assert_eq!(id("minecraft:potion", "alpha:base"), 4);
        assert_eq!(id("minecraft:potion", "alpha:zap"), 5);
        assert_eq!(id("minecraft:potion", "ruby:glow"), 6);

        let mut commands = serde_json::from_str(
            r#"{"effects":["minecraft:speed"],
                "by_registry":{"minecraft:mob_effect":{"ids":["minecraft:speed"]},
                "minecraft:potion":{"ids":["minecraft:water"]}}}"#,
        )
        .unwrap();
        plan.effect_command_ids(&mut commands);
        let commands = serde_json::to_value(commands).unwrap();
        let effects = serde_json::json!(["minecraft:speed", "ruby:glow"]);
        assert_eq!(commands["effects"], effects);
        assert_eq!(
            commands["by_registry"]["minecraft:mob_effect"]["ids"],
            effects
        );
        assert_eq!(
            commands["by_registry"]["minecraft:potion"]["ids"],
            serde_json::json!(["minecraft:water", "alpha:base", "alpha:zap", "ruby:glow"])
        );
        // No potions: `registries.json` is asked for no potion registry.
        plan.potions.clear();
        assert_eq!(plan.effect_registries().len(), 1);
    }

    /// A mod potion's bottles and tipped arrow follow vanilla's last of each in
    /// the tabs that list potions, the search tab naming them in English.
    #[test]
    fn mod_potions_follow_vanillas_in_their_tabs() {
        let (glow, long) = (
            potion("ruby:glow", "glow"),
            potion("ruby:long/glow", "glow"),
        );
        let mut plan = Plan::empty();
        plan.potions = vec![&glow, &long];
        plan.names = [
            ("item.minecraft.potion.effect.glow", "Potion of Glowing"),
            (
                "item.minecraft.tipped_arrow.effect.glow",
                "Arrow of Glowing",
            ),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into();
        let stack = |item: &str, potion: &str| {
            format!(
                "{{components:{{\"minecraft:potion_contents\":{{potion:\"{potion}\"}}}},count:1,id:\"minecraft:{item}\"}}"
            )
        };
        let plain = |item: &str| format!("{{count:1,id:\"minecraft:{item}\"}}");
        let combat = [
            plain("arrow"),
            stack("tipped_arrow", "minecraft:water"),
            stack("tipped_arrow", "minecraft:awkward"),
        ];
        let food = [
            plain("apple"),
            stack("potion", "minecraft:water"),
            stack("potion", "minecraft:awkward"),
            stack("splash_potion", "minecraft:water"),
            plain("milk_bucket"),
        ];
        let doc = serde_json::json!({"tabs": [
            {"type": "CATEGORY", "items": combat},
            {"type": "CATEGORY", "items": food},
            {"type": "SEARCH", "items": [plain("apple"), stack("potion", "minecraft:water")],
                "lines": [["Apple"], ["Water Bottle"]],
                "tags": [["minecraft:horse_food"], ["minecraft:brewing_potion_inputs"]]},
        ]});
        let mut doc: J = serde_json::from_value(doc).unwrap();
        plan.creative_inventory(&mut doc).unwrap();
        let doc = serde_json::to_value(doc).unwrap();
        assert_eq!(
            doc["tabs"][0]["items"],
            serde_json::json!([
                plain("arrow"),
                stack("tipped_arrow", "minecraft:water"),
                stack("tipped_arrow", "minecraft:awkward"),
                stack("tipped_arrow", "ruby:glow"),
                stack("tipped_arrow", "ruby:long/glow"),
            ])
        );
        assert_eq!(
            doc["tabs"][1]["items"],
            serde_json::json!([
                plain("apple"),
                stack("potion", "minecraft:water"),
                stack("potion", "minecraft:awkward"),
                stack("potion", "ruby:glow"),
                stack("potion", "ruby:long/glow"),
                stack("splash_potion", "minecraft:water"),
                stack("splash_potion", "ruby:glow"),
                stack("splash_potion", "ruby:long/glow"),
                plain("milk_bucket"),
            ])
        );
        let search = &doc["tabs"][2];
        assert_eq!(
            search["items"],
            serde_json::json!([
                plain("apple"),
                stack("potion", "minecraft:water"),
                stack("potion", "ruby:glow"),
                stack("potion", "ruby:long/glow"),
            ])
        );
        assert_eq!(
            search["lines"],
            serde_json::json!([
                ["Apple"],
                ["Water Bottle"],
                ["Potion of Glowing"],
                ["Potion of Glowing"]
            ])
        );
        let brewing = serde_json::json!(["minecraft:brewing_potion_inputs"]);
        assert_eq!(search["tags"][2], brewing);
        assert_eq!(search["tags"][3], brewing);
    }
}
