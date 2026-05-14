use std::collections::HashMap;

use macroquad::prelude::*;

use crate::helpers::resolution_ui_scale;
use crate::item::*;
use crate::map::TileSet;

pub const HOTBAR_SIZE: usize = 10;

const INVENTORY_WHEEL_SLOTS: usize = 15;
const HOTBAR_SLOT_SIZE: f32 = 52.0;
const HUD_SLOT_SCALE: f32 = 1.2;
const HOTBAR_MARGIN: f32 = 14.0;
const WHEEL_SLOT_SIZE: f32 = 64.0;
const WHEEL_CENTER_SLOT_SIZE: f32 = 82.0;
const WHEEL_RADIUS: f32 = 160.0;
const WHEEL_RING_THICKNESS: f32 = 85.0; // Configurable: thickness of the outer ring
const WHEEL_ITEM_SCALE: f32 = 0.9; // Configurable: 0.9x smaller items
const WHEEL_ITEM_ALIGNMENT: f32 = 0.5; // Configurable: 0.0=inner edge, 0.5=middle, 1.0=outer edge
const WHEEL_RING_OUTLINE: f32 = 5.0; // Configurable: thickness of the black outlines
const WHEEL_SPIN_SPEED: f32 = 0.28;

#[derive(Clone, Copy)]
struct WheelSlotLayout {
    item: ItemId,
    rect: Rect,
    key_index: Option<usize>,
}

struct InventoryLayout {
    hud_rect: Rect,
    wheel_rect: Option<Rect>,
    wheel_slots: Vec<WheelSlotLayout>,
}

pub struct StackInventory {
    counts: HashMap<ItemId, u32>,
    order: Vec<ItemId>,
    selected_item: Option<ItemId>,
    open: bool,
}

impl StackInventory {
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
            order: Vec::new(),
            selected_item: None,
            open: false,
        }
    }

    pub fn seed_demo(catalog: &InventoryCatalog) -> Self {
        let mut inventory = Self::new();
        for item in catalog.iter_ids() {
            let def = catalog.get(item);
            let amount = match def.category {
                ItemCategory::Resources => 120 + (item.0 as u32 * 17),
                ItemCategory::Materials => 80 + (item.0 as u32 * 13),
                ItemCategory::Tiles => 240 + ((item.0 as u32 % 9) * 96),
                ItemCategory::Utility => 12 + (item.0 as u32 * 3),
                ItemCategory::All => 0,
            };
            inventory.add(item, amount.max(1));
        }
        inventory.selected_item = inventory.priority_items().first().copied();
        inventory
    }

    pub fn add(&mut self, item: ItemId, amount: u32) {
        if amount == 0 {
            return;
        }
        let entry = self.counts.entry(item).or_insert(0);
        if *entry == 0 {
            self.order.push(item);
        }
        *entry = entry.saturating_add(amount);
        if self.selected_item.is_none() {
            self.selected_item = Some(item);
        }
    }

    pub fn remove(&mut self, item: ItemId, amount: u32) -> u32 {
        if amount == 0 {
            return 0;
        }
        let Some(existing) = self.counts.get_mut(&item) else {
            return 0;
        };
        let removed = (*existing).min(amount);
        *existing -= removed;
        if *existing == 0 {
            self.counts.remove(&item);
            self.order.retain(|candidate| *candidate != item);
            if self.selected_item == Some(item) {
                self.selected_item = self.priority_items().first().copied();
            }
        }
        removed
    }

    pub fn amount(&self, item: ItemId) -> u32 {
        self.counts.get(&item).copied().unwrap_or(0)
    }

    pub fn items(&self) -> Vec<(ItemId, u32)> {
        self.order
            .iter()
            .filter_map(|item| {
                let amount = self.amount(*item);
                if amount > 0 { Some((*item, amount)) } else { None }
            })
            .collect()
    }

    pub fn selected_item(&self) -> Option<ItemId> {
        self.selected_item
    }

    pub fn consume_selected_one(&mut self) -> Option<ItemId> {
        let selected = self.selected_item?;
        if self.amount(selected) == 0 {
            return None;
        }
        let removed = self.remove(selected, 1);
        if removed == 1 { Some(selected) } else { None }
    }

    pub fn transfer_one_to(&mut self, other: &mut StackInventory, item: ItemId) -> bool {
        if self.remove(item, 1) == 1 {
            other.add(item, 1);
            true
        } else {
            false
        }
    }

    pub fn captures_pointer(&self, mouse: Vec2, catalog: &InventoryCatalog) -> bool {
        let layout = self.layout(catalog);
        if layout.hud_rect.contains(mouse) {
            return true;
        }
        layout
            .wheel_rect
            .map(|rect| rect.contains(mouse))
            .unwrap_or(false)
    }

    pub fn handle_input(&mut self, catalog: &InventoryCatalog) {
        if is_key_pressed(KeyCode::Tab) || is_key_pressed(KeyCode::I) {
            self.open = !self.open;
        }

        for slot in 0..HOTBAR_SIZE {
            let key = match slot {
                0 => KeyCode::Key1,
                1 => KeyCode::Key2,
                2 => KeyCode::Key3,
                3 => KeyCode::Key4,
                4 => KeyCode::Key5,
                5 => KeyCode::Key6,
                6 => KeyCode::Key7,
                7 => KeyCode::Key8,
                8 => KeyCode::Key9,
                9 => KeyCode::Key0,
                _ => KeyCode::Unknown,
            };
            if is_key_pressed(key) {
                self.selected_item = self.priority_items().get(slot).copied();
            }
        }

        if is_key_pressed(KeyCode::Q) {
            self.cycle_selection(-1);
        }
        if is_key_pressed(KeyCode::E) {
            self.cycle_selection(1);
        }

        let mouse = vec2(mouse_position().0, mouse_position().1);
        let layout = self.layout(catalog);
        let (_, wheel_y) = mouse_wheel();
        let pointer_on_wheel = layout
            .wheel_rect
            .map(|rect| rect.contains(mouse))
            .unwrap_or(false);
        if wheel_y.abs() > f32::EPSILON && (!self.open || pointer_on_wheel) {
            self.cycle_selection(if wheel_y > 0.0 { -1 } else { 1 });
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            if layout.hud_rect.contains(mouse) {
                self.open = !self.open;
                return;
            }

            if self.open {
                for slot in &layout.wheel_slots {
                    if slot.rect.contains(mouse) {
                        self.selected_item = Some(slot.item);
                        self.open = false;
                        return;
                    }
                }
            }
        }

        if self.selected_item.is_none_or(|item| self.amount(item) == 0) {
            self.selected_item = self.priority_items().first().copied();
        }
    }

    pub fn draw(&self, catalog: &InventoryCatalog, tileset: &TileSet, hotbar_slot: &Texture2D) {
        let layout = self.layout(catalog);
        if self.open {
            self.draw_inventory_wheel(catalog, tileset, hotbar_slot, &layout);
        }
        self.draw_hud_slot(catalog, tileset, hotbar_slot, &layout);
    }

    fn draw_inventory_wheel(
        &self,
        catalog: &InventoryCatalog,
        tileset: &TileSet,
        hotbar_slot: &Texture2D,
        layout: &InventoryLayout,
    ) {
        let Some(wheel_rect) = layout.wheel_rect else {
            return;
        };
        let center = vec2(
            wheel_rect.x + wheel_rect.w * 0.5,
            wheel_rect.y + wheel_rect.h * 0.5,
        );
        let outer_ring_radius = WHEEL_RADIUS;
        let outer_ring_thickness = WHEEL_RING_THICKNESS;
        let outer_ring_center_radius =
            outer_ring_radius - outer_ring_thickness + WHEEL_RING_OUTLINE;
        let center_circle_radius = WHEEL_CENTER_SLOT_SIZE * 0.72;

        draw_circle(
            center.x,
            center.y,
            center_circle_radius,
            Color::new(0.0, 0.0, 0.0, 0.46),
        );
        draw_circle_lines(
            center.x,
            center.y,
            center_circle_radius,
            WHEEL_RING_OUTLINE,
            BLACK,
        );
        draw_circle_lines(
            center.x,
            center.y,
            outer_ring_center_radius,
            outer_ring_thickness - WHEEL_RING_OUTLINE,
            Color::new(0.0, 0.0, 0.0, 0.46),
        );
        draw_circle_lines(
            center.x,
            center.y,
            outer_ring_radius - outer_ring_thickness,
            WHEEL_RING_OUTLINE,
            BLACK,
        );
        draw_circle_lines(
            center.x,
            center.y,
            outer_ring_radius,
            WHEEL_RING_OUTLINE,
            BLACK,
        );

        let center_slot = Rect::new(
            center.x - WHEEL_CENTER_SLOT_SIZE * 0.5,
            center.y - WHEEL_CENTER_SLOT_SIZE * 0.5,
            WHEEL_CENTER_SLOT_SIZE,
            WHEEL_CENTER_SLOT_SIZE,
        );
        self.draw_slot_frame(hotbar_slot, center_slot, 1.0);
        if let Some(item) = self.selected_item {
            let def = catalog.get(item);
            draw_rectangle_lines(
                center_slot.x,
                center_slot.y,
                center_slot.w,
                center_slot.h,
                3.0,
                def.accent,
            );
            let icon_pad = 4.0;
            let icon_rect = Rect::new(
                center_slot.x + icon_pad,
                center_slot.y + icon_pad,
                center_slot.w - icon_pad * 2.0,
                center_slot.h - icon_pad * 2.0,
            );
            draw_item_icon(def, tileset, icon_rect);
            draw_stack_amount(center_slot, self.amount(item), 13, 0.72);
        }

        for slot in &layout.wheel_slots {
            let selected = self.selected_item == Some(slot.item);
            self.draw_slot_frame(hotbar_slot, slot.rect, if selected { 1.0 } else { 0.92 });
            if selected {
                draw_rectangle_lines(
                    slot.rect.x,
                    slot.rect.y,
                    slot.rect.w,
                    slot.rect.h,
                    3.0,
                    catalog.get(slot.item).accent,
                );
            }

            if let Some(key_index) = slot.key_index {
                draw_text_ex(
                    hotbar_key_label(key_index),
                    slot.rect.x + 4.0,
                    slot.rect.y + 12.0,
                    TextParams {
                        font_size: 12,
                        color: Color::from_hex(0xF6D98F),
                        ..Default::default()
                    },
                );
            }

            let def = catalog.get(slot.item);
            let icon_pad = if selected { 5.0 } else { 6.0 };
            let icon_rect = Rect::new(
                slot.rect.x + icon_pad,
                slot.rect.y + icon_pad,
                slot.rect.w - icon_pad * 2.0,
                slot.rect.h - icon_pad * 2.0,
            );
            draw_item_icon(def, tileset, icon_rect);
            draw_stack_amount(slot.rect, self.amount(slot.item), 13, 0.72);
        }
    }

    fn draw_hud_slot(
        &self,
        catalog: &InventoryCatalog,
        tileset: &TileSet,
        hotbar_slot: &Texture2D,
        layout: &InventoryLayout,
    ) {
        let rect = layout.hud_rect;
        self.draw_slot_frame(hotbar_slot, rect, 1.0);

        if let Some(item) = self.selected_item {
            let def = catalog.get(item);
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, def.accent);
            let icon_pad = 7.5;
            let icon_rect = Rect::new(
                rect.x + icon_pad,
                rect.y + icon_pad,
                rect.w - icon_pad * 2.0,
                rect.h - icon_pad * 2.0,
            );
            draw_item_icon(def, tileset, icon_rect);
            draw_stack_amount(rect, self.amount(item), 13, 0.64);
        }
    }

    fn draw_slot_frame(&self, hotbar_slot: &Texture2D, rect: Rect, alpha: f32) {
        draw_texture_ex(
            hotbar_slot,
            rect.x,
            rect.y,
            Color::new(1.0, 1.0, 1.0, alpha.clamp(0.0, 1.0)),
            DrawTextureParams {
                dest_size: Some(vec2(rect.w, rect.h)),
                ..Default::default()
            },
        );
    }

    fn layout(&self, catalog: &InventoryCatalog) -> InventoryLayout {
        let ui_scale = resolution_ui_scale();
        let hud_size = HOTBAR_SLOT_SIZE * HUD_SLOT_SCALE * ui_scale;
        let hud_margin = HOTBAR_MARGIN * ui_scale;
        let hud_rect = Rect::new(hud_margin, hud_margin, hud_size, hud_size);
        let mut wheel_rect = None;
        let mut wheel_slots = Vec::new();

        if self.open {
            let center = vec2(screen_width() * 0.5, screen_height() * 0.5);
            let wheel_size = (WHEEL_RADIUS + WHEEL_SLOT_SIZE * 0.75) * 2.0;
            wheel_rect = Some(Rect::new(
                center.x - wheel_size * 0.5,
                center.y - wheel_size * 0.5,
                wheel_size,
                wheel_size,
            ));

            let items = self.wheel_items(catalog);
            let priority = self.priority_items();
            let spin = get_time() as f32 * WHEEL_SPIN_SPEED;
            let count = items.len().max(1) as f32;
            let item_radius = WHEEL_RADIUS - WHEEL_RING_THICKNESS * (1.0 - WHEEL_ITEM_ALIGNMENT);
            let scaled_slot_size = WHEEL_SLOT_SIZE * WHEEL_ITEM_SCALE;
            for (idx, item) in items.into_iter().enumerate() {
                let angle =
                    spin + idx as f32 / count * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                let pos = center + vec2(angle.cos(), angle.sin()) * item_radius;
                wheel_slots.push(WheelSlotLayout {
                    item,
                    rect: Rect::new(
                        pos.x - scaled_slot_size * 0.5,
                        pos.y - scaled_slot_size * 0.5,
                        scaled_slot_size,
                        scaled_slot_size,
                    ),
                    key_index: priority.iter().position(|candidate| *candidate == item),
                });
            }
        }

        InventoryLayout {
            hud_rect,
            wheel_rect,
            wheel_slots,
        }
    }

    fn wheel_items(&self, catalog: &InventoryCatalog) -> Vec<ItemId> {
        let mut items = self.priority_items();
        if let Some(selected) = self.selected_item {
            if self.amount(selected) > 0 && !items.contains(&selected) {
                items.insert(0, selected);
            }
        }
        items
            .into_iter()
            .filter(|item| catalog.get(*item).category != ItemCategory::All)
            .take(INVENTORY_WHEEL_SLOTS)
            .collect()
    }

    fn priority_items(&self) -> Vec<ItemId> {
        let mut items: Vec<ItemId> = self
            .order
            .iter()
            .copied()
            .filter(|item| self.amount(*item) > 0)
            .collect();
        items.sort_by_key(|item| {
            let order = self
                .order
                .iter()
                .position(|candidate| candidate == item)
                .unwrap_or(usize::MAX);
            (self.amount(*item), order)
        });
        items
    }

    fn cycle_selection(&mut self, delta: i32) {
        let items = self.priority_items();
        if items.is_empty() {
            self.selected_item = None;
            return;
        }
        let current = self
            .selected_item
            .and_then(|item| items.iter().position(|candidate| *candidate == item))
            .unwrap_or(0);
        let len = items.len() as i32;
        let next = (current as i32 + delta).rem_euclid(len) as usize;
        self.selected_item = Some(items[next]);
    }
}

fn draw_stack_amount(rect: Rect, amount: u32, font_size: u16, alpha: f32) {
    let amount = format_stack_count(amount);
    let label_metrics = measure_text(&amount, None, font_size, 1.0);
    draw_rectangle(
        rect.x + rect.w - label_metrics.width - 9.0,
        rect.y + rect.h - 15.0,
        label_metrics.width + 6.0,
        12.0,
        Color::new(0.04, 0.025, 0.01, alpha),
    );
    draw_text_ex(
        &amount,
        rect.x + rect.w - label_metrics.width - 6.0,
        rect.y + rect.h - 5.0,
        TextParams {
            font_size,
            color: Color::from_hex(0xFFF0C7),
            ..Default::default()
        },
    );
}

fn hotbar_key_label(index: usize) -> &'static str {
    match index {
        0 => "1",
        1 => "2",
        2 => "3",
        3 => "4",
        4 => "5",
        5 => "6",
        6 => "7",
        7 => "8",
        8 => "9",
        9 => "0",
        _ => "",
    }
}
