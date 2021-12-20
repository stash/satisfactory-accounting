use log::warn;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use satisfactory_accounting::accounting::{
    BuildNode, Building, BuildingSettings, GeneratorSettings, ManufacturerSettings, MinerSettings,
    Node, NodeKind, PumpSettings,
};
use satisfactory_accounting::database::{
    BuildingId, BuildingKind, BuildingKindId, BuildingType, ItemId, RecipeId,
};

use crate::GetDb;

mod balance;
mod building;
mod drag;
mod graph_manipulation;
mod group;
mod icon;

#[derive(Debug, PartialEq, Properties)]
pub struct Props {
    /// The node to display.
    pub node: Node,
    /// Path to this node in the tree.
    pub path: Vec<usize>,
    /// Callback to tell the parent to delete this node.
    #[prop_or_default]
    pub delete: Option<Callback<usize>>,
    /// Callback to tell the parent to copy this node.
    #[prop_or_default]
    pub copy: Option<Callback<usize>>,
    /// Callback to tell the parent to replace this node.
    pub replace: Callback<(usize, Node)>,
    /// Callback to tell the parent to move a node.
    pub move_node: Callback<(Vec<usize>, Vec<usize>)>,
}

/// Messages which can be sent to a Node.
pub enum Msg {
    // Messages for groups:
    /// Replace the child at the given index with the specified node.
    ReplaceChild { idx: usize, replacement: Node },
    /// Delete the child at the specified index.
    DeleteChild { idx: usize },
    /// Copy the child at the specified index.
    CopyChild { idx: usize },
    /// Add the given node as a child at the end of the list.
    AddChild { child: Node },
    /// Rename this node.
    Rename { name: String },
    /// When another node is dragged over this one.
    DragOver { insert_pos: usize },
    /// When another dragging node leaves this one.
    DragLeave,
    /// Move a node between positions.
    MoveNode {
        src_path: Vec<usize>,
        dest_path: Vec<usize>,
    },

    // Messages for buildings:
    /// Change the building type of this node.
    ChangeType { id: BuildingId },
    /// Change the recipe for the building, if a manufacturer.
    ChangeRecipe { id: RecipeId },
    /// Change the item for the building, if a Generator, Miner, or Pump.
    ChangeItem { id: ItemId },
}

/// Display for a single AccountingGraph node.
#[derive(Default)]
pub struct NodeDisplay {
    /// Element where children are attached.
    children: NodeRef,
    /// When a drag is in progress and over our children area, this is the proposed insert
    /// position.
    insert_pos: Option<usize>,
}

impl Component for NodeDisplay {
    type Message = Msg;
    type Properties = Props;

    fn create(_: &Context<Self>) -> Self {
        Default::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let our_idx = ctx.props().path.last().copied().unwrap_or_default();
        let db = ctx.db();
        match msg {
            Msg::ReplaceChild { idx, replacement } => {
                if let NodeKind::Group(group) = ctx.props().node.kind() {
                    if idx < group.children.len() {
                        let mut new_group = group.clone();
                        new_group.children[idx] = replacement;
                        ctx.props().replace.emit((our_idx, new_group.into()));
                    } else {
                        warn!(
                            "Cannot replace child index {}; out of range for this group",
                            idx
                        );
                    }
                } else {
                    warn!("Cannot replace child of a non-group");
                }
                false
            }
            Msg::DeleteChild { idx } => {
                if let NodeKind::Group(group) = ctx.props().node.kind() {
                    if idx < group.children.len() {
                        let mut new_group = group.clone();
                        new_group.children.remove(idx);
                        ctx.props().replace.emit((our_idx, new_group.into()));
                    } else {
                        warn!(
                            "Cannot delete child index {}; out of range for this group",
                            idx
                        );
                    }
                } else {
                    warn!("Cannot delete child of a non-group");
                }
                false
            }
            Msg::CopyChild { idx } => {
                if let NodeKind::Group(group) = ctx.props().node.kind() {
                    if idx < group.children.len() {
                        let mut new_group = group.clone();
                        let copied = new_group.children[idx].clone();
                        new_group.children.insert(idx + 1, copied);
                        ctx.props().replace.emit((our_idx, new_group.into()));
                    } else {
                        warn!(
                            "Cannot copy child index {}; out of range for this group",
                            idx
                        );
                    }
                } else {
                    warn!("Cannot copy child of a non-group");
                }
                false
            }
            Msg::AddChild { child } => {
                if let NodeKind::Group(group) = ctx.props().node.kind() {
                    let mut new_group = group.clone();
                    new_group.children.push(child);
                    ctx.props().replace.emit((our_idx, new_group.into()));
                } else {
                    warn!("Cannot add child to a non-group");
                }
                false
            }
            Msg::Rename { name } => {
                if let NodeKind::Group(group) = ctx.props().node.kind() {
                    let name = name.trim().to_owned();
                    if name != group.name {
                        let mut new_group = group.clone();
                        new_group.name = name;
                        ctx.props().replace.emit((our_idx, new_group.into()));
                    }
                } else {
                    warn!("Cannot rename a non-group");
                }
                false
            }
            Msg::DragOver { insert_pos } => {
                self.insert_pos = Some(insert_pos);
                true
            }
            Msg::DragLeave => {
                self.insert_pos = None;
                true
            }
            Msg::MoveNode {
                src_path,
                dest_path,
            } => {
                let path = &ctx.props().path[..];
                let prefix_len = path.len();
                debug_assert!(
                    prefix_len < dest_path.len(),
                    "Got asked to move a node for a parent."
                );
                if prefix_len < src_path.len()
                    && path == &src_path[..prefix_len]
                    && path == &dest_path[..prefix_len]
                {
                    // This node is the common ancestor of the source and destination
                    // paths.
                    if let NodeKind::Group(group) = ctx.props().node.kind() {
                        if let Some(new_group) = graph_manipulation::move_child(
                            group,
                            &src_path[prefix_len..],
                            &dest_path[prefix_len..],
                        ) {
                            ctx.props().replace.emit((our_idx, new_group.into()));
                        }
                    } else {
                        warn!("Attempting to move nodes in a non-group.");
                    }
                } else {
                    // No common ancestor yet, ask parent to do the move.
                    ctx.props().move_node.emit((src_path, dest_path));
                }
                if self.insert_pos.is_some() {
                    self.insert_pos = None;
                    true
                } else {
                    false
                }
            }
            Msg::ChangeType { id } => {
                if let NodeKind::Building(building) = ctx.props().node.kind() {
                    if building.building != Some(id) {
                        let mut new_bldg = building.clone();
                        new_bldg.building = Some(id);
                        match db.get(id) {
                            Some(building) => {
                                new_bldg.settings =
                                    new_bldg.settings.build_new_settings(&building.kind);
                            }
                            None => warn!("New building ID is unknown."),
                        }
                        match new_bldg.build_node(&db) {
                            Ok(new_node) => ctx.props().replace.emit((our_idx, new_node)),
                            Err(e) => warn!("Unable to build node: {}", e),
                        }
                    }
                } else {
                    warn!("Cannot change building type id of a non-building");
                }
                false
            }
            Msg::ChangeRecipe { id } => {
                let building = match ctx.props().node.kind() {
                    NodeKind::Building(building) => building,
                    _ => {
                        warn!("Cannot change recipe id of a non-building");
                        return false;
                    }
                };
                if let Some(building_id) = building.building {
                    match db.get(building_id) {
                        Some(BuildingType {
                            kind: BuildingKind::Manufacturer(m),
                            ..
                        }) => {
                            if !m.available_recipes.contains(&id) {
                                warn!(
                                    "Recipe {} is not available for building {}",
                                    id, building_id
                                );
                                return false;
                            }
                        }
                        Some(_) => {
                            warn!("Cannot change recipe id, building is not a manufacturer");
                            return false;
                        }
                        None => {
                            warn!("Cannot change recipe id, unknown building");
                            return false;
                        }
                    }
                } else {
                    warn!("Cannot change recipe id, building not set");
                    return false;
                };
                let settings = ManufacturerSettings {
                    recipe: Some(id),
                    ..match &building.settings {
                        BuildingSettings::Manufacturer(ms) => ms.clone(),
                        settings => {
                            warn!("Had to change building settings kind, did not match building kind in db");
                            ManufacturerSettings {
                                clock_speed: settings.clock_speed(),
                                ..Default::default()
                            }
                        }
                    }
                }.into();
                let new_bldg = Building {
                    settings,
                    ..building.clone()
                };
                match new_bldg.build_node(&db) {
                    Ok(new_node) => ctx.props().replace.emit((our_idx, new_node)),
                    Err(e) => warn!("Unable to build node: {}", e),
                }
                false
            }
            Msg::ChangeItem { id } => {
                let building = match ctx.props().node.kind() {
                    NodeKind::Building(building) => building,
                    _ => {
                        warn!("Cannot change item id of a non-building");
                        return false;
                    }
                };
                let kind_id = if let Some(building_id) = building.building {
                    match db.get(building_id) {
                        Some(BuildingType {
                            kind: BuildingKind::Miner(m),
                            ..
                        }) => {
                            if !m.allowed_resources.contains(&id) {
                                warn!(
                                    "Resource {} is not available for building {}",
                                    id, building_id
                                );
                                return false;
                            }
                            BuildingKindId::Miner
                        }
                        Some(BuildingType {
                            kind: BuildingKind::Generator(g),
                            ..
                        }) => {
                            if !g.allowed_fuel.contains(&id) {
                                warn!("Fuel {} is not available for building {}", id, building_id);
                                return false;
                            }
                            BuildingKindId::Generator
                        }
                        Some(BuildingType {
                            kind: BuildingKind::Pump(p),
                            ..
                        }) => {
                            if !p.allowed_resources.contains(&id) {
                                warn!(
                                    "Resource {} is not available for building {}",
                                    id, building_id
                                );
                                return false;
                            }
                            BuildingKindId::Pump
                        }
                        Some(_) => {
                            warn!("Cannot change item id, building is not a miner, generator, or pump");
                            return false;
                        }
                        None => {
                            warn!("Cannot change recipe id, unknown building");
                            return false;
                        }
                    }
                } else {
                    warn!("Cannot change recipe id, building not set");
                    return false;
                };
                let settings = match (kind_id, &building.settings) {
                    (BuildingKindId::Miner, BuildingSettings::Miner(ms)) => MinerSettings {
                        resource: Some(id),
                        ..ms.clone()
                    }
                    .into(),
                    (BuildingKindId::Miner, settings) => {
                        warn!("Had to change building settings kind, did not match building kind in db");
                        MinerSettings {
                            resource: Some(id),
                            clock_speed: settings.clock_speed(),
                            ..Default::default()
                        }
                        .into()
                    }
                    (BuildingKindId::Generator, BuildingSettings::Generator(gs)) => {
                        GeneratorSettings {
                            fuel: Some(id),
                            ..gs.clone()
                        }
                        .into()
                    }
                    (BuildingKindId::Generator, settings) => {
                        warn!("Had to change building settings kind, did not match building kind in db");
                        GeneratorSettings {
                            fuel: Some(id),
                            clock_speed: settings.clock_speed(),
                            ..Default::default()
                        }
                        .into()
                    }
                    (BuildingKindId::Pump, BuildingSettings::Pump(ms)) => PumpSettings {
                        resource: Some(id),
                        ..ms.clone()
                    }
                    .into(),
                    (BuildingKindId::Pump, settings) => {
                        warn!("Had to change building settings kind, did not match building kind in db");
                        PumpSettings {
                            resource: Some(id),
                            clock_speed: settings.clock_speed(),
                            ..Default::default()
                        }
                        .into()
                    }
                    // We know the other BuidingKindId values are impossible because we
                    // only return these three from the previous match.
                    _ => unreachable!(),
                };
                let new_bldg = Building {
                    settings,
                    ..building.clone()
                };
                match new_bldg.build_node(&db) {
                    Ok(new_node) => ctx.props().replace.emit((our_idx, new_node)),
                    Err(e) => warn!("Unable to build node: {}", e),
                }

                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        match ctx.props().node.kind() {
            NodeKind::Group(group) => self.view_group(ctx, group),
            NodeKind::Building(building) => self.view_building(ctx, building),
        }
    }
}

/// CSS class that identifies children which identifies the `div` which marks where an
/// element will be dropped. Used to avoid having the insert point count towards the
/// index being chosen for insertion when searching children to figure out what index the
/// drop is at. Also used to style the insert point.
const DRAG_INSERT_POINT: &str = "drag-insert-point";

impl NodeDisplay {
    /// Creates the delete button, if the parent allows this node to be deleted.
    fn delete_button(&self, ctx: &Context<Self>) -> Html {
        match ctx.props().delete.clone() {
            Some(delete_from_parent) => {
                let idx = ctx
                    .props()
                    .path
                    .last()
                    .copied()
                    .expect("Parent provided a delete callback, but this is the root node.");
                let onclick = Callback::from(move |_| delete_from_parent.emit(idx));
                html! {
                    <button {onclick} class="delete" title="Delete">
                        <span class="material-icons">{"delete"}</span>
                    </button>
                }
            }
            None => html! {},
        }
    }

    /// Creates the copy button, if the parent allows this node to be copied.
    fn copy_button(&self, ctx: &Context<Self>) -> Html {
        match ctx.props().copy.clone() {
            Some(copy_from_parent) => {
                let idx = ctx
                    .props()
                    .path
                    .last()
                    .copied()
                    .expect("Parent provided a copy callback, but this is the root node.");
                let onclick = Callback::from(move |_| copy_from_parent.emit(idx));
                html! {
                    <button {onclick} class="copy" title="Copy">
                        <span class="material-icons">{"content_copy"}</span>
                    </button>
                }
            }
            None => html! {},
        }
    }
}

fn get_value_from_input_event(e: InputEvent) -> String {
    let event: Event = e.dyn_into().unwrap();
    let event_target = event.target().unwrap();
    let target: HtmlInputElement = event_target.dyn_into().unwrap();
    target.value()
}
