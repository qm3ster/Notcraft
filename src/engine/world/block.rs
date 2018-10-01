use image::RgbaImage;
use std::collections::HashMap;
use engine::VoxelProperties;
use cgmath::{Vector2, Vector3};
use engine::{Precomputed, Side, Voxel};

pub const AIR: BlockId = 0;
pub const STONE: BlockId = 1;
pub const DIRT: BlockId = 2;
pub const GRASS: BlockId = 3;
pub const WATER: BlockId = 4;

pub type BlockId = usize;

// fn create_basic_render_prototype(opaque: bool, offsets: [Vector2<f32>; 6]) -> impl BlockRenderPrototype {
//     #[derive(Copy, Clone, Debug, PartialEq)]
//     struct Basic {
//         opaque: bool,
//         offsets: [Vector2<f32>; 6],
//     }

//     impl BlockRenderPrototype for Basic {
//         fn block_texture(&self) -> RgbaImage { unimplemented!() }
//         fn can_merge(&self, other: &BlockRenderPrototype) -> bool { unimplemented!() }
//         fn is_opaque(&self) -> bool { self.opaque }
//         fn texture_for_side(&self, side: Side) -> Vector2<f32> {
//             self.offsets[match side {
//                 Side::Top => 0,
//                 Side::Bottom => 1,
//                 Side::Left => 2,
//                 Side::Right => 3,
//                 Side::Front => 4,
//                 Side::Back => 5,
//             }]
//         }
//     }

//     Basic { opaque, offsets }
// }

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BlockRenderPrototype {
    pub opaque: bool,
    pub texture_offsets: [Vector2<f32>; 6],
}

impl BlockRenderPrototype {
    pub fn texture_for_side(&self, side: Side) -> Vector2<f32> {
        self.texture_offsets[match side {
            Side::Right => 0,
            Side::Top => 1,
            Side::Front => 2,
            Side::Left => 3,
            Side::Bottom => 4,
            Side::Back => 5,
        }]
    }
}

pub struct BlockRegistry {
    current_id: BlockId,
    name_map: HashMap<String, BlockId>,
    map: HashMap<BlockId, BlockRenderPrototype>,
}

impl BlockRegistry {
    pub fn new() -> Self {
        BlockRegistry {
            current_id: 0,
            name_map: HashMap::default(),
            map: HashMap::default(),
        }
    }

// fn calculate_vertex_data(block: Block, pre: Precomputed) -> BlockVertex {
//     BlockVertex {
//         pos: pre.pos,
//         norm: pre.norm,
//         face: pre.face,
//         uv: pre.face_offset,
//         tile: match block {
//             Block::Air => Vector2::new(0.0, 0.0),
//             Block::Stone => Vector2::new(1.0, 0.0),
//             Block::Dirt => Vector2::new(2.0, 0.0),
//             Block::Grass => match pre.side {
//                 Side::Top => Vector2::new(0.0, 0.0),
//                 Side::Bottom => Vector2::new(2.0, 0.0),
//                 _ => Vector2::new(0.0, 1.0),
//             },
//             Block::Water => Vector2::new(1.0, 0.0),
//         }
//     }
// }
    pub fn with_defaults(mut self) -> Self {
        macro_rules! proto {
            ($opaque:expr, [$($x:expr, $y:expr);*]) => {
                BlockRenderPrototype {
                    opaque: $opaque,
                    texture_offsets: [$(Vector2::new($x as f32, $y as f32)),*]
                }
            };
        }

        self.register("air",   proto! { false, [0.0, 0.0; 0.0, 0.0; 0.0, 0.0; 0.0, 0.0; 0.0, 0.0; 0.0, 0.0] }, Some(AIR));
        self.register("stone", proto! { true,  [1.0, 0.0; 1.0, 0.0; 1.0, 0.0; 1.0, 0.0; 1.0, 0.0; 1.0, 0.0] }, Some(STONE));
        self.register("dirt",  proto! { true,  [2.0, 0.0; 2.0, 0.0; 2.0, 0.0; 2.0, 0.0; 2.0, 0.0; 2.0, 0.0] }, Some(DIRT));
        self.register("grass", proto! { true,  [0.0, 1.0; 0.0, 0.0; 0.0, 1.0; 0.0, 1.0; 2.0, 0.0; 0.0, 1.0] }, Some(GRASS));
        self.register("water", proto! { true,  [1.0, 0.0; 1.0, 0.0; 1.0, 0.0; 1.0, 0.0; 1.0, 0.0; 1.0, 0.0] }, Some(WATER));

        self
    }

    /// Register a block renderer prototype and return its ID
    pub fn register(&mut self, name: impl Into<String>, render_prototype: BlockRenderPrototype, id: Option<BlockId>) -> BlockId {
        // Force this item to have a particular ID, and panic if one already exists
        if let Some(force_id) = id {
            // Don't overwrite any previous items
            debug_assert!(!self.map.contains_key(&force_id));
            self.name_map.insert(name.into(), force_id);
            self.map.insert(force_id, render_prototype);
            force_id
        } else {
            // Since we can register anything anywhere, we step over the items that are already
            // registered. We just keep trying the next item until we find a free slot.
            while self.map.contains_key(&self.current_id) { self.current_id += 1; }
            self.name_map.insert(name.into(), self.current_id);
            self.map.insert(self.current_id, render_prototype);
            self.current_id
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=&BlockRenderPrototype> {
        self.map.iter().map(|(_, v)| v)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

use std::ops::Index;

impl Index<BlockId> for BlockRegistry {
    type Output = BlockRenderPrototype;

    fn index(&self, index: BlockId) -> &BlockRenderPrototype {
        &self.map[&index]
    }
}