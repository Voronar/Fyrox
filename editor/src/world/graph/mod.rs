use crate::{
    load_image,
    message::MessageSender,
    scene::{
        commands::{
            graph::{AddModelCommand, LinkNodesCommand},
            ChangeSelectionCommand, CommandGroup, GameSceneCommand,
        },
        GameScene, Selection,
    },
    world::{graph::selection::GraphSelection, WorldViewerDataProvider},
};
use fyrox::{
    asset::{manager::ResourceManager, untyped::UntypedResource},
    core::{
        algebra::Vector3,
        futures::executor::block_on,
        make_relative_path,
        pool::{ErasedHandle, Handle},
    },
    graph::SceneGraph,
    resource::model::{Model, ModelResourceExtension},
    scene::{node::Node, Scene},
};
use std::{borrow::Cow, path::Path, path::PathBuf};

pub mod item;
pub mod menu;
pub mod selection;

pub struct EditorSceneWrapper<'a> {
    pub selection: &'a Selection,
    pub game_scene: &'a GameScene,
    pub scene: &'a mut Scene,
    pub path: Option<&'a Path>,
    pub sender: &'a MessageSender,
    pub resource_manager: &'a ResourceManager,
    pub instantiation_scale: Vector3<f32>,
}

impl<'a> WorldViewerDataProvider for EditorSceneWrapper<'a> {
    fn root_node(&self) -> ErasedHandle {
        self.game_scene.scene_content_root.into()
    }

    fn path(&self) -> Option<&Path> {
        self.path
    }

    fn children_of(&self, node: ErasedHandle) -> Vec<ErasedHandle> {
        self.scene
            .graph
            .try_get(node.into())
            .map(|n| {
                n.children()
                    .iter()
                    .map(|h| ErasedHandle::from(*h))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn child_count_of(&self, node: ErasedHandle) -> usize {
        self.scene
            .graph
            .try_get(node.into())
            .map_or(0, |node| node.children().len())
    }

    fn is_node_has_child(&self, node: ErasedHandle, child: ErasedHandle) -> bool {
        self.scene
            .graph
            .try_get(node.into())
            .map_or(false, |node| node.children().contains(&child.into()))
    }

    fn parent_of(&self, node: ErasedHandle) -> ErasedHandle {
        self.scene
            .graph
            .try_get(node.into())
            .map(|node| node.parent().into())
            .unwrap_or_default()
    }

    fn name_of(&self, node: ErasedHandle) -> Option<Cow<str>> {
        self.scene
            .graph
            .try_get(node.into())
            .map(|n| Cow::Borrowed(n.name()))
    }

    fn is_valid_handle(&self, node: ErasedHandle) -> bool {
        self.scene.graph.is_valid_handle(node.into())
    }

    fn icon_of(&self, node: ErasedHandle) -> Option<UntypedResource> {
        let node = self.scene.graph.try_get(node.into()).unwrap();
        if node.is_point_light() || node.is_directional_light() || node.is_spot_light() {
            load_image(include_bytes!("../../../resources/light.png"))
        } else if node.is_joint() || node.is_joint2d() {
            load_image(include_bytes!("../../../resources/joint.png"))
        } else if node.is_rigid_body() || node.is_rigid_body2d() {
            load_image(include_bytes!("../../../resources/rigid_body.png"))
        } else if node.is_collider() || node.is_collider2d() {
            load_image(include_bytes!("../../../resources/collider.png"))
        } else if node.is_sound() {
            load_image(include_bytes!("../../../resources/sound_source.png"))
        } else {
            load_image(include_bytes!("../../../resources/cube.png"))
        }
    }

    fn is_instance(&self, node: ErasedHandle) -> bool {
        self.scene
            .graph
            .try_get(node.into())
            .map_or(false, |n| n.resource().is_some())
    }

    fn selection(&self) -> Vec<ErasedHandle> {
        if let Selection::Graph(ref graph_selection) = self.selection {
            graph_selection
                .nodes
                .iter()
                .map(|h| ErasedHandle::from(*h))
                .collect::<Vec<_>>()
        } else {
            Default::default()
        }
    }

    fn on_change_hierarchy_request(&self, child: ErasedHandle, parent: ErasedHandle) {
        let child: Handle<Node> = child.into();
        let parent: Handle<Node> = parent.into();

        if let Selection::Graph(ref selection) = self.selection {
            if selection.nodes.contains(&child) {
                let mut commands = Vec::new();

                for &node_handle in selection.nodes.iter() {
                    // Make sure we won't create any loops - child must not have parent in its
                    // descendants.
                    let mut attach = true;
                    let mut p = parent;
                    while p.is_some() {
                        if p == node_handle {
                            attach = false;
                            break;
                        }
                        p = self.scene.graph[p].parent();
                    }

                    if attach {
                        commands.push(GameSceneCommand::new(LinkNodesCommand::new(
                            node_handle,
                            parent,
                        )));
                    }
                }

                if !commands.is_empty() {
                    self.sender.do_scene_command(CommandGroup::from(commands));
                }
            }
        }
    }

    fn on_asset_dropped(&mut self, path: PathBuf, node: ErasedHandle) {
        if let Ok(relative_path) = make_relative_path(path) {
            if let Some(model) = self
                .resource_manager
                .try_request::<Model>(relative_path)
                .and_then(|m| block_on(m).ok())
            {
                // Instantiate the model.
                let instance = model.instantiate(self.scene);

                self.scene.graph[instance]
                    .local_transform_mut()
                    .set_scale(self.instantiation_scale);

                let sub_graph = self.scene.graph.take_reserve_sub_graph(instance);

                let group = vec![
                    GameSceneCommand::new(AddModelCommand::new(sub_graph)),
                    GameSceneCommand::new(LinkNodesCommand::new(instance, node.into())),
                    GameSceneCommand::new(ChangeSelectionCommand::new(
                        Selection::Graph(GraphSelection::single_or_empty(instance)),
                        self.selection.clone(),
                    )),
                ];

                self.sender.do_scene_command(CommandGroup::from(group));
            }
        }
    }

    fn validate(&self) -> Vec<(ErasedHandle, Result<(), String>)> {
        self.scene
            .graph
            .pair_iter()
            .map(|(handle, node)| (handle.into(), node.validate(self.scene)))
            .collect()
    }

    fn on_selection_changed(&self, selection: &[ErasedHandle]) {
        let mut new_selection = Selection::None;
        for &selected_item in selection {
            match new_selection {
                Selection::None => {
                    new_selection =
                        Selection::Graph(GraphSelection::single_or_empty(selected_item.into()));
                }
                Selection::Graph(ref mut selection) => {
                    selection.insert_or_exclude(selected_item.into())
                }
                _ => (),
            }
        }

        if &new_selection != self.selection {
            self.sender.do_scene_command(ChangeSelectionCommand::new(
                new_selection,
                self.selection.clone(),
            ));
        }
    }
}
