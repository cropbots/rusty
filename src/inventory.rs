use std::array;
use std::collections::HashMap;

use macroquad::prelude::*;

use crate::map::TileSet;

pub const HOTBAR_SIZE: usize = 8;

const HOTBAR_SLOT_SIZE: f32 = 52.0;
const HOTBAR_GAP: f32 = 10.0;
const HOTBAR_MARGIN: f32 = 14.0;
const PANEL_MARGIN: f32 = 14.0;
const PANEL_HEADER_H: f32 = 36.0;
const PANEL_TAB_H: f32 = 30.0;
const PANEL_FOOTER_H: f32 = 32.0;
const ITEM_CARD_H: f32 = 42.0;
const ITEM_CARD_GAP: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemCategory {
    All,
    Resources,
    Materials,
    Tiles,
    Utility,
}

impl ItemCategory {
    pub const FILTERS: [ItemCategory; 5] = [
        ItemCategory::All,
        ItemCategory::Resources,
        ItemCategory::Materials,
        ItemCategory::Tiles,
        ItemCategory::Utility,
    ];

    fn label(self) -> &'static str {
        match self {
            ItemCategory::All => "All",
            ItemCategory::Resources => "Resources",
            ItemCategory::Materials => "Materials",
            ItemCategory::Tiles => "Tiles",
            ItemCategory::Utility => "Utility",
        }
    }
}

#[derive(Clone)]
pub enum ItemIcon {
    Tile(u8),
    Texture(Texture2D),
}

pub struct ItemDefinition {
    pub name: String,
    pub short_name: String,
    pub category: ItemCategory,
    pub icon: ItemIcon,
    pub accent: Color,
}

pub struct InventoryCatalog {
    items: Vec<ItemDefinition>,
}

impl InventoryCatalog {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn register(&mut self, def: ItemDefinition) -> ItemId {
        let id = ItemId(self.items.len());
        self.items.push(def);
        id
    }

    pub fn get(&self, id: ItemId) -> &ItemDefinition {
        &self.items[id.0]
    }

    pub fn demo(tileset_count: usize, gear: Texture2D, gear_outline: Texture2D) -> Self {
        let mut catalog = Self::new();
        let resources = [
            (
                "Copper Scrap",
                "Scrap",
                ItemCategory::Resources,
                Color::from_hex(0xCC7A4C),
            ),
            (
                "Fiber Bundle",
                "Fiber",
                ItemCategory::Resources,
                Color::from_hex(0x85B864),
            ),
            (
                "Resin Clump",
                "Resin",
                ItemCategory::Resources,
                Color::from_hex(0xD3B65F),
            ),
            (
                "Seed Pack",
                "Seeds",
                ItemCategory::Resources,
                Color::from_hex(0x77C2A8),
            ),
            (
                "Clay Lump",
                "Clay",
                ItemCategory::Materials,
                Color::from_hex(0x9D6B53),
            ),
            (
                "Stone Dust",
                "Dust",
                ItemCategory::Materials,
                Color::from_hex(0x8A8F99),
            ),
            (
                "Core Gear",
                "Gear",
                ItemCategory::Utility,
                Color::from_hex(0x7FB5E6),
            ),
            (
                "Spare Cog",
                "Cog",
                ItemCategory::Utility,
                Color::from_hex(0xB4C4CF),
            ),
        ];
        for (idx, (name, short, category, accent)) in resources.into_iter().enumerate() {
            let texture = if idx % 2 == 0 {
                gear.clone()
            } else {
                gear_outline.clone()
            };
            catalog.register(ItemDefinition {
                name: name.to_string(),
                short_name: short.to_string(),
                category,
                icon: ItemIcon::Texture(texture),
                accent,
            });
        }

        let tile_palette = [
            ("Soil", Color::from_hex(0x7C5A46)),
            ("Grass", Color::from_hex(0x6EAA4F)),
            ("Path", Color::from_hex(0xB89A6A)),
            ("Stone", Color::from_hex(0x8D949B)),
            ("Water", Color::from_hex(0x5A90D6)),
            ("Floor", Color::from_hex(0xC9B98A)),
        ];
        let tile_total = tileset_count.min(36);
        for tile_id in 0..tile_total {
            let (prefix, accent) = tile_palette[tile_id % tile_palette.len()];
            catalog.register(ItemDefinition {
                name: format!("{prefix} Tile {tile_id:02}"),
                short_name: format!("{prefix} {tile_id:02}"),
                category: ItemCategory::Tiles,
                icon: ItemIcon::Tile(tile_id as u8),
                accent,
            });
        }
        catalog
    }

    pub fn iter_ids(&self) -> impl Iterator<Item = ItemId> + '_ {
        (0..self.items.len()).map(ItemId)
    }
}

#[derive(Clone, Copy)]
struct ItemCardLayout {
    item: ItemId,
    rect: Rect,
}

struct InventoryLayout {
    hotbar_rect: Rect,
    hotbar_slots: [Rect; HOTBAR_SIZE],
    panel_rect: Option<Rect>,
    tab_rects: Vec<(ItemCategory, Rect)>,
    item_cards: Vec<ItemCardLayout>,
    list_rect: Option<Rect>,
}

pub struct StackInventory {
    counts: HashMap<ItemId, u32>,
    order: Vec<ItemId>,
    hotbar: [Option<ItemId>; HOTBAR_SIZE],
    selected_hotbar: usize,
    selected_filter: ItemCategory,
    selected_item: Option<ItemId>,
    scroll_rows: usize,
    open: bool,
}

impl StackInventory {
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
            order: Vec::new(),
            hotbar: [None; HOTBAR_SIZE],
            selected_hotbar: 0,
            selected_filter: ItemCategory::All,
            selected_item: None,
            scroll_rows: 0,
            open: false,
        }
    }

    fn bind_item_to_slot(&mut self, item: ItemId, slot: usize) {
        // Enforce uniqueness: if this item is in another slot, clear that slot
        for i in 0..HOTBAR_SIZE {
            if i != slot && self.hotbar[i] == Some(item) {
                self.hotbar[i] = None;
            }
        }
        self.hotbar[slot] = Some(item);
        self.selected_hotbar = slot;
        self.selected_item = Some(item);
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

        let default_hotbar: Vec<ItemId> = inventory
            .visible_items(catalog, ItemCategory::All)
            .into_iter()
            .take(HOTBAR_SIZE)
            .collect();
        for (slot, item) in default_hotbar.into_iter().enumerate() {
            inventory.hotbar[slot] = Some(item);
        }
        inventory.selected_item = inventory.hotbar[0];
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
            for slot in &mut self.hotbar {
                if *slot == Some(item) {
                    *slot = None;
                }
            }
            if self.selected_item == Some(item) {
                self.selected_item = self.selected_hotbar_item();
            }
        }
        removed
    }

    pub fn amount(&self, item: ItemId) -> u32 {
        self.counts.get(&item).copied().unwrap_or(0)
    }

    pub fn captures_pointer(&self, mouse: Vec2, catalog: &InventoryCatalog) -> bool {
        let layout = self.layout(catalog);
        if layout.hotbar_rect.contains(mouse) {
            return true;
        }
        layout
            .panel_rect
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
                _ => KeyCode::Key8,
            };
            if is_key_pressed(key) {
                self.selected_hotbar = slot;
                self.selected_item = self.hotbar[slot];
            }
        }

        if is_key_pressed(KeyCode::Q) {
            self.cycle_hotbar(-1);
        }
        if is_key_pressed(KeyCode::E) {
            self.cycle_hotbar(1);
        }

        let mouse = vec2(mouse_position().0, mouse_position().1);
        let layout = self.layout(catalog);
        let (_, wheel_y) = mouse_wheel();
        let pointer_on_panel = layout
            .panel_rect
            .map(|rect| rect.contains(mouse))
            .unwrap_or(false);

        if wheel_y.abs() > f32::EPSILON {
            if self.open && pointer_on_panel {
                let max_rows = self.max_scroll_rows(catalog, &layout);
                if wheel_y < 0.0 {
                    self.scroll_rows = (self.scroll_rows + 1).min(max_rows);
                } else {
                    self.scroll_rows = self.scroll_rows.saturating_sub(1);
                }
            } else {
                self.cycle_hotbar(if wheel_y > 0.0 { -1 } else { 1 });
            }
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            for (idx, rect) in layout.hotbar_slots.iter().enumerate() {
                if rect.contains(mouse) {
                    self.selected_hotbar = idx;
                    self.selected_item = self.hotbar[idx];
                    return;
                }
            }

            if self.open {
                for (category, rect) in &layout.tab_rects {
                    if rect.contains(mouse) {
                        self.selected_filter = *category;
                        self.scroll_rows = 0;
                        return;
                    }
                }

                for card in &layout.item_cards {
                    if card.rect.contains(mouse) {
                        self.bind_item_to_slot(card.item, self.selected_hotbar);
                        return;
                    }
                }
            }
        }

        if is_mouse_button_pressed(MouseButton::Right) && self.open {
            for card in &layout.item_cards {
                if card.rect.contains(mouse) {
                    // If already pinned, just select that slot
                    if let Some(existing_slot) = self.hotbar.iter().position(|&s| s == Some(card.item)) {
                        self.selected_hotbar = existing_slot;
                        self.selected_item = Some(card.item);
                    } else if let Some(empty_slot) = self.hotbar.iter().position(|entry| entry.is_none()) {
                        self.bind_item_to_slot(card.item, empty_slot);
                    } else {
                        self.bind_item_to_slot(card.item, self.selected_hotbar);
                    }
                    return;
                }
            }
        }

        self.sanitize_hotbar();
    }

    pub fn draw(&self, catalog: &InventoryCatalog, tileset: &TileSet) {
        let layout = self.layout(catalog);
        if let Some(panel_rect) = layout.panel_rect {
            self.draw_panel(catalog, tileset, panel_rect, &layout);
        }
        self.draw_hotbar(catalog, tileset, &layout);
    }

    fn draw_panel(
        &self,
        catalog: &InventoryCatalog,
        tileset: &TileSet,
        panel_rect: Rect,
        layout: &InventoryLayout,
    ) {
        draw_panel_shell(
            panel_rect,
            Color::from_hex(0x121922),
            Color::from_hex(0x29435C),
        );

        draw_text_ex(
            "Inventory",
            panel_rect.x + 14.0,
            panel_rect.y + 24.0,
            TextParams {
                font_size: 24,
                color: Color::from_hex(0xEEF4FA),
                ..Default::default()
            },
        );

        let distinct = self.order.len();
        let total: u64 = self.counts.values().map(|value| *value as u64).sum();
        let summary = format!("{distinct} stacks  |  {total} total");
        draw_text_ex(
            &summary,
            panel_rect.x + panel_rect.w - 170.0,
            panel_rect.y + 23.0,
            TextParams {
                font_size: 16,
                color: Color::from_hex(0x9FB2C5),
                ..Default::default()
            },
        );

        for (category, rect) in &layout.tab_rects {
            let active = *category == self.selected_filter;
            let fill = if active {
                Color::from_hex(0x385C7B)
            } else {
                Color::from_hex(0x1B2732)
            };
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                1.0,
                if active {
                    Color::from_hex(0x8FD3FF)
                } else {
                    Color::from_hex(0x355066)
                },
            );
            draw_centered_label(category.label(), *rect, 17, WHITE);
        }

        if let Some(list_rect) = layout.list_rect {
            draw_rectangle(
                list_rect.x,
                list_rect.y,
                list_rect.w,
                list_rect.h,
                Color::new(0.02, 0.04, 0.06, 0.18),
            );
        }

        for card in &layout.item_cards {
            let def = catalog.get(card.item);
            let amount = self.amount(card.item);
            let selected = self.selected_item == Some(card.item);
            let border = if selected {
                def.accent
            } else {
                Color::from_hex(0x2E475E)
            };
            let fill = if selected {
                Color::new(
                    def.accent.r * 0.25,
                    def.accent.g * 0.25,
                    def.accent.b * 0.25,
                    0.95,
                )
            } else {
                Color::from_hex(0x17212B)
            };
            
            let pinned = self.hotbar.iter().any(|&s| s == Some(card.item));
            
            draw_rectangle(card.rect.x, card.rect.y, card.rect.w, card.rect.h, fill);
            draw_rectangle_lines(
                card.rect.x,
                card.rect.y,
                card.rect.w,
                card.rect.h,
                1.0,
                if pinned && !selected { Color::from_hex(0x456078) } else { border },
            );

            if pinned {
                draw_rectangle(
                    card.rect.x + card.rect.w - 6.0,
                    card.rect.y + 4.0,
                    2.0,
                    card.rect.h - 8.0,
                    def.accent,
                );
            }

            let icon_rect = Rect::new(card.rect.x + 6.0, card.rect.y + 5.0, 30.0, 30.0);
            draw_icon_frame(icon_rect, def.accent);
            draw_item_icon(def, tileset, icon_rect);

            draw_text_ex(
                &fit_label(&def.short_name, 14),
                card.rect.x + 44.0,
                card.rect.y + 18.0,
                TextParams {
                    font_size: 17,
                    color: if pinned { Color::from_hex(0xEDF4FA) } else { Color::from_hex(0xCED9E3) },
                    ..Default::default()
                },
            );
            draw_text_ex(
                &format_stack_count(amount),
                card.rect.x + 44.0,
                card.rect.y + 33.0,
                TextParams {
                    font_size: 15,
                    color: if selected { Color::from_hex(0xC2D1E0) } else { Color::from_hex(0x8A9FB2) },
                    ..Default::default()
                },
            );
        }

        let footer = Rect::new(
            panel_rect.x + 10.0,
            panel_rect.y + panel_rect.h - PANEL_FOOTER_H - 8.0,
            panel_rect.w - 20.0,
            PANEL_FOOTER_H,
        );
        draw_rectangle(
            footer.x,
            footer.y,
            footer.w,
            footer.h,
            Color::from_hex(0x151D24),
        );
        draw_rectangle_lines(
            footer.x,
            footer.y,
            footer.w,
            footer.h,
            1.0,
            Color::from_hex(0x304455),
        );
        let footer_text = if let Some(item) = self.selected_item {
            let def = catalog.get(item);
            format!(
                "{}  |  LMB: Bind to slot  |  RMB: Quick Pin",
                def.name
            )
        } else {
            "Select a stack to pin it to the hotbar".to_string()
        };
        draw_text_ex(
            &fit_label(&footer_text, 64),
            footer.x + 10.0,
            footer.y + 21.0,
            TextParams {
                font_size: 15,
                color: Color::from_hex(0x9FB2C5),
                ..Default::default()
            },
        );
    }

    fn draw_hotbar(&self, catalog: &InventoryCatalog, tileset: &TileSet, layout: &InventoryLayout) {
        let bar = layout.hotbar_rect;
        draw_panel_shell(bar, Color::from_hex(0x111820), Color::from_hex(0x2E495F));

        for (idx, rect) in layout.hotbar_slots.iter().enumerate() {
            let selected = idx == self.selected_hotbar;
            let item = self.hotbar[idx];
            let accent = item
                .map(|id| catalog.get(id).accent)
                .unwrap_or(Color::from_hex(0x4D6374));
            let fill = if selected {
                Color::new(accent.r * 0.3, accent.g * 0.3, accent.b * 0.3, 1.0)
            } else {
                Color::from_hex(0x1B232B)
            };
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                1.0,
                if selected {
                    accent
                } else {
                    Color::from_hex(0x40505D)
                },
            );

            draw_text_ex(
                &(idx + 1).to_string(),
                rect.x + 4.0,
                rect.y + 12.0,
                TextParams {
                    font_size: 13,
                    color: Color::from_hex(0x8DA4B8),
                    ..Default::default()
                },
            );

            if let Some(item) = item {
                let def = catalog.get(item);
                let icon_rect =
                    Rect::new(rect.x + 8.0, rect.y + 10.0, rect.w - 16.0, rect.h - 20.0);
                draw_item_icon(def, tileset, icon_rect);
                let amount = format_stack_count(self.amount(item));
                let label_metrics = measure_text(&amount, None, 16, 1.0);
                draw_rectangle(
                    rect.x + rect.w - label_metrics.width - 12.0,
                    rect.y + rect.h - 18.0,
                    label_metrics.width + 8.0,
                    14.0,
                    Color::new(0.03, 0.05, 0.07, 0.78),
                );
                draw_text_ex(
                    &amount,
                    rect.x + rect.w - label_metrics.width - 8.0,
                    rect.y + rect.h - 6.0,
                    TextParams {
                        font_size: 16,
                        color: WHITE,
                        ..Default::default()
                    },
                );
            }
        }

        if let Some(item) = self.selected_hotbar_item() {
            let def = catalog.get(item);
            let label = format!("{}  |  {}", def.name, format_stack_count(self.amount(item)));
            draw_text_ex(
                &fit_label(&label, 34),
                bar.x + 12.0,
                bar.y - 10.0,
                TextParams {
                    font_size: 18,
                    color: Color::from_hex(0xEDF4FA),
                    ..Default::default()
                },
            );
        }
    }

    fn layout(&self, catalog: &InventoryCatalog) -> InventoryLayout {
        let hotbar_w =
            HOTBAR_SIZE as f32 * HOTBAR_SLOT_SIZE + (HOTBAR_SIZE as f32 - 1.0) * HOTBAR_GAP + 20.0;
        let hotbar_h = HOTBAR_SLOT_SIZE + 18.0;
        let hotbar_rect = Rect::new(
            (screen_width() - hotbar_w) * 0.5,
            screen_height() - hotbar_h - HOTBAR_MARGIN,
            hotbar_w,
            hotbar_h,
        );
        let hotbar_slots = array::from_fn(|idx| {
            Rect::new(
                hotbar_rect.x + 10.0 + idx as f32 * (HOTBAR_SLOT_SIZE + HOTBAR_GAP),
                hotbar_rect.y + 9.0,
                HOTBAR_SLOT_SIZE,
                HOTBAR_SLOT_SIZE,
            )
        });

        let mut panel_rect = None;
        let mut tab_rects = Vec::new();
        let mut item_cards = Vec::new();
        let mut list_rect = None;

        if self.open {
            let panel_w = screen_width().min(520.0) - PANEL_MARGIN * 2.0;
            let panel_h = screen_height().min(430.0) - PANEL_MARGIN * 2.0;
            let x = PANEL_MARGIN;
            let y = (hotbar_rect.y - panel_h - 10.0).max(PANEL_MARGIN);
            let rect = Rect::new(x, y, panel_w.max(320.0), panel_h.max(240.0));
            panel_rect = Some(rect);

            let tab_y = rect.y + PANEL_HEADER_H + 10.0;
            let tab_w = (rect.w - 20.0 - (ItemCategory::FILTERS.len() as f32 - 1.0) * 6.0)
                / ItemCategory::FILTERS.len() as f32;
            for (idx, category) in ItemCategory::FILTERS.iter().enumerate() {
                tab_rects.push((
                    *category,
                    Rect::new(
                        rect.x + 10.0 + idx as f32 * (tab_w + 6.0),
                        tab_y,
                        tab_w,
                        PANEL_TAB_H,
                    ),
                ));
            }

            let content_top = tab_y + PANEL_TAB_H + 10.0;
            let content_bottom = rect.y + rect.h - PANEL_FOOTER_H - 18.0;
            let content_rect = Rect::new(
                rect.x + 10.0,
                content_top,
                rect.w - 20.0,
                content_bottom - content_top,
            );
            list_rect = Some(content_rect);

            let columns = if content_rect.w >= 400.0 { 2 } else { 1 };
            let card_w = (content_rect.w - (columns as f32 - 1.0) * ITEM_CARD_GAP) / columns as f32;
            let row_step = ITEM_CARD_H + ITEM_CARD_GAP;
            let filtered = self.visible_items(catalog, self.selected_filter);
            let max_visible_rows = (content_rect.h / row_step).floor().max(1.0) as usize;
            let start_index = self.scroll_rows * columns;

            for (visible_idx, item) in filtered.into_iter().skip(start_index).enumerate() {
                let row = visible_idx / columns;
                if row >= max_visible_rows {
                    break;
                }
                let col = visible_idx % columns;
                item_cards.push(ItemCardLayout {
                    item,
                    rect: Rect::new(
                        content_rect.x + col as f32 * (card_w + ITEM_CARD_GAP),
                        content_rect.y + row as f32 * row_step,
                        card_w,
                        ITEM_CARD_H,
                    ),
                });
            }
        }

        InventoryLayout {
            hotbar_rect,
            hotbar_slots,
            panel_rect,
            tab_rects,
            item_cards,
            list_rect,
        }
    }

    fn visible_items(&self, catalog: &InventoryCatalog, filter: ItemCategory) -> Vec<ItemId> {
        self.order
            .iter()
            .copied()
            .filter(|item| self.amount(*item) > 0)
            .filter(|item| filter == ItemCategory::All || catalog.get(*item).category == filter)
            .collect()
    }

    fn max_scroll_rows(&self, catalog: &InventoryCatalog, layout: &InventoryLayout) -> usize {
        let Some(list_rect) = layout.list_rect else {
            return 0;
        };
        let columns = if list_rect.w >= 400.0 { 2 } else { 1 };
        let row_step = ITEM_CARD_H + ITEM_CARD_GAP;
        let visible_rows = (list_rect.h / row_step).floor().max(1.0) as usize;
        let total_items = self.visible_items(catalog, self.selected_filter).len();
        total_items
            .saturating_add(columns - 1)
            .saturating_div(columns)
            .saturating_sub(visible_rows)
    }

    fn cycle_hotbar(&mut self, delta: i32) {
        let len = HOTBAR_SIZE as i32;
        self.selected_hotbar = (self.selected_hotbar as i32 + delta).rem_euclid(len) as usize;
        self.selected_item = self.hotbar[self.selected_hotbar];
    }

    fn selected_hotbar_item(&self) -> Option<ItemId> {
        self.hotbar[self.selected_hotbar]
    }

    fn sanitize_hotbar(&mut self) {
        for slot in &mut self.hotbar {
            let remove = match *slot {
                Some(item) => self.counts.get(&item).copied().unwrap_or(0) == 0,
                None => false,
            };
            if remove {
                *slot = None;
            }
        }
        if self
            .selected_item
            .is_some_and(|item| self.amount(item) == 0)
        {
            self.selected_item = self.selected_hotbar_item();
        }
    }
}

fn draw_panel_shell(rect: Rect, fill: Color, stroke: Color) {
    // Drop shadow
    draw_rectangle(
        rect.x + 4.0,
        rect.y + 4.0,
        rect.w,
        rect.h,
        Color::new(0.0, 0.0, 0.0, 0.4),
    );
    // Main fill
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
    // Bevel/glow inner highlight
    draw_rectangle_lines(
        rect.x + 1.0,
        rect.y + 1.0,
        rect.w - 2.0,
        rect.h - 2.0,
        1.0,
        Color::new(1.0, 1.0, 1.0, 0.05),
    );
    // Main stroke
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        2.0,
        stroke,
    );
}

fn draw_centered_label(label: &str, rect: Rect, font_size: u16, color: Color) {
    let metrics = measure_text(label, None, font_size, 1.0);
    let x = rect.x + (rect.w - metrics.width) * 0.5;
    let y = rect.y + rect.h * 0.5 + metrics.offset_y.abs() * 0.25 + font_size as f32 * 0.33;
    draw_text_ex(
        label,
        x,
        y,
        TextParams {
            font_size,
            color,
            ..Default::default()
        },
    );
}

fn draw_icon_frame(rect: Rect, accent: Color) {
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        Color::new(accent.r * 0.18, accent.g * 0.18, accent.b * 0.18, 0.95),
    );
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, accent);
}

fn draw_item_icon(def: &ItemDefinition, tileset: &TileSet, rect: Rect) {
    match &def.icon {
        ItemIcon::Tile(tile) => {
            if let Some(source) = tileset.tile_rect(*tile) {
                draw_texture_ex(
                    tileset.texture(),
                    rect.x,
                    rect.y,
                    WHITE,
                    DrawTextureParams {
                        source: Some(source),
                        dest_size: Some(vec2(rect.w, rect.h)),
                        ..Default::default()
                    },
                );
            }
        }
        ItemIcon::Texture(texture) => {
            draw_texture_ex(
                texture,
                rect.x,
                rect.y,
                def.accent,
                DrawTextureParams {
                    dest_size: Some(vec2(rect.w, rect.h)),
                    ..Default::default()
                },
            );
        }
    }
}

fn format_stack_count(amount: u32) -> String {
    match amount {
        0..=999 => amount.to_string(),
        1_000..=999_999 => format!("{:.1}k", amount as f32 / 1_000.0),
        _ => format!("{:.1}m", amount as f32 / 1_000_000.0),
    }
}

fn fit_label(label: &str, max_chars: usize) -> String {
    let mut count = 0usize;
    let mut out = String::new();
    for ch in label.chars() {
        if count >= max_chars {
            out.push_str("...");
            return out;
        }
        out.push(ch);
        count += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_catalog() -> InventoryCatalog {
        let mut catalog = InventoryCatalog::new();
        catalog.register(ItemDefinition {
            name: "Stone".to_string(),
            short_name: "Stone".to_string(),
            category: ItemCategory::Materials,
            icon: ItemIcon::Tile(0),
            accent: WHITE,
        });
        catalog.register(ItemDefinition {
            name: "Grass".to_string(),
            short_name: "Grass".to_string(),
            category: ItemCategory::Tiles,
            icon: ItemIcon::Tile(1),
            accent: GREEN,
        });
        catalog
    }

    #[test]
    fn merges_stack_counts_by_item() {
        let catalog = test_catalog();
        let mut inventory = StackInventory::new();
        let item = catalog.iter_ids().next().unwrap();
        inventory.add(item, 12);
        inventory.add(item, 8);
        assert_eq!(inventory.amount(item), 20);
        assert_eq!(inventory.order, vec![item]);
    }

    #[test]
    fn removing_last_item_clears_hotbar_binding() {
        let catalog = test_catalog();
        let mut inventory = StackInventory::new();
        let item = catalog.iter_ids().next().unwrap();
        inventory.add(item, 5);
        inventory.hotbar[0] = Some(item);
        let removed = inventory.remove(item, 5);
        assert_eq!(removed, 5);
        assert_eq!(inventory.amount(item), 0);
        assert_eq!(inventory.hotbar[0], None);
    }

    #[test]
    fn hotbar_enforces_uniqueness() {
        let catalog = test_catalog();
        let mut inventory = StackInventory::new();
        let item = catalog.iter_ids().next().unwrap();
        inventory.add(item, 10);
        
        // Bind to slot 0
        inventory.bind_item_to_slot(item, 0);
        assert_eq!(inventory.hotbar[0], Some(item));
        
        // Bind to slot 1
        inventory.bind_item_to_slot(item, 1);
        assert_eq!(inventory.hotbar[1], Some(item));
        assert_eq!(inventory.hotbar[0], None, "Item should have been removed from slot 0");
    }
}
