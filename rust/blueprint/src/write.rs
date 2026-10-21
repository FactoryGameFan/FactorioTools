//! Writing blueprint strings: `GridToBlueprintString` and the output half of
//! `EntityItemsConverter`, from `src/FactorioTools.Serialization`.
//!
//! The JSON text matches what `JsonSerializer` writes with `BlueprintSerialization.Context`:
//! properties in declaration order, null properties left out (`WhenWritingNull`), and .NET's
//! number and string formats (see [`crate::json_out`]). The compressed bytes differ from
//! .NET's `ZLibStream`, but they decompress to the same text.

use std::io::Write;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use dotnet_compat::NetDictionary;
use planner::context::Context;
use planner::data::{
    Blueprint, Entity, EntityItems, Icon, ModuleInsertPlan, Position, SignalId, entity_names,
    item_names, signal_types,
};
use planner::grid::EntityKind;
use planner::quality::Quality;

use crate::json_out::JsonWriter;

/// `GridToBlueprintString.FormatVersion(2, 0, 32, 0)`, stamped on every blueprint written.
pub const OUTPUT_VERSION: u64 = format_version(2, 0, 32, 0);

/// `defines.inventory.mining_drill_modules` in Factorio 2.0.
const MINING_DRILL_MODULE_INVENTORY: i32 = 2;
/// `defines.inventory.beacon_modules` in Factorio 2.0.
const BEACON_MODULE_INVENTORY: i32 = 1;

/// `FormatVersion`: Factorio's 64-bit version number.
pub const fn format_version(major: u16, minor: u16, patch: u16, developer: u16) -> u64 {
    ((major as u64) << 48) | ((minor as u64) << 32) | ((patch as u64) << 16) | developer as u64
}

/// `ParseVersion`.
pub const fn parse_version(version: u64) -> (u16, u16, u16, u16) {
    (
        ((version >> 48) & 0xFFFF) as u16,
        ((version >> 32) & 0xFFFF) as u16,
        ((version >> 16) & 0xFFFF) as u16,
        (version & 0xFFFF) as u16,
    )
}

/// `EntityNameToSize`: the footprint used to place the FBE offset entity. A name not in
/// the table counts as 1x1.
fn entity_size(name: &str) -> (f32, f32) {
    use entity_names::{aai_industry, vanilla};
    match name {
        vanilla::BEACON | vanilla::PUMPJACK => (3.0, 3.0),
        vanilla::BIG_ELECTRIC_POLE | vanilla::SUBSTATION => (2.0, 2.0),
        vanilla::MEDIUM_ELECTRIC_POLE
        | vanilla::PIPE
        | vanilla::PIPE_TO_GROUND
        | vanilla::HEAT_PIPE
        | vanilla::SMALL_ELECTRIC_POLE
        | aai_industry::SMALL_IRON_ELECTRIC_POLE => (1.0, 1.0),
        _ => (1.0, 1.0),
    }
}

/// `ToOutputQuality`: no quality field for normal.
fn output_quality(quality: Quality) -> Option<String> {
    (quality != Quality::NORMAL).then(|| quality.to_blueprint_string().to_string())
}

/// `ToOutputItems`: the 2.0 `items` array, or nothing when there are no modules.
fn output_items(
    modules: &[(String, i32)],
    inventory: i32,
    quality: Quality,
) -> Option<EntityItems> {
    if modules.is_empty() {
        return None;
    }

    let quality = output_quality(quality);
    let mut stack = 0;
    let plans = modules
        .iter()
        .map(|(name, count)| {
            let plan = ModuleInsertPlan {
                name: name.clone(),
                inventory,
                start_stack: stack,
                count: *count,
                quality: quality.clone(),
            };
            stack += count;
            plan
        })
        .collect();
    Some(EntityItems::InsertPlans(plans))
}

/// `GridToBlueprintString.Execute`: the planned grid as a blueprint string.
pub fn grid_to_blueprint_string(
    context: &Context,
    add_fbe_offset: bool,
    add_avoid_entities: bool,
) -> String {
    let options = &context.options;
    let grid = &context.grid;
    let mut entities = Vec::new();
    let mut next_entity_number = 1;
    let mut grid_id_to_entity_number: NetDictionary<i32, i32> = NetDictionary::new();

    // The local function `GetEntityNumber`.
    let mut entity_number_of = |id: i32, next: &mut i32| -> i32 {
        if let Some(number) = grid_id_to_entity_number.get(&id) {
            return *number;
        }
        let number = *next;
        *next += 1;
        grid_id_to_entity_number.add(id, number);
        number
    };

    for location in grid.entity_locations().iter() {
        let entity = grid
            .get(location)
            .expect("an entity at every entity location");
        let mut position = Position {
            x: location.x as f32,
            y: location.y as f32,
        };

        let output = match &entity.kind {
            EntityKind::PumpjackCenter { direction } => {
                let number = next_entity_number;
                next_entity_number += 1;
                Some(Entity {
                    entity_number: number,
                    direction: Some(*direction),
                    name: entity_names::vanilla::PUMPJACK.to_string(),
                    position,
                    items: output_items(
                        &options.pumpjack_modules,
                        MINING_DRILL_MODULE_INVENTORY,
                        options.pumpjack_module_quality,
                    ),
                    quality: output_quality(options.pumpjack_quality),
                    ..Entity::default()
                })
            }
            EntityKind::PumpjackSide { .. } => None,
            EntityKind::UndergroundPipe { direction } => {
                let number = next_entity_number;
                next_entity_number += 1;
                Some(Entity {
                    entity_number: number,
                    direction: Some(*direction),
                    name: entity_names::vanilla::PIPE_TO_GROUND.to_string(),
                    position,
                    ..Entity::default()
                })
            }
            EntityKind::HeatPipe => {
                let number = next_entity_number;
                next_entity_number += 1;
                Some(Entity {
                    entity_number: number,
                    name: options.heat_pipe_entity_name.clone(),
                    position,
                    ..Entity::default()
                })
            }
            // `case Pipe:` also matches `Terminal`, a `Pipe` subclass.
            EntityKind::Pipe | EntityKind::Terminal => {
                let number = next_entity_number;
                next_entity_number += 1;
                Some(Entity {
                    entity_number: number,
                    name: entity_names::vanilla::PIPE.to_string(),
                    position,
                    ..Entity::default()
                })
            }
            EntityKind::ElectricPoleCenter { neighbors } => {
                if options.electric_pole_width % 2 == 0 {
                    position.x += 0.5;
                }
                if options.electric_pole_height % 2 == 0 {
                    position.y += 0.5;
                }

                let number = entity_number_of(entity.id, &mut next_entity_number);
                // `Neighbors.Select(id => Grid[EntityIdToLocation[id]]).Select(GetEntityNumber)`:
                // the entity found is the neighbor itself, so this is its id.
                let neighbours = neighbors
                    .iter()
                    .map(|id| entity_number_of(grid.get_entity(id).id, &mut next_entity_number))
                    .collect();
                Some(Entity {
                    entity_number: number,
                    name: options.electric_pole_entity_name.clone(),
                    position,
                    quality: output_quality(options.electric_pole_quality),
                    neighbours: Some(neighbours),
                    ..Entity::default()
                })
            }
            EntityKind::ElectricPoleSide { .. } => None,
            EntityKind::BeaconCenter => {
                if options.beacon_width % 2 == 0 {
                    position.x += 0.5;
                }
                if options.beacon_height % 2 == 0 {
                    position.y += 0.5;
                }

                let number = next_entity_number;
                next_entity_number += 1;
                Some(Entity {
                    entity_number: number,
                    name: options.beacon_entity_name.clone(),
                    position,
                    items: output_items(
                        &options.beacon_modules,
                        BEACON_MODULE_INVENTORY,
                        options.beacon_module_quality,
                    ),
                    quality: output_quality(options.beacon_quality),
                    ..Entity::default()
                })
            }
            EntityKind::AvoidEntity => {
                if add_avoid_entities {
                    let number = next_entity_number;
                    next_entity_number += 1;
                    Some(Entity {
                        entity_number: number,
                        name: entity_names::vanilla::WALL.to_string(),
                        position,
                        ..Entity::default()
                    })
                } else {
                    None
                }
            }
            EntityKind::BeaconSide { .. } | EntityKind::TemporaryEntity => None,
        };

        if let Some(output) = output {
            entities.push(output);
        }
    }

    let blueprint = Blueprint {
        icons: vec![Icon {
            index: 1,
            signal: SignalId {
                name: entity_names::vanilla::PUMPJACK.to_string(),
                signal_type: signal_types::vanilla::ITEM.to_string(),
            },
        }],
        item: item_names::vanilla::BLUEPRINT.to_string(),
        entities,
        version: 0,
    };

    serialize_blueprint(&blueprint, add_fbe_offset)
}

/// `SerializeBlueprint`: stamps [`OUTPUT_VERSION`], writes Factorio 2.0 directions (the
/// internal value doubled), optionally adds the FBE offset wall, then compresses and
/// encodes. The C# rewrites the blueprint it is given; this leaves it alone.
pub fn serialize_blueprint(blueprint: &Blueprint, add_fbe_offset: bool) -> String {
    encode_blueprint_json(&output_blueprint_json(blueprint, add_fbe_offset))
}

/// The JSON text [`serialize_blueprint`] compresses.
pub fn output_blueprint_json(blueprint: &Blueprint, add_fbe_offset: bool) -> String {
    let mut entities: Vec<(Entity, Option<i32>)> = blueprint
        .entities
        .iter()
        .map(|entity| {
            // `ToOutputDirection`: 1.1-style 8-way values to 2.0's 16-way values.
            let direction = entity.direction.map(|d| d.value() * 2);
            (entity.clone(), direction)
        })
        .collect();

    // FBE shifts blueprint coordinates. A wall in the corner keeps FBE's rendering at the
    // grid's own coordinates.
    if add_fbe_offset && !entities.is_empty() {
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut max_entity_number = i32::MIN;

        for (entity, _) in &entities {
            let (width, height) = entity_size(&entity.name);
            max_x = max_x.max(entity.position.x + width / 2.0);
            max_y = max_y.max(entity.position.y + height / 2.0);
            max_entity_number = max_entity_number.max(entity.entity_number);
        }

        entities.push((
            Entity {
                entity_number: max_entity_number + 1,
                name: entity_names::vanilla::WALL.to_string(),
                position: Position {
                    // `(float)-Math.Ceiling(maxX)`.
                    x: (-f64::from(max_x).ceil()) as f32,
                    y: (-f64::from(max_y).ceil()) as f32,
                },
                ..Entity::default()
            },
            None,
        ));
    }

    let mut w = JsonWriter::new();
    w.begin_object();
    w.name("blueprint");
    write_blueprint(&mut w, blueprint, &entities, OUTPUT_VERSION);
    w.end_object();
    w.finish()
}

/// `Blueprint`, with each entity's already-converted direction beside it.
fn write_blueprint(
    w: &mut JsonWriter,
    blueprint: &Blueprint,
    entities: &[(Entity, Option<i32>)],
    version: u64,
) {
    w.begin_object();
    w.name("icons").begin_array();
    for icon in &blueprint.icons {
        w.begin_object();
        w.name("signal").begin_object();
        w.name("name").string(&icon.signal.name);
        w.name("type").string(&icon.signal.signal_type);
        w.end_object();
        w.name("index").int(icon.index.into());
        w.end_object();
    }
    w.end_array();

    w.name("entities").begin_array();
    for (entity, direction) in entities {
        write_entity(w, entity, *direction);
    }
    w.end_array();

    w.name("item").string(&blueprint.item);
    w.name("version").uint(version);
    w.end_object();
}

/// `Entity`. Null properties are left out, as `WhenWritingNull` does.
fn write_entity(w: &mut JsonWriter, entity: &Entity, direction: Option<i32>) {
    w.begin_object();
    w.name("entity_number").int(entity.entity_number.into());
    w.name("name").string(&entity.name);
    w.name("position").begin_object();
    w.name("x").f32(entity.position.x);
    w.name("y").f32(entity.position.y);
    w.end_object();
    if let Some(direction) = direction {
        w.name("direction").int(direction.into());
    }
    if let Some(mirror) = entity.mirror {
        w.name("mirror").bool(mirror);
    }
    if let Some(items) = &entity.items {
        w.name("items");
        write_items(w, items);
    }
    if let Some(quality) = &entity.quality {
        w.name("quality").string(quality);
    }
    if let Some(neighbours) = &entity.neighbours {
        w.name("neighbours").begin_array();
        for n in neighbours {
            w.int((*n).into());
        }
        w.end_array();
    }
    w.end_object();
}

/// `EntityItemsConverter.Write`.
fn write_items(w: &mut JsonWriter, items: &EntityItems) {
    match items {
        EntityItems::Counts(counts) => {
            w.begin_object();
            for (name, count) in counts {
                w.name(name).int((*count).into());
            }
            w.end_object();
        }
        EntityItems::InsertPlans(plans) => {
            w.begin_array();
            for plan in plans {
                w.begin_object();
                w.name("id").begin_object();
                w.name("name").string(&plan.name);
                if let Some(quality) = &plan.quality {
                    w.name("quality").string(quality);
                }
                w.end_object();
                w.name("items").begin_object();
                w.name("in_inventory").begin_array();
                for i in 0..plan.count {
                    w.begin_object();
                    w.name("inventory").int(plan.inventory.into());
                    w.name("stack").int((plan.start_stack + i).into());
                    w.end_object();
                }
                w.end_array();
                w.end_object();
                w.end_object();
            }
            w.end_array();
        }
    }
}

/// UTF-8, zlib, base64, then the version character `'0'`.
pub fn encode_blueprint_json(json: &str) -> String {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(json.as_bytes())
        .expect("writing to memory cannot fail");
    let compressed = encoder.finish().expect("writing to memory cannot fail");
    format!("0{}", STANDARD.encode(compressed))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Factorio oracle (`tools/capture-factorio-oracle.sh`). The planner crate's
    /// `tests/factorio_oracle.rs` checks the rest of it.
    fn factorio_oracle_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/oil-field/factorio-oracle.json")
    }

    /// `FactorioOracleTest.EntityNameToSizeMatchesTheDerivedCollisionBoxFootprint`: each
    /// footprint is the collision box rounded up to whole tiles, as the game derives it.
    /// Names absent from the fixture are skipped, which covers the AAI Industry pole: the
    /// oracle is captured with mods disabled on purpose.
    #[test]
    fn entity_size_matches_the_derived_collision_box_footprint() {
        let text = std::fs::read_to_string(factorio_oracle_path()).unwrap();
        let oracle: serde_json::Value = serde_json::from_str(&text).unwrap();
        let names = entity_names::vanilla::ALL
            .into_iter()
            .chain([entity_names::aai_industry::SMALL_IRON_ELECTRIC_POLE]);

        let mut mismatches = Vec::new();
        for name in names {
            let Some(entity) = oracle["entities"].get(name) else {
                continue;
            };
            let corner = |i: usize| {
                let point = &entity["collision_box"][i];
                (point[0].as_f64().unwrap(), point[1].as_f64().unwrap())
            };
            let (min, max) = (corner(0), corner(1));
            let width = (max.0 - min.0).ceil() as f32;
            let height = (max.1 - min.1).ceil() as f32;

            let size = entity_size(name);
            if size != (width, height) {
                mismatches.push(format!(
                    "{name}: entity_size says {}x{}, collision_box derives {width}x{height}",
                    size.0, size.1
                ));
            }
        }

        assert!(
            mismatches.is_empty(),
            "{}. Either Factorio changed (re-run tools/capture-factorio-oracle.sh and review \
             the diff), or the footprint table is wrong.",
            mismatches.join("; ")
        );
    }
}
