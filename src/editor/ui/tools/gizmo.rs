use bevy::{prelude::*, render::camera::CameraProjection};
use bevy_egui::egui::{self, Key};
use egui_gizmo::*;

use crate::{
    editor::core::{EditorTool, Selected},
    EditorCameraMarker,
};

pub struct GizmoTool {
    pub gizmo_mode: GizmoMode,
}

impl Default for GizmoTool {
    fn default() -> Self {
        Self {
            gizmo_mode: GizmoMode::Translate,
        }
    }
}

impl EditorTool for GizmoTool {
    fn name(&self) -> &str {
        "Gizmo"
    }

    fn ui(&mut self, ui: &mut egui::Ui, world: &mut World) {
        // GIZMO DRAW
        // Draw gizmo per entity to individual move
        // If SHIFT pressed draw "mean" gizmo to move all selected entities together

        let mode2name = vec![
            (GizmoMode::Translate, "Translate"),
            (GizmoMode::Rotate, "Rotate"),
            (GizmoMode::Scale, "Scale"),
        ];

        for (mode, name) in mode2name {
            if self.gizmo_mode == mode {
                ui.button(egui::RichText::new(name).strong()).clicked();
            } else if ui.button(name).clicked() {
                self.gizmo_mode = mode;
            }
        }

        if ui.ui_contains_pointer() && !ui.ctx().wants_keyboard_input() {
            //hot keys. Blender keys preffer
            let mode2key = vec![
                (GizmoMode::Translate, Key::G),
                (GizmoMode::Rotate, Key::R),
                (GizmoMode::Scale, Key::S),
            ];

            for (mode, key) in mode2key {
                if ui.input(|s| s.key_pressed(key)) {
                    self.gizmo_mode = mode;
                }
            }
        }

        let (cam_transform, cam_proj) = {
            let mut cam_query =
                world.query_filtered::<(&GlobalTransform, &Projection), With<EditorCameraMarker>>();
            let Ok((ref_tr, ref_cam)) = cam_query.get_single(world) else {
                return;
            };
            (*ref_tr, ref_cam.clone())
        };

        let selected = world
            .query_filtered::<Entity, With<Selected>>()
            .iter(world)
            .collect::<Vec<_>>();
        let mut disable_pan_orbit = false;
        let _gizmo_mode = GizmoMode::Translate;

        unsafe {
            let cell = world.as_unsafe_world_cell();

            let view_matrix = Mat4::from(cam_transform.affine().inverse());
            if ui.input(|s| s.modifiers.shift) {
                let mut mean_transform = Transform::IDENTITY;
                for e in &selected {
                    let Some(ecell) = cell.get_entity(*e) else {
                        continue;
                    };
                    let Some(global_transform) = ecell.get_mut::<GlobalTransform>() else {
                        continue;
                    };
                    let tr = global_transform.compute_transform();
                    mean_transform.translation += tr.translation;
                    mean_transform.scale += tr.scale;
                }
                mean_transform.translation /= selected.len() as f32;
                mean_transform.scale /= selected.len() as f32;

                let mut global_mean = GlobalTransform::from(mean_transform);

                let mut loc_transform = vec![];
                for e in &selected {
                    let Some(ecell) = cell.get_entity(*e) else {
                        continue;
                    };
                    let Some(global_transform) = ecell.get_mut::<GlobalTransform>() else {
                        continue;
                    };
                    loc_transform.push(global_transform.reparented_to(&global_mean));
                }

                if let Some(result) =
                    egui_gizmo::Gizmo::new("Selected gizmo mean global".to_string())
                        .projection_matrix(cam_proj.get_projection_matrix().to_cols_array_2d())
                        .view_matrix(view_matrix.to_cols_array_2d())
                        .model_matrix(mean_transform.compute_matrix().to_cols_array_2d())
                        .mode(self.gizmo_mode)
                        .interact(ui)
                {
                    mean_transform = Transform {
                        translation: Vec3::from(<[f32; 3]>::from(result.translation)),
                        rotation: Quat::from_array(<[f32; 4]>::from(result.rotation)),
                        scale: Vec3::from(<[f32; 3]>::from(result.scale)),
                    };
                    disable_pan_orbit = true;
                }

                global_mean = GlobalTransform::from(mean_transform);

                for (idx, e) in selected.iter().enumerate() {
                    let Some(ecell) = cell.get_entity(*e) else {
                        continue;
                    };
                    let Some(mut transform) = ecell.get_mut::<Transform>() else {
                        continue;
                    };

                    let new_global = global_mean.mul_transform(loc_transform[idx]);

                    if let Some(parent) = ecell.get::<Parent>() {
                        if let Some(parent) = cell.get_entity(parent.get()) {
                            if let Some(parent_global) = parent.get::<GlobalTransform>() {
                                *transform = new_global.reparented_to(parent_global);
                            }
                        }
                    } else {
                        *transform = new_global.compute_transform();
                    }
                }
            } else {
                for e in &selected {
                    let Some(ecell) = cell.get_entity(*e) else {
                        continue;
                    };
                    let Some(mut transform) = ecell.get_mut::<Transform>() else {
                        continue;
                    };
                    if let Some(parent) = ecell.get::<Parent>() {
                        if let Some(parent) = cell.get_entity(parent.get()) {
                            if let Some(parent_global) = parent.get::<GlobalTransform>() {
                                if let Some(global) = ecell.get::<GlobalTransform>() {
                                    if let Some(result) = egui_gizmo::Gizmo::new(
                                                format!("Selected gizmo {:?}", *e))
                                                .projection_matrix(
                                                    cam_proj.get_projection_matrix().to_cols_array_2d(),
                                                )
                                                .view_matrix(view_matrix.to_cols_array_2d())
                                                .model_matrix(
                                                    global.compute_matrix().to_cols_array_2d(),
                                                )
                                                .mode(self.gizmo_mode)
                                                .interact(ui) {
                                            let new_transform = Transform {
                                            translation: Vec3::from(<[f32; 3]>::from(
                                                result.translation,
                                            )),
                                            rotation: Quat::from_array(<[f32; 4]>::from(
                                                result.rotation,
                                            )),
                                            scale: Vec3::from(<[f32; 3]>::from(result.scale)),
                                        };

                                        let new_transform = GlobalTransform::from(new_transform);
                                        *transform = new_transform.reparented_to(parent_global);
                                        transform.set_changed();
                                        disable_pan_orbit = true;
                                    }
                                    continue;
                                }
                            }
                        }
                    }
                    if let Some(result) = egui_gizmo::Gizmo::new(format!("Selected gizmo {:?}", *e))
                            .projection_matrix(cam_proj.get_projection_matrix().to_cols_array_2d())
                            .view_matrix(view_matrix.to_cols_array_2d())
                            .model_matrix(transform.compute_matrix().to_cols_array_2d())
                            .mode(self.gizmo_mode)
                            .interact(ui) {
                        *transform = Transform {
                            translation: Vec3::from(<[f32; 3]>::from(result.translation)),
                            rotation: Quat::from_array(<[f32; 4]>::from(result.rotation)),
                            scale: Vec3::from(<[f32; 3]>::from(result.scale)),
                        };
                        transform.set_changed();
                        disable_pan_orbit = true;
                    }
                }
            }

            if disable_pan_orbit {
                cell.get_resource_mut::<crate::editor::PanOrbitEnabled>()
                    .unwrap()
                    .0 = false;
            }
        }
    }
}
