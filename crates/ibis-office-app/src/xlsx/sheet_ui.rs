// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) ibis inc.
// Additional terms under AGPL-3.0 section 7(b): see NOTICE.
//! The Excel-like parts of the window for a workbook: the formula bar with
//! the name box, the ジャンプ dialog, the row and column headers, the
//! selected cell and the sheet tabs, and moving about the cells.
//!
//! They are drawn in the editor only; the worksheet itself (and what is
//! exported) is drawn by `sheet_view.rs`.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use ibis_doc_model::{CellRange, SlideId, cell_name, sheet_command::Axis};

use crate::shell::icons::Material;
use crate::shell::editor::{SlideEditor, SlideLayout, Tool, WorkspaceContent, sheet_of};

actions!(
    ibis_office,
    [
        /// Open the ジャンプ dialog (Excel's Go To).
        GoTo,
        /// Open テーブルの作成 (Excel's ⌘T).
        InsertTable,
        /// Select the cell at the edge of the data region above (Excel's Ctrl+↑).
        JumpCellUp,
        /// Select the cell at the edge of the data region below.
        JumpCellDown,
        /// Select the cell at the edge of the data region to the left.
        JumpCellLeft,
        /// Select the cell at the edge of the data region to the right.
        JumpCellRight,
        /// Stretch the selected range to the edge of the data region above.
        ExtendJumpUp,
        /// Stretch the selected range to the edge of the data region below.
        ExtendJumpDown,
        /// Stretch the selected range to the edge of the data region to the left.
        ExtendJumpLeft,
        /// Stretch the selected range to the edge of the data region to the right.
        ExtendJumpRight,
        /// Select the cell in column A of the selected row (Excel's Home).
        SelectRowStart,
        /// Stretch the selected range to column A.
        ExtendRowStart,
        /// Select A1, or the top-left cell past frozen panes (Excel's Ctrl+Home).
        SelectFirstCell,
        /// Stretch the selected range to that cell.
        ExtendFirstCell,
        /// Select the last cell in use (Excel's Ctrl+End).
        SelectLastCell,
        /// Stretch the selected range to the last cell in use.
        ExtendLastCell,
        /// Edit the selected cell's content (Excel's F2).
        EditCell,
        /// Backspace: a character while typing, else the selected cells' values.
        CellBackspace,
        /// Delete: a character while typing, else the selected cells' values.
        CellDelete,
        /// Bold on or off for the selected cells (⌘B).
        CellBold,
        /// Italic on or off (⌘I).
        CellItalic,
        /// Underline on or off (⌘U).
        CellUnderline,
        /// Copy the selected cells (⌘C).
        CellCopy,
        /// Cut the selected cells (⌘X).
        CellCut,
        /// Paste at the selected cell (⌘V).
        CellPaste,
        /// Paste values only (⇧⌘V).
        CellPastePlain,
        /// Take the formats off the selected cells (⌘\\).
        ClearCellFormats,
        /// Tab: finish the entry and go right (remembering where the Tabs began).
        CellTab,
        /// Select the whole sheet (⌘A).
        SelectAllCells,
        /// Open the link panel (⌘K).
        InsertLink,
        /// F4: cycle the reference while typing a formula.
        CycleReference,
        /// Enter: finish the entry and go down (to where the Tabs began, after Tabs).
        CellEnter,
        /// ⌘1 (⌃1): the cell format settings (セルの書式設定).
        OpenCellFormat,
        /// ⌃⇧L (⌘⇧F on a Mac): the filter ▼ on or off.
        ToggleFilter,
        /// ⌘D (⌃D): copy the selection's first row down over it.
        FillDown,
        /// ⌘R (⌃R): copy the selection's first column right over it.
        FillRight,
        /// ⌃; today's date into the cell (Excel's current date).
        InsertDate,
        /// ⌘; (⌃⇧; on Windows) the time now into the cell.
        InsertTime,
        /// ⌃Space: the columns of the selection, whole.
        SelectColumns,
        /// ⇧Space: the rows of the selection, whole.
        SelectRows,
        /// ⌃Enter: put the entry into every selected cell (and the ranges
        /// 条件を選択してジャンプ selected with them), staying put.
        CellFillSelection,
        /// ⌥Enter (Excel's Alt+Enter): a line break in what is being typed.
        CellLineBreak,
        /// ⌃E: フラッシュ フィル.
        FlashFill,
        /// Hide the selected rows (⌃9), as Excel.
        HideRows,
        /// Hide the selected columns (⌃0).
        HideColumns,
        /// Show the hidden rows in the selection (⌃⇧9).
        UnhideRows,
        /// Show the hidden columns in the selection (⌃⇧0).
        UnhideColumns,
        /// F9: calculate all formula cells and data tables in the workbook.
        CalculateNow,
        /// Shift+F9: calculate active worksheet.
        CalculateSheet,
    ]
);

/// Height of the column headers (A, B, C…), in pixels.
pub(crate) const COLUMN_HEADER_HEIGHT: f32 = 20.0;
/// Width of the row headers (1, 2, 3…), in pixels.
pub(crate) const ROW_HEADER_WIDTH: f32 = 44.0;
/// Room kept for the workspace's scrollbars when a cell is scrolled into view.
pub(super) const SCROLLBAR_ROOM: f32 = 14.0;

/// The tint over the selected range (Excel's translucent grey).
const RANGE_TINT: u32 = 0x2020201a;
/// Excel's green, for the selected cell and sheet tab.
const SELECTION_COLOR: u32 = 0x107c41;
/// The line along the edge of frozen panes.
pub(crate) const FROZEN_LINE: u32 = 0x9b9b9b;
/// Page break preview: the grey over the cells outside the print area (a
/// translucent grey, so their contents still show, as in Excel) and the
/// blue line around the print area.
const OUTSIDE_PRINT_AREA: u32 = 0x5f5f5f99;
const PRINT_AREA_LINE: u32 = 0x2f5ce0;
/// Width of the line around the print area, in pixels.
const PRINT_AREA_LINE_WIDTH: f32 = 2.0;
/// The headers' background, lines and labels.
const HEADER_BACKGROUND: u32 = 0xf3f3f3;
const HEADER_LINE: u32 = 0xd4d4d4;
const HEADER_TEXT: u32 = 0x444444;
/// The headers of the selected rows and columns.
const HEADER_SELECTED: u32 = 0xd3e7d9;
/// Size of the labels in the headers, in pixels.
const HEADER_FONT_SIZE: f32 = 11.0;

/// The letters of a column, such as `AB`.
pub(crate) fn column_letters(column: u32) -> String {
    cell_name(column, 0).trim_end_matches(|c: char| c.is_ascii_digit()).to_string()
}

impl SlideEditor {
    /// The name box and the content of the selected cell: its formula
    /// (with `=`) if it has one, otherwise its value as displayed. The
    /// content is read-only; a reference typed in the name box is selected.
    pub(crate) fn render_formula_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_name_box(window, cx);
        if self.wb().jump.pending_scroll && self.workspace_bounds.is_some() {
            // Jumped before the workspace was laid out.
            self.wb_mut().jump.pending_scroll = false;
            self.scroll_range_into_view(true);
        }
        let theme = cx.theme();
        let editing_text = self.cell_edit().filter(|e| e.slide == self.current_slide).map(|e| e.text.clone());
        // While typed into, the bar shows the caret and the selection as the cell does.
        let bar_rect = self.sheet_state().map(|s| s.bar_bounds.clone());
        let editing_parts = self.cell_edit().filter(|e| e.slide == self.current_slide).map(|e| e.bar_parts());
        let content = match (self.selection, self.current_sheet(), self.current_cell()) {
            _ if editing_text.is_some() => editing_text.clone().unwrap_or_default(),
            // 数式を表示しない on a protected sheet: nothing.
            (None, Some(_), Some(_)) if self.formula_hidden_here() => String::new(),
            // A cell of a data table shows its TABLE formula, as Excel's bar does.
            (None, Some((_, sheet)), Some((column, row))) if sheet.data_tables.iter().any(|t| (t.range.first_row..=t.range.last_row).contains(&row) && (t.range.first_column..=t.range.last_column).contains(&column)) => {
                let t = sheet.data_tables.iter().find(|t| (t.range.first_row..=t.range.last_row).contains(&row) && (t.range.first_column..=t.range.last_column).contains(&column)).unwrap();
                let name = |c: Option<(u32, u32)>| c.map(|(r, c)| ibis_doc_model::cell_name(c, r)).unwrap_or_default();
                format!("{{=TABLE({},{})}}", name(t.row_input), name(t.column_input))
            }
            (None, Some((_, sheet)), Some((column, row))) => {
                let content = sheet.cell(row, column).map(|cell| match &cell.formula {
                    Some(formula) => format!("={formula}"),
                    // A number as it is (a date or time as shown), as Excel's bar shows it.
                    None => match &cell.value {
                        ibis_doc_model::CellValue::Number(n) if !ibis_calc::numfmt::parsed(&sheet.styles[cell.style as usize].number_format).first().is_some_and(|s| s.is_date()) => ibis_calc::eval::number_to_text(*n),
                        _ => cell.text.clone(),
                    },
                });
                // One line, as the bar is one line high.
                content.unwrap_or_default().replace(['\r', '\n'], " ")
            }
            (Some(shape_id), _, _) => {
                let formula = self.slide().shape_formulas.get(&shape_id);
                match formula {
                    Some(f) => if f.starts_with('=') { f.clone() } else { format!("={f}") },
                    None => String::new(),
                }
            }
            _ if !self.selected_shapes().is_empty() => {
                let shapes = self.selected_shapes();
                let formula = shapes.first().and_then(|s| self.slide().shape_formulas.get(&s.id));
                match formula {
                    Some(f) => if f.starts_with('=') { f.clone() } else { format!("={f}") },
                    None => String::new(),
                }
            }
            _ => String::new(),
        };
        let field = || {
            div()
                .h(px(22.))
                .px_2()
                .flex()
                .items_center()
                .border_1()
                .border_color(theme.border)
                .rounded_sm()
                .bg(theme.background)
        };
        h_flex()
            .h(px(30.))
            .flex_shrink_0()
            .px_2()
            .gap_2()
            .items_center()
            .text_sm()
            .bg(theme.title_bar)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .w(px(96.))
                    .flex_shrink_0()
                    .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                        this.wb_mut().jump.select_on_release = !this.wb().jump.name_box.read(cx).focus_handle(cx).is_focused(window);
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            if std::mem::take(&mut this.wb_mut().jump.select_on_release) {
                                this.wb().jump.name_box.update(cx, |input, cx| input.select_all(window, cx));
                            }
                        }),
                    )
                    .child(Input::new(&self.wb().jump.name_box).small()),
            )
            .child(div().px_1().italic().text_color(theme.muted_foreground).child("fx"))
            .child(
                field()
                    .flex_1()
                    .min_w_0()
                    // A press on the bar types into the selected cell, as in Excel (the keys go to the cell).
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, e: &MouseDownEvent, window, cx| {
                            if this.cell_edit().is_none() && this.selection.is_none() && this.current_cell().is_some() {
                                this.begin_cell_edit(super::cell_edit::EditMode::Edit, true, cx);
                            }
                            // The caret goes where the bar was pressed.
                            if this.cell_edit().is_some() {
                                this.press_in_bar_text(e.position, e.click_count, e.modifiers.shift, window, cx);
                            }
                            this.focus_handle.focus(window, cx);
                        }),
                    )
                    .when_some(bar_rect, |d, rect| d.child(canvas(|_, _, _| {}, move |bounds, _, _, _| rect.set(Some(bounds))).absolute().size_full()))
                    .relative()
                    .child(match editing_parts {
                        Some((before, selected, after, caret_first)) => {
                            let caret = || div().w(px(1.)).h(px(14.)).flex_shrink_0().bg(theme.foreground);
                            h_flex()
                                .truncate()
                                .child(SharedString::from(before))
                                .when(caret_first || selected.is_empty(), |d| d.child(caret()))
                                .when(!selected.is_empty(), |d| d.child(div().bg(rgba(0x3390ff55)).child(SharedString::from(selected.clone()))))
                                .when(!caret_first && !selected.is_empty(), |d| d.child(caret()))
                                .child(SharedString::from(after))
                                .into_any_element()
                        }
                        None => div().truncate().child(SharedString::from(content)).into_any_element(),
                    }),
            )
            // Over the sheet, under the name box.
            .when(self.wb().jump.goto_open, |d| d.relative().child(deferred(self.render_goto(cx)).with_priority(1)))
            .when(self.wb().table_dialog.is_some(), |d| d.relative().children(self.render_table_dialog(cx)))
    }

    /// The tabs of the worksheets along the bottom, as in Excel.
    pub(crate) fn render_sheet_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let green = rgb(SELECTION_COLOR);
        let (background, border, muted, hover) = (theme.background, theme.border, theme.muted_foreground, theme.muted);
        let tabs = self.presentation().slides.iter().enumerate().map(|(index, slide)| {
            let id = slide.id;
            let current = id == self.current_slide;
            let name = slide.name.clone().unwrap_or_else(|| format!("Sheet{}", index + 1));
            // The sheet's tab colour: the tab filled with it, or (the tab
            // shown) a bar of it under the name, as Excel draws them.
            let tab_color = sheet_of(slide).and_then(|(_, sheet)| sheet.tab_color);
            let tab_fill = tab_color.map(|c| rgb(c.0));
            let on_fill = tab_color.map(|c| {
                let luma = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
                if luma > 140.0 { rgb(0x000000) } else { rgb(0xffffff) }
            });
            div()
                .id(("sheet-tab", id.0))
                .relative()
                .flex()
                .flex_shrink_0()
                .items_center()
                .h_full()
                .px_4()
                .border_r_1()
                .border_color(border)
                .cursor_pointer()
                .map(|d| {
                    if current {
                        let bar = tab_fill.unwrap_or(green);
                        d.bg(background).text_color(green).font_weight(FontWeight::SEMIBOLD).child(
                            div().absolute().left_0().right_0().bottom_0().h(px(if tab_fill.is_some() { 4. } else { 2. })).bg(bar),
                        )
                    } else if let (Some(fill), Some(text)) = (tab_fill, on_fill) {
                        d.bg(fill).text_color(text).hover(move |s| s.opacity(0.85))
                    } else {
                        d.text_color(muted).hover(move |s| s.bg(hover))
                    }
                })
                // A tab dragged onto another moves there (as in Excel).
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, _| {
                    if let Some(s) = this.sheet_state_mut() {
                        s.tab_drag = Some(id);
                    }
                }))
                .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, window, cx| this.on_tab_right_down(id, e, window, cx)))
                .on_mouse_up(MouseButton::Left, cx.listener(move |this, _: &MouseUpEvent, window, cx| {
                    let Some(from) = this.sheet_state_mut().and_then(|s| s.tab_drag.take()) else { return };
                    if from == id {
                        return;
                    }
                    if this.workbook_protection_refuses(cx) {
                        return;
                    }
                    let Some(wb) = this.presentation().workbook.clone() else { return };
                    let Some(to) = wb.sheets.iter().position(|s| s.slide == Some(id)) else { return };
                    if this.current_slide != from {
                        this.select_slide(from, window, cx);
                    }
                    let slide = this.current_slide;
                    if let Err(err) = this.document.execute(vec![ibis_doc_model::Command::Sheet(ibis_doc_model::SheetCommand::MoveSheet { slide, to })]) {
                        log::warn!("moving the sheet rejected: {err}");
                    }
                    cx.notify();
                }))
                .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                    if event.click_count() >= 2 {
                        return this.begin_sheet_rename(id, window, cx);
                    }
                    this.select_slide(id, window, cx);
                    this.focus_handle.focus(window, cx);
                }))
                .map(|d| match self.wb().jump.renaming == Some(id) {
                    true => d.child(div().w(px(140.)).child(Input::new(&self.wb().jump.tab_rename).small())),
                    false => d.child(SharedString::from(name)),
                })
        });
        // At the left, as Excel: ◀ ▶ scroll the tabs, ≡ lists the sheets.
        let scroll = self.wb().tab_scroll.clone();
        let button = |id: &'static str, icon: &'static str| {
            div()
                .id(id)
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .w(px(28.))
                .h_full()
                .cursor_pointer()
                .text_color(muted)
                .hover(move |s| s.bg(hover))
                .child(Icon::new(Material(icon)).size(px(18.)))
        };
        let step = {
            let scroll = scroll.clone();
            move |direction: f32, cx: &mut Context<Self>| {
                let (offset, max) = (scroll.offset(), scroll.max_offset().x);
                let x = (f32::from(offset.x) - direction * 120.0).clamp(-f32::from(max), 0.0);
                scroll.set_offset(point(px(x), offset.y));
                cx.notify();
            }
        };
        let (left_step, right_step) = (step.clone(), step);
        let left = button("sheet-tabs-left", "arrow_left").on_click(cx.listener(move |_, _, _, cx| left_step(-1.0, cx)));
        let right = button("sheet-tabs-right", "arrow_right").on_click(cx.listener(move |_, _, _, cx| right_step(1.0, cx)));
        let make_add = |id: &'static str| {
            button(id, "add").on_click(cx.listener(|this, _, window, cx| {
                this.insert_sheet(window, cx);
                // The new tab shows.
                if let Some(index) = this.presentation().slide_index(this.current_slide) {
                    this.wb().tab_scroll.scroll_to_item(index);
                }
                this.focus_handle.focus(window, cx);
            }))
        };
        let editor = cx.entity().downgrade();
        let sheets: Vec<(usize, SlideId, String, bool)> = self
            .presentation()
            .slides
            .iter()
            .enumerate()
            .map(|(i, slide)| (i, slide.id, slide.name.clone().unwrap_or_else(|| format!("Sheet{}", i + 1)), slide.id == self.current_slide))
            .collect();
        let list_scroll = scroll.clone();
        let list = Button::new("sheet-list")
            .ghost()
            .small()
            .icon(Icon::new(Material("menu")).size(px(18.)))
            .w(px(28.))
            .h_full()
            .flex_shrink_0()
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |menu, _, _| {
                sheets.iter().fold(menu.min_w(px(160.)), |menu, (index, id, name, current)| {
                    let (editor, scroll, id, index) = (editor.clone(), list_scroll.clone(), *id, *index);
                    menu.item(PopupMenuItem::new(SharedString::from(name.clone())).checked(*current).on_click(move |_, window, cx| {
                        editor
                            .update(cx, |this, cx| {
                                this.select_slide(id, window, cx);
                                this.focus_handle.focus(window, cx);
                                scroll.scroll_to_item(index);
                            })
                            .ok();
                    }))
                })
            });

        // Determine whether the + button is placed inline (right of the last sheet tab)
        // or fixed on the right edge of the scroll view.
        let slide_count = self.presentation().slides.len();
        let last_tab_bounds = if slide_count > 0 {
            scroll.bounds_for_item(slide_count - 1)
        } else {
            None
        };
        let scroll_bounds = scroll.bounds();

        let is_plus_fixed = if let (Some(last_tab), true) = (last_tab_bounds, scroll_bounds.size.width > px(0.)) {
            // Right edge of the last tab from the content origin.
            let tabs_right = last_tab.right() - scroll_bounds.left() - scroll.offset().x;
            // Check if inline + button (28px) fits within the scroll viewport width.
            let fits_inline = (tabs_right + px(28.)) <= scroll_bounds.size.width;
            !fits_inline || scroll.max_offset().x > px(0.5) || scroll.offset().x < px(-0.5)
        } else {
            // Fallback for initial render before bounds are laid out.
            slide_count > 14
        };

        let mut tab_elements: Vec<AnyElement> = tabs.map(|t| t.into_any_element()).collect();
        if !is_plus_fixed {
            tab_elements.push(make_add("sheet-add").into_any_element());
        }

        // Soft shadows at the ends the tabs scroll towards.
        let (offset, max) = (f32::from(scroll.offset().x), f32::from(scroll.max_offset().x));
        let (more_left, more_right) = (offset < -0.5, offset > -max + 0.5 && max > 0.5);
        let shadow = |from_left: bool| {
            let (dark, clear) = (hsla(0.0, 0.0, 0.0, 0.22), hsla(0.0, 0.0, 0.0, 0.0));
            let (a, b) = if from_left { (dark, clear) } else { (clear, dark) };
            div().absolute().top_0().bottom_0().w(px(18.)).map(|d| if from_left { d.left_0() } else { d.right_0() }).bg(linear_gradient(90., linear_color_stop(a, 0.), linear_color_stop(b, 1.)))
        };
        h_flex()
            .id("sheet-tabs")
            .h(px(30.))
            .flex_shrink_0()
            .text_sm()
            .bg(theme.title_bar)
            .border_t_1()
            .border_color(border)
            .child(left)
            .child(right)
            .child(list)
            .child(div().w(px(1.)).h_full().flex_shrink_0().bg(border))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .id("sheet-tab-scroll")
                            .size_full()
                            .overflow_x_scroll()
                            .track_scroll(&scroll)
                            .children(tab_elements),
                    )
                    .when(more_left, |d| d.child(shadow(true)))
                    .when(more_right, |d| d.child(shadow(false))),
            )
            .when(is_plus_fixed, |d| {
                d.child(div().w(px(1.)).h_full().flex_shrink_0().bg(border))
                    .child(make_add("sheet-add"))
            })
    }

    /// The row and column headers, laid over the top and left edges of the
    /// workspace. They follow the sheet as it scrolls sideways (the columns)
    /// or up and down (the rows), and highlight the selected cell's.
    pub(crate) fn render_sheet_headers(&self, content: WorkspaceContent, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (shape, sheet) = self.current_sheet()?;
        if !sheet.headings {
            return None;
        }
        let scale = content.scale;
        let to_px = |origin: Pixels, at: i64, offset: i64| f32::from(origin) + (at + offset) as f32 * scale;
        let xs: Vec<f32> = sheet.column_offsets().into_iter().map(|o| to_px(content.slide_origin.x, shape.frame.x, o)).collect();
        let ys: Vec<f32> = sheet.row_offsets().into_iter().map(|o| to_px(content.slide_origin.y, shape.frame.y, o)).collect();
        let range = self.selected_range().filter(|_| self.selection.is_none());
        let scroll = self.workspace_scroll.clone();
        let font = font(cx.theme().font_family.clone());
        let (head_w, head_h) = (px(ROW_HEADER_WIDTH), px(COLUMN_HEADER_HEIGHT));
        // Frozen columns and rows stay put; the others scroll past them.
        let (frozen_columns, frozen_rows) = sheet.frozen.unwrap_or((0, 0));
        let split = self.frozen_split(content).unwrap_or((None, None));
        // Split panes (表示 > 分割) scroll on their own, all the lines in each.
        let (pane_shift, all_lines) = (self.pane_shift(), self.split_panes().is_some());
        let (split_x, split_y) = (split.0.map_or(head_w, px), split.1.map_or(head_h, px));
        let geometry = self.wb().header_geometry.clone();
        let paint_geometry = geometry.clone();
        let drag = self.wb().header_drag;

        let painter = move |bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App| {
            let offset = scroll.offset();
            let o = bounds.origin;
            let geometry = paint_geometry.clone();
            *geometry.borrow_mut() = super::headers::HeaderGeometry { scale, origin: (f32::from(o.x), f32::from(o.y)), ..Default::default() };
            let (background, line, text, selected, green) =
                (rgb(HEADER_BACKGROUND), rgb(HEADER_LINE), rgb(HEADER_TEXT), rgb(HEADER_SELECTED), rgb(SELECTION_COLOR));
            let label = |window: &mut Window, cx: &mut App, s: String, color: Rgba, x: Pixels, y: Pixels, w: Pixels, h: Pixels| {
                let run = TextRun {
                    len: s.len(),
                    font: font.clone(),
                    color: color.into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let shaped = window.text_system().shape_line(s.into(), px(HEADER_FONT_SIZE), &[run], None);
                if w < px(1.) || h < px(1.) {
                    return;
                }
                // A label that fits is centred; one that does not starts at
                // the header's left and is cut off at its edges.
                let fits = shaped.width <= w - px(2.);
                let origin = point(if fits { x + (w - shaped.width) / 2.0 } else { x + px(1.) }, y);
                let cell = Bounds::new(point(x, y), size(w, h));
                window.with_content_mask(Some(ContentMask { bounds: cell }), |window| {
                    shaped.paint(origin, h, TextAlign::Left, None, window, cx).ok();
                });
            };

            // Column headers: the frozen ones unscrolled, then the others
            // scrolled and clipped to the right of them.
            window.paint_quad(fill(Bounds::new(o, size(bounds.size.width, head_h)), background));
            let columns = |window: &mut Window, cx: &mut App, strip: Bounds<Pixels>, shift: Pixels, frozen: bool| {
                window.with_content_mask(Some(ContentMask { bounds: strip }), |window| {
                    for (column, pair) in xs.windows(2).enumerate() {
                        if !all_lines && (column < frozen_columns as usize) != frozen {
                            continue;
                        }
                        let (left, right) = (o.x + shift + px(pair[0]), o.x + shift + px(pair[1]));
                        if right >= strip.left() && left <= strip.right() {
                            geometry.borrow_mut().columns.push((column as u32, f32::from(left), f32::from(right)));
                        }
                        if right <= strip.left() || right - left < px(0.5) {
                            continue;
                        }
                        if left >= strip.right() {
                            break;
                        }
                        let is_selected =
                            range.is_some_and(|r| (r.first_column..=r.last_column).contains(&(column as u32)));
                        let cell = Bounds::new(point(left, o.y), size(right - left, head_h));
                        if is_selected {
                            window.paint_quad(fill(cell, selected));
                            window.paint_quad(fill(Bounds::new(point(left, o.y + head_h - px(2.)), size(right - left, px(2.))), green));
                        }
                        window.paint_quad(fill(Bounds::new(point(right - px(0.5), o.y), size(px(1.), head_h)), line));
                        let color = if is_selected { green } else { text };
                        label(window, cx, column_letters(column as u32), color, left, o.y, right - left, head_h);
                    }
                });
            };
            let frozen_strip = Bounds::new(point(o.x + head_w, o.y), size(split_x - head_w, head_h));
            columns(window, cx, frozen_strip, pane_shift.x, true);
            let strip = Bounds::new(point(o.x + split_x, o.y), size(bounds.size.width - split_x, head_h));
            columns(window, cx, strip, offset.x, false);
            window.paint_quad(fill(Bounds::new(point(o.x, o.y + head_h - px(1.)), size(bounds.size.width, px(1.))), line));

            // Row headers, likewise.
            window.paint_quad(fill(Bounds::new(point(o.x, o.y + head_h), size(head_w, bounds.size.height - head_h)), background));
            let rows = |window: &mut Window, cx: &mut App, strip: Bounds<Pixels>, shift: Pixels, frozen: bool| {
                window.with_content_mask(Some(ContentMask { bounds: strip }), |window| {
                    for (row, pair) in ys.windows(2).enumerate() {
                        if !all_lines && (row < frozen_rows as usize) != frozen {
                            continue;
                        }
                        let (top, bottom) = (o.y + shift + px(pair[0]), o.y + shift + px(pair[1]));
                        if bottom >= strip.top() && top <= strip.bottom() {
                            geometry.borrow_mut().rows.push((row as u32, f32::from(top), f32::from(bottom)));
                        }
                        if bottom <= strip.top() || bottom - top < px(0.5) {
                            continue;
                        }
                        if top >= strip.bottom() {
                            break;
                        }
                        let is_selected = range.is_some_and(|r| (r.first_row..=r.last_row).contains(&(row as u32)));
                        let cell = Bounds::new(point(o.x, top), size(head_w, bottom - top));
                        if is_selected {
                            window.paint_quad(fill(cell, selected));
                            window.paint_quad(fill(Bounds::new(point(o.x + head_w - px(2.), top), size(px(2.), bottom - top)), green));
                        }
                        window.paint_quad(fill(Bounds::new(point(o.x, bottom - px(0.5)), size(head_w, px(1.))), line));
                        let color = if is_selected { green } else { text };
                        label(window, cx, (row + 1).to_string(), color, o.x, top, head_w, bottom - top);
                    }
                });
            };
            let frozen_strip = Bounds::new(point(o.x, o.y + head_h), size(head_w, split_y - head_h));
            rows(window, cx, frozen_strip, pane_shift.y, true);
            let strip = Bounds::new(point(o.x, o.y + split_y), size(head_w, bounds.size.height - split_y));
            rows(window, cx, strip, offset.y, false);
            window.paint_quad(fill(Bounds::new(point(o.x + head_w - px(1.), o.y), size(px(1.), bounds.size.height)), line));

            // The corner where they meet.
            window.paint_quad(fill(Bounds::new(o, size(head_w - px(1.), head_h - px(1.))), background));

            // A line being dragged: where the column or row will end, across the sheet.
            if let Some(d) = drag {
                let at = px(d.current.max(d.start));
                let guide = match d.axis {
                    Axis::Columns => Bounds::new(point(at, o.y), size(px(1.), bounds.size.height)),
                    Axis::Rows => Bounds::new(point(o.x, at), size(bounds.size.width, px(1.))),
                };
                window.paint_quad(fill(guide, green));
            }
        };
        // The header's menu, open at the pointer.
        let popup = self.wb().header_popup.as_ref().map(|p| deferred(anchored().position(p.position).snap_to_window_with_margin(px(8.)).child(p.menu.clone())).with_priority(2));
        // The resize pointer while a line is dragged or the pointer is on one.
        let on_line = |axis: Axis| self.wb().header_drag.is_some_and(|d| d.axis == axis) || self.wb().header_hover == Some(axis);
        let (drag_columns, drag_rows) = (on_line(Axis::Columns), on_line(Axis::Rows));

        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(canvas(|_, _, _| {}, move |bounds, _, window, cx| painter(bounds, window, cx)).absolute().size_full())
                // Dragging a header's line or along the headers goes on over the sheet.
                .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, window, cx| this.on_header_move(e, window, cx)))
                .on_mouse_up(MouseButton::Left, cx.listener(|this, e: &MouseUpEvent, _, cx| {
                    this.on_header_up(cx);
                    this.finish_fill(e.modifiers.alt, cx);
                    this.finish_format_paint(cx);
                    if let Some((column, row)) = this.cell_under(e.position) {
                        this.finish_pointing(column, row, cx);
                    }
                }))
                .on_mouse_up_out(MouseButton::Left, cx.listener(|this, e: &MouseUpEvent, _, cx| {
                    this.on_header_up(cx);
                    this.finish_fill(e.modifiers.alt, cx);
                }))
                // The headers take the pointer from the sheet under them,
                // but let the wheel through to scroll it.
                .child(
                    div()
                        .id("column-headers")
                        .absolute()
                        .top_0()
                        .left(head_w)
                        .right_0()
                        .h(head_h)
                        .block_mouse_except_scroll()
                        .when(drag_columns, |d| d.cursor(CursorStyle::ResizeLeftRight))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, window, cx| this.on_header_down(Axis::Columns, e, window, cx)))
                        // The strip hides the workspace under it from the pointer: moves over it too.
                        .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, window, cx| this.on_header_move(e, window, cx)))
                        .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| this.on_header_up(cx)))
                        .on_mouse_down(MouseButton::Right, cx.listener(|this, e: &MouseDownEvent, window, cx| this.on_header_right_down(Axis::Columns, e, window, cx))),
                )
                .child(
                    div()
                        .id("row-headers")
                        .absolute()
                        .top(head_h)
                        .left_0()
                        .bottom_0()
                        .w(head_w)
                        .block_mouse_except_scroll()
                        .when(drag_rows, |d| d.cursor(CursorStyle::ResizeUpDown))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, window, cx| this.on_header_down(Axis::Rows, e, window, cx)))
                        // The strip hides the workspace under it from the pointer: moves over it too.
                        .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, window, cx| this.on_header_move(e, window, cx)))
                        .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| this.on_header_up(cx)))
                        .on_mouse_down(MouseButton::Right, cx.listener(|this, e: &MouseDownEvent, window, cx| this.on_header_right_down(Axis::Rows, e, window, cx))),
                )
                // The corner still takes the pointer.
                .child(div().id("header-corner").absolute().top_0().left_0().w(head_w).h(head_h).block_mouse_except_scroll())
                .children(self.filter_buttons(cx))
                .children(self.pivot_buttons(cx))
                .children(popup)
                .children(self.wb().list_popup.as_ref().map(|p| deferred(anchored().position(p.position).snap_to_window_with_margin(px(8.)).child(p.menu.clone())).with_priority(2)))
                .children(self.render_filter_panel(cx))
                .children(self.render_pivot_popup(cx))
                .children(self.render_completion(cx))
                .children(self.render_comment_card(cx))
                .into_any_element(),
        )
    }

    /// The frame around the selected range, in the slide's coordinates:
    /// the range tinted except its selected cell, as in Excel, and the fill
    /// handle at its corner, which drags out a range.
    pub(crate) fn render_cell_cursor(&self, scale: f32, cx: &mut Context<Self>) -> Option<Div> {
        if self.selection.is_some() {
            return None;
        }
        let (shape, sheet) = self.current_sheet()?;
        let range = self.selected_range()?;
        let (xs, ys) = (sheet.column_offsets(), sheet.row_offsets());
        let at = |offsets: &[i64], i: u32| offsets[(i as usize).min(offsets.len() - 1)];
        let x = |c: u32| (shape.frame.x + at(&xs, c)) as f32 * scale;
        let y = |r: u32| (shape.frame.y + at(&ys, r)) as f32 * scale;
        let (x0, x1, y0, y1) = (x(range.first_column), x(range.last_column + 1), y(range.first_row), y(range.last_row + 1));
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let green = rgb(SELECTION_COLOR);
        // The tint: the range less the selected cell (or its merged range).
        let mut tint = Vec::new();
        if let Some((column, row)) = self.current_cell() {
            let single = CellRange { first_row: row, first_column: column, last_row: row, last_column: column };
            let cell = sheet.merge_at(row, column).unwrap_or(single);
            if cell != range {
                let (cx0, cx1, cy0, cy1) = (x(cell.first_column), x(cell.last_column + 1), y(cell.first_row), y(cell.last_row + 1));
                for (l, t, r, b) in [(x0, y0, x1, cy0), (x0, cy1, x1, y1), (x0, cy0, cx0, cy1), (cx1, cy0, x1, cy1)] {
                    if r > l && b > t {
                        tint.push(div().absolute().left(px(l - x0 - 1.0)).top(px(t - y0 - 1.0)).w(px(r - l)).h(px(b - t)).bg(rgba(RANGE_TINT)));
                    }
                }
            }
        }
        // The ranges 条件を選択してジャンプ selected with it: tinted.
        for r in self.selected_ranges().into_iter().skip(1) {
            let (l, t, rr, b) = (x(r.first_column), y(r.first_row), x(r.last_column + 1), y(r.last_row + 1));
            if rr > l && b > t {
                tint.push(div().absolute().left(px(l - x0 - 1.0)).top(px(t - y0 - 1.0)).w(px(rr - l)).h(px(b - t)).bg(rgba(RANGE_TINT)));
            }
        }
        // The cell being typed into: its text over the cell, with the caret.
        let editor = self.cell_edit().filter(|e| e.slide == self.current_slide).map(|e| {
            let (ex0, ex1, ey0, ey1) = (x(e.column), x(e.column + 1), y(e.row), y(e.row + 1));
            let style = sheet.cell(e.row, e.column).map(|c| sheet.styles[c.style as usize].clone()).unwrap_or_default();
            let size = style.font_size as f32 / 100.0 * ibis_doc_model::EMU_PER_POINT as f32 * scale;
            let items = e.items();
            let rect = e.rect.clone();
            e.pixels_per_point.set(ibis_doc_model::EMU_PER_POINT as f32 * scale);
            let cell_size = size.max(6.0);
            let mut line = div()
                .absolute()
                .left(px(ex0 - x0 - 1.0))
                .top(px(ey0 - y0 - 1.0))
                .min_w(px(ex1 - ex0 + 2.0))
                .h(px(ey1 - ey0 + 2.0))
                .px(px(2.0))
                .flex()
                .items_end()
                .bg(white())
                .border_2()
                .border_color(green)
                .text_size(px(size.max(6.0)))
                .text_color(rgb(style.color.0))
                .whitespace_nowrap();
            for item in items {
                line = match item {
                    super::cell_edit::EditItem::Caret => line.child(div().w(px(1.0)).h(px(cell_size * 1.2)).bg(rgb(0x000000))),
                    super::cell_edit::EditItem::Text { text, run, selected } => {
                        let mut part = div().child(SharedString::from(text));
                        if let Some(r) = &run {
                            if r.bold.unwrap_or(style.bold) {
                                part = part.font_weight(FontWeight::BOLD);
                            }
                            if r.italic.unwrap_or(style.italic) {
                                part = part.italic();
                            }
                            if r.underline.unwrap_or(style.underline).enabled() {
                                part = part.underline();
                            }
                            if r.strike.unwrap_or(style.strike) {
                                part = part.line_through();
                            }
                            if let Some(c) = r.color {
                                part = part.text_color(rgb(c.0));
                            }
                            if let Some(s) = r.size {
                                part = part.text_size(px((s as f32 / 100.0 * ibis_doc_model::EMU_PER_POINT as f32 * scale).max(6.0)));
                            }
                        }
                        if selected {
                            part = part.bg(rgba(0x3390ff55));
                        }
                        line.child(part)
                    }
                };
            }
            line.child(canvas(|_, _, _| {}, move |bounds, _, _, _| rect.set(Some(bounds))).absolute().size_full())
        });
        // データの入力規則 of the selected cell: a list's drop-down button at
        // its right, and the message under it.
        let validation = self.current_rule().filter(|_| self.cell_edit().is_none()).map(|rule| {
            let (cx0, cx1, cy0, cy1) = self.current_cell().map_or((x0, x1, y0, y1), |(c, r)| (x(c), x(c + 1), y(r), y(r + 1)));
            let mut parts = div().absolute().left(px(cx0 - x0)).top(px(cy0 - y0)).w(px(cx1 - cx0)).h(px(cy1 - cy0));
            if rule.kind == "list" && rule.dropdown {
                let side = (cy1 - cy0).clamp(12.0, 20.0);
                parts = parts.child(
                    div()
                        .id("validation-dropdown")
                        .absolute()
                        .left(px(cx1 - cx0 + 1.0))
                        .bottom(px(0.))
                        .size(px(side))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(0xF2F2F2))
                        .border_1()
                        .border_color(rgb(0xA6A6A6))
                        .text_size(px(side * 0.6))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.open_validation_list(e.position, window, cx);
                        }))
                        .child("▼"),
                );
            }
            if let Some((title, text)) = rule.prompt.clone() {
                parts = parts.child(
                    v_flex()
                        .absolute()
                        .left(px(12.))
                        .top(px(cy1 - cy0 + 6.0))
                        .max_w(px(220.))
                        .p(px(6.))
                        .bg(rgb(0xFFFFE1))
                        .border_1()
                        .border_color(rgb(0x767676))
                        .text_size(px(12.))
                        .text_color(rgb(0x000000))
                        .when(!title.is_empty(), |d| d.child(div().font_weight(FontWeight::BOLD).child(SharedString::from(title))))
                        .child(SharedString::from(text)),
                );
            }
            parts
        });
        Some(
            div()
                .absolute()
                .left(px(x0 - 1.0))
                .top(px(y0 - 1.0))
                .w(px(x1 - x0 + 2.0))
                .h(px(y1 - y0 + 2.0))
                .border_2()
                .border_color(green)
                .children(tint)
                .children(editor)
                .children(validation)
                // エラー チェック: the button at the left of the selected cell that has an error.
                .children(self.current_error().filter(|_| self.cell_edit().is_none()).map(|_| {
                    let (cx0, cy0) = self.current_cell().map_or((x0, y0), |(c, r)| (x(c), y(r)));
                    div()
                        .id("error-check")
                        .absolute()
                        .left(px(cx0 - x0 - 20.0))
                        .top(px(cy0 - y0))
                        .size(px(18.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(0xFFD966))
                        .border_1()
                        .border_color(rgb(0xA6A6A6))
                        .rounded(px(3.))
                        .text_size(px(12.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0x7F6000))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.open_error_menu(e.position, window, cx);
                        }))
                        .child("!")
                }))
                // オートフィル オプション, just past the filled range's corner.
                .children(self.fill_options_range().map(|_| {
                    div()
                        .id("fill-options")
                        .absolute()
                        .right(px(-26.))
                        .bottom(px(-24.))
                        .size(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(0xFFFFFF))
                        .border_1()
                        .border_color(rgb(0xA6A6A6))
                        .rounded(px(3.))
                        .text_size(px(11.))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.open_fill_options(e.position, window, cx);
                        }))
                        .child("▼")
                }))
                .child(
                    // A little larger than it looks, to be easy to catch.
                    div()
                        .id("fill-handle")
                        .absolute()
                        .right(px(-6.))
                        .bottom(px(-6.))
                        .size(px(11.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor(CursorStyle::Crosshair)
                        .on_mouse_down(MouseButton::Left, cx.listener(Self::on_fill_handle_down))
                        .child(div().size(px(7.)).bg(green).border_1().border_color(white())),
                ),
        )
    }

    /// Press on a worksheet: select the cell under the pointer.
    pub(crate) fn on_sheet_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.tool == Tool::TextBox {
            // A text box may be drawn over the sheet.
            return self.on_workspace_down(event, window, cx);
        }
        cx.stop_propagation();
        self.commit_edit(window, cx);
        self.focus_handle.focus(window, cx);
        self.selection = None;
        let Some((column, row)) = self.cell_under(event.position) else { return cx.notify() };
        // A press on the text being typed puts the caret there (and selects by dragging).
        if self.cell_edit().is_some_and(|e| (e.column, e.row) == (column, row) && e.slide == self.current_slide)
            && self.press_in_cell_text(event.position, event.click_count, event.modifiers.shift, cx)
        {
            return;
        }
        // Typing a formula where a reference may go (after `=`, `(`, `,` or
        // an operator): the cell pressed goes into it (dragging makes it a range).
        if self.point_reference(column, row, cx) {
            return;
        }
        // A double-click on the cell being typed into leaves it be; on
        // another cell, it edits that cell's content.
        if self.cell_edit().is_some_and(|e| (e.column, e.row) == (column, row) && e.slide == self.current_slide) {
            return;
        }
        self.commit_cell_edit(cx);
        if event.click_count >= 2 && !event.modifiers.shift {
            self.set_cell(column, row);
            if self.show_pivot_details(column, row, cx) {
                return;
            }
            self.begin_cell_edit(super::cell_edit::EditMode::Edit, true, cx);
            return;
        }
        // ⌘-click on a linked cell follows the link; elsewhere it adds a
        // range to those selected (Excel's Ctrl+クリック), dragging stretches it.
        if event.modifiers.platform && !event.modifiers.shift {
            let kept = self.selected_ranges();
            self.set_cell(column, row);
            if self.follow_cell_link(window, cx) {
                return;
            }
            self.keep_with_selection(kept);
            self.wb_mut().cell_drag = true;
            return cx.notify();
        }
        if event.modifiers.shift {
            // Shift extends the selection to the cell pressed.
            let slide = self.current_slide;
            self.wb_mut().cell_ends.insert(slide, (column, row));
        } else {
            self.set_cell(column, row);
        }
        // Dragging on from here selects a range.
        self.wb_mut().cell_drag = true;
        cx.notify();
    }

    /// Press on the fill handle at the selection's corner: dragging it
    /// selects a range from the selected cell.
    pub(crate) fn on_fill_handle_down(&mut self, _: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.commit_edit(window, cx);
        self.commit_cell_edit(cx);
        self.focus_handle.focus(window, cx);
        self.begin_fill();
        self.wb_mut().cell_drag = true;
        cx.notify();
    }

    /// Where the frozen panes of the current worksheet end, in the
    /// workspace (which is also where they are in the unscrolled content):
    /// the x after the frozen columns and the y below the frozen rows, in
    /// pixels, each `None` if that axis has none frozen.
    pub(crate) fn frozen_split(&self, content: WorkspaceContent) -> Option<(Option<f32>, Option<f32>)> {
        let (shape, sheet) = self.current_sheet()?;
        let (columns, rows) = sheet.frozen.or_else(|| self.split_panes().map(|p| p.at))?;
        let at = |offsets: Vec<i64>, i: u32| offsets[(i as usize).min(offsets.len() - 1)];
        let head_w = if sheet.headings { ROW_HEADER_WIDTH } else { 0.0 };
        let head_h = if sheet.headings { COLUMN_HEADER_HEIGHT } else { 0.0 };
        let x = (columns > 0).then(|| {
            (f32::from(content.slide_origin.x) + (shape.frame.x + at(sheet.column_offsets(), columns)) as f32 * content.scale)
                .max(head_w)
        });
        let y = (rows > 0).then(|| {
            (f32::from(content.slide_origin.y) + (shape.frame.y + at(sheet.row_offsets(), rows)) as f32 * content.scale)
                .max(head_h)
        });
        (x.is_some() || y.is_some()).then_some((x, y))
    }

    /// The slide's layout for a window position: over the frozen panes the
    /// sheet does not scroll on their axis.
    fn sheet_layout_at(&self, position: Point<Pixels>) -> Option<SlideLayout> {
        let mut layout = self.slide_layout()?;
        let (Some(ws), Some(content)) = (self.workspace_bounds, self.workspace_content()) else { return Some(layout) };
        let Some((split_x, split_y)) = self.frozen_split(content) else { return Some(layout) };
        let local = position - ws.origin;
        let shift = self.pane_shift();
        if split_x.is_some_and(|x| f32::from(local.x) < x) {
            layout.origin.x = ws.origin.x + content.slide_origin.x + shift.x;
        }
        if split_y.is_some_and(|y| f32::from(local.y) < y) {
            layout.origin.y = ws.origin.y + content.slide_origin.y + shift.y;
        }
        Some(layout)
    }

    /// The window position of a cell's centre (unfrozen part), for scripted clicks.
    #[allow(dead_code)] // For the scenario tests.
    pub(crate) fn cell_center(&self, column: u32, row: u32) -> Option<Point<Pixels>> {
        let layout = self.slide_layout()?;
        let (shape, sheet) = self.current_sheet()?;
        let (xs, ys) = (sheet.column_offsets(), sheet.row_offsets());
        let x = shape.frame.x + (xs.get(column as usize)? + xs.get(column as usize + 1)?) / 2;
        let y = shape.frame.y + (ys.get(row as usize)? + ys.get(row as usize + 1)?) / 2;
        Some(point(layout.origin.x + layout.to_px(x), layout.origin.y + layout.to_px(y)))
    }

    /// The window position of a cell's bottom-right corner (where the fill
    /// handle is when it ends the selection), for scripted drags.
    #[allow(dead_code)] // For the scenario tests.
    pub(crate) fn cell_corner(&self, column: u32, row: u32) -> Option<Point<Pixels>> {
        let layout = self.slide_layout()?;
        let (shape, sheet) = self.current_sheet()?;
        let (xs, ys) = (sheet.column_offsets(), sheet.row_offsets());
        let x = shape.frame.x + xs.get(column as usize + 1)?;
        let y = shape.frame.y + ys.get(row as usize + 1)?;
        Some(point(layout.origin.x + layout.to_px(x), layout.origin.y + layout.to_px(y)))
    }

    /// The cell under a window position, if it is on the worksheet.
    pub(crate) fn cell_under(&self, position: Point<Pixels>) -> Option<(u32, u32)> {
        let (layout, (shape, sheet)) = self.sheet_layout_at(position).zip(self.current_sheet())?;
        let p = layout.to_emu(position);
        sheet.cell_at(p.x - shape.frame.x, p.y - shape.frame.y)
    }

    /// Page break preview (Excel's 改ページ プレビュー): the cells outside
    /// the print area greyed and a blue line around it, in the slide's
    /// coordinates. Without a print area, the page the sheet prints is used.
    pub(crate) fn render_page_break_preview(&self, scale: f32, lines_layer: bool) -> Option<Div> {
        let (shape, sheet) = self.current_sheet()?;
        if !sheet.page_break_preview && !sheet.page_layout {
            return None;
        }
        let layout_view = sheet.page_layout;
        let frame = shape.frame;
        let (x0, y0, x1, y1) = match sheet.print_area {
            Some(area) => {
                let (xs, ys) = (sheet.column_offsets(), sheet.row_offsets());
                // Whole columns or rows (an open end) reach the grid's end.
                let at = |offsets: &[i64], i: u32| offsets[(i as usize).min(offsets.len() - 1)];
                (
                    frame.x + at(&xs, area.first_column),
                    frame.y + at(&ys, area.first_row),
                    frame.x + at(&xs, area.last_column.saturating_add(1)),
                    frame.y + at(&ys, area.last_row.saturating_add(1)),
                )
            }
            None => {
                let page = self.presentation().size_of(self.slide());
                (frame.x, frame.y, frame.x + page.width.min(frame.width), frame.y + page.height.min(frame.height))
            }
        };
        let to_px = |emu: i64| emu as f32 * scale;
        let (fl, ft, fr, fb) = (to_px(frame.x), to_px(frame.y), to_px(frame.right()), to_px(frame.bottom()));
        let (pl, pt, pr, pb) = (to_px(x0), to_px(y0), to_px(x1), to_px(y1));
        let grey = rgba(OUTSIDE_PRINT_AREA);
        let rect = |l: f32, t: f32, r: f32, b: f32| {
            (r > l && b > t).then(|| div().absolute().left(px(l)).top(px(t)).w(px(r - l)).h(px(b - t)).bg(grey))
        };
        let shades = if layout_view || lines_layer { [None, None, None, None] } else { [
            rect(fl, ft, fr, pt),
            rect(fl, pb, fr, fb),
            rect(fl, pt.max(ft), pl, pb.min(fb)),
            rect(pr, pt.max(ft), fr, pb.min(fb)),
        ] };
        let w = PRINT_AREA_LINE_WIDTH;
        // Where the pages break: a dashed blue line at an automatic break, a
        // solid one at a manual break, and the page's number in grey.
        // (The pages come from the exporter, which is not in every build: without it, no page lines.)
        #[cfg(all(feature = "export", any(target_os = "macos", target_os = "linux")))]
        let (print_scale, pages) = crate::export::preview_pages(self.presentation(), self.slide(), &self.resources);
        #[cfg(not(all(feature = "export", any(target_os = "macos", target_os = "linux"))))]
        let (print_scale, pages): (f64, Vec<ibis_doc_model::EmuRect>) = (1.0, Vec::new());
        let (xs, ys) = (sheet.column_offsets(), sheet.row_offsets());
        let manual_x: Vec<i64> = sheet.print.column_breaks.iter().filter_map(|b| xs.get(*b as usize)).map(|x| frame.x + x).collect();
        let manual_y: Vec<i64> = sheet.print.row_breaks.iter().filter_map(|b| ys.get(*b as usize)).map(|y| frame.y + y).collect();
        let blue = rgb(PRINT_AREA_LINE);
        let mut lines: Vec<Div> = Vec::new();
        for page in pages.iter().filter(|_| lines_layer) {
            let (l, t, r, b) = (to_px(page.x), to_px(page.y), to_px(page.x + page.width), to_px(page.y + page.height));
            // The left and top edges (the previous page's right and bottom) that are not the area's start.
            if page.x > x0 {
                let line = div().absolute().left(px(l - 0.5)).top(px(t)).w(px(1.0)).h(px(b - t));
                lines.push(if manual_x.contains(&page.x) { line.bg(blue) } else { line.border_l_1().border_dashed().border_color(blue) });
            }
            if page.y > y0 {
                let line = div().absolute().left(px(l)).top(px(t - 0.5)).w(px(r - l)).h(px(1.0));
                lines.push(if manual_y.contains(&page.y) { line.bg(blue) } else { line.border_t_1().border_dashed().border_color(blue) });
            }
        }
        let numbers = pages.iter().enumerate().filter(|_| lines_layer).map(|(i, page)| {
            let (l, t, r, b) = (to_px(page.x), to_px(page.y), to_px(page.x + page.width), to_px(page.y + page.height));
            div()
                .absolute()
                .left(px(l))
                .top(px(t))
                .w(px(r - l))
                .h(px(b - t))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(((r - l).min(b - t) * 0.35).clamp(12.0, 160.0)))
                .text_color(rgba(0x80808040))
                .child(SharedString::from(format!("{}", i + 1)))
        });
        // Page layout view: each page's printed area outlined, and the paper's
        // edge (the margins, which print at the print scale) around all of them.
        let mut frames: Vec<Div> = Vec::new();
        if layout_view && lines_layer && !pages.is_empty() {
            let grey = rgb(0x9a9a9a);
            for page in pages.iter() {
                let (l, t, r, b) = (to_px(page.x), to_px(page.y), to_px(page.x + page.width), to_px(page.y + page.height));
                frames.push(div().absolute().left(px(l)).top(px(t)).w(px(r - l)).h(px(b - t)).border_1().border_color(grey));
            }
            let [ml, mr, mt, mb] = sheet.print.margins;
            let k = 1.0 / print_scale.max(0.1);
            let l = pages.iter().map(|p| p.x).min().unwrap_or(0) as f64 - ml as f64 * k;
            let t = pages.iter().map(|p| p.y).min().unwrap_or(0) as f64 - mt as f64 * k;
            let r = pages.iter().map(|p| p.x + p.width).max().unwrap_or(0) as f64 + mr as f64 * k;
            let b = pages.iter().map(|p| p.y + p.height).max().unwrap_or(0) as f64 + mb as f64 * k;
            let (l, t, r, b) = (to_px(l as i64), to_px(t as i64), to_px(r as i64), to_px(b as i64));
            frames.push(div().absolute().left(px(l)).top(px(t)).w(px(r - l)).h(px(b - t)).border_1().border_dashed().border_color(grey));
        }
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .children(shades.into_iter().flatten())
                .children(numbers)
                .children(lines)
                .children(frames)
                .when(pr > pl && pb > pt && !layout_view && !lines_layer, |d| {
                    d.child(
                        div()
                            .absolute()
                            .left(px(pl - w / 2.0))
                            .top(px(pt - w / 2.0))
                            .w(px(pr - pl + w))
                            .h(px(pb - pt + w))
                            .border(px(w))
                            .border_color(rgb(PRINT_AREA_LINE)),
                    )
                }),
        )
    }

    /// Stretch the selected range to the cell under `position` (the nearest
    /// cell at the sheet's edges).
    pub(crate) fn extend_cells_to(&mut self, position: Point<Pixels>) {
        let Some((layout, (shape, sheet))) = self.sheet_layout_at(position).zip(self.current_sheet()) else { return };
        let p = layout.to_emu(position);
        let clamp = |offsets: Vec<i64>, v: i64| -> u32 {
            let last = offsets.len().saturating_sub(2);
            (offsets.partition_point(|o| *o <= v).saturating_sub(1)).min(last) as u32
        };
        let (xs, ys) = (sheet.column_offsets(), sheet.row_offsets());
        let (px_, py_) = (p.x - shape.frame.x, p.y - shape.frame.y);
        let mut column = clamp(xs.clone(), px_);
        let mut row = clamp(ys.clone(), py_);
        // Dragging the fill handle goes one way (`fill_direction`).
        if let Some(source) = self.sheet_state().and_then(|s| s.fill_source) {
            (column, row) = super::fill::fill_target(source, (column, row), (px_, py_), &xs, &ys);
        }
        let anchor = self.current_cell();
        let was_kept = self.selected_ranges().len() > 1;
        if anchor == Some((column, row)) {
            let slide = self.current_slide;
            self.wb_mut().cell_ends.remove(&slide);
        } else {
            let slide = self.current_slide;
            self.wb_mut().cell_ends.insert(slide, (column, row));
        }
        self.extra_ranges_follow(was_kept);
    }

    /// Move the selection one cell (or past the merged range it is on),
    /// skipping hidden rows and columns and stopping at the sheet's edge.
    pub(crate) fn move_cell(&mut self, dx: i32, dy: i32, _: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            return;
        }
        // While typing: ← → move the caret in Edit mode; otherwise the entry is finished first.
        if self.cell_edit().is_some() {
            if dy == 0 && self.cell_caret(dx, cx) {
                return;
            }
            if dx == 0 && self.completion_move(dy, cx) {
                return;
            }
            self.commit_cell_edit(cx);
        }
        let (Some((_, sheet)), Some((column, row))) = (self.current_sheet(), self.current_cell()) else {
            return;
        };
        // From the selected cell (or its merged range), not the whole range.
        let range = sheet.merge_at(row, column).unwrap_or(CellRange { first_row: row, first_column: column, last_row: row, last_column: column });
        let (columns, rows) = (sheet.columns.clone(), sheet.rows.clone());
        let step = |at: u32, first: u32, last: u32, d: i32, sizes: &[i64]| -> Option<u32> {
            if d == 0 {
                return Some(at);
            }
            let mut i = if d > 0 { last as i64 + 1 } else { first as i64 - 1 };
            while (0..sizes.len() as i64).contains(&i) && sizes[i as usize] == 0 {
                i += d.signum() as i64;
            }
            (0..sizes.len() as i64).contains(&i).then_some(i as u32)
        };
        let next = step(column, range.first_column, range.last_column, dx, &columns)
            .zip(step(row, range.first_row, range.last_row, dy, &rows));
        let Some((column, row)) = next else { return };
        self.selection = None;
        self.set_cell(column, row);
        self.scroll_cell_into_view();
        cx.notify();
    }

    /// Tab: as → (after finishing the entry), remembering the column the
    /// Tabs began at for Enter.
    pub(crate) fn cell_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The formula's list open: Tab puts in the item chosen.
        if self.completion_accept(cx) {
            return;
        }
        let Some((column, _)) = self.current_cell() else { return };
        let start = self.wb().tab_start.filter(|t| Some(t.1) == self.current_cell()).map_or(column, |t| t.0);
        self.commit_cell_edit(cx);
        self.move_cell(1, 0, window, cx);
        let here = self.current_cell();
        self.wb_mut().tab_start = here.map(|h| (start, h));
    }

    /// Enter: down a row; after Tabs, back to the column they began at (Excel).
    pub(crate) fn cell_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.wb_mut().tab_start.take().filter(|t| Some(t.1) == self.current_cell());
        match tab {
            Some((start, (_, row))) => {
                self.commit_cell_edit(cx);
                if self.current_sheet().is_some_and(|(_, s)| (row as usize + 1) < s.rows.len()) {
                    self.selection = None;
                    self.set_cell(start, row + 1);
                    self.scroll_cell_into_view();
                }
                cx.notify();
            }
            None => self.move_cell(0, 1, window, cx),
        }
    }

    /// Shift+arrow: move the range's far corner (the one away from the
    /// selected cell) a cell, past hidden rows and columns, as Excel does.
    pub(crate) fn extend_cells(&mut self, dx: i32, dy: i32, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            return;
        }
        // Typing in Edit mode: Shift+← → select text.
        if dy == 0 && self.cell_extend_selection(dx, cx) {
            return;
        }
        let (Some((_, sheet)), Some(range), Some((column, row))) = (self.current_sheet(), self.selected_range(), self.current_cell()) else {
            return;
        };
        // The far edge in each direction: the side of the range the selected
        // cell is not on.
        let far_column = if column > range.first_column { range.first_column } else { range.last_column };
        let far_row = if row > range.first_row { range.first_row } else { range.last_row };
        let step = |at: u32, d: i32, sizes: &[i64]| -> u32 {
            if d == 0 {
                return at;
            }
            let mut i = at as i64 + d as i64;
            while (0..sizes.len() as i64).contains(&i) && sizes[i as usize] == 0 {
                i += d as i64;
            }
            if (0..sizes.len() as i64).contains(&i) { i as u32 } else { at }
        };
        let end = (step(far_column, dx, &sheet.columns), step(far_row, dy, &sheet.rows));
        if end == (column, row) {
            let slide = self.current_slide;
            self.wb_mut().cell_ends.remove(&slide);
        } else {
            let slide = self.current_slide;
            self.wb_mut().cell_ends.insert(slide, end);
        }
        self.scroll_cell_into_view();
        cx.notify();
    }

    /// Select a cell of the current worksheet (its merged range's top-left
    /// cell if it is merged).
    pub(crate) fn set_cell(&mut self, column: u32, row: u32) {
        let Some((_, sheet)) = self.current_sheet() else { return };
        let cell = sheet.merge_at(row, column).map_or((column, row), |m| (m.first_column, m.first_row));
        let slide = self.current_slide;
        self.wb_mut().cells.insert(slide, cell);
        let slide = self.current_slide;
        self.wb_mut().cell_ends.remove(&slide);
    }

    /// Scroll the workspace so the selected cell is in view, clear of the headers.
    fn scroll_cell_into_view(&self) {
        self.scroll_range_into_view(false);
    }

    /// Scroll the workspace so the selected range is in view, clear of the
    /// headers: as little as it takes, or (`top_left`, as Excel's ジャンプ
    /// and name box do) on each axis it is not in view, with the range at
    /// the view's top or left, past frozen panes.
    pub(crate) fn scroll_range_into_view(&self, top_left: bool) {
        let (Some(content), Some(ws), Some((shape, sheet)), Some(range)) =
            (self.workspace_content(), self.workspace_bounds, self.current_sheet(), self.selected_range())
        else {
            return;
        };
        let scale = content.scale;
        let (xs, ys) = (sheet.column_offsets(), sheet.row_offsets());
        // Past frozen panes the view starts after them; a cell in them is
        // always in view, so that axis does not scroll.
        let (frozen_columns, frozen_rows) = sheet.frozen.unwrap_or((0, 0));
        let (split_x, split_y) = self.frozen_split(content).unwrap_or((None, None));
        let at = |offsets: &[i64], i: u32| offsets[(i as usize).min(offsets.len() - 1)];
        let span = |origin: Pixels, frame: i64, offsets: &[i64], first: u32, last: u32| {
            let start = f32::from(origin) + (frame + at(offsets, first)) as f32 * scale;
            let end = f32::from(origin) + (frame + at(offsets, last + 1)) as f32 * scale;
            (start, end)
        };
        // `visible` is the scrolled-away amount; the view shows content from
        // `visible + head` to `visible + view`.
        let axis = |visible: f32, (start, end): (f32, f32), head: f32, view: f32, content: f32| {
            let visible = if start - head < visible {
                start - head
            } else if end > visible + view - SCROLLBAR_ROOM && top_left {
                start - head
            } else if end > visible + view - SCROLLBAR_ROOM {
                (end - view + SCROLLBAR_ROOM).min(start - head)
            } else {
                visible
            };
            -visible.clamp(0.0, (content - view).max(0.0))
        };
        let offset = self.workspace_scroll.offset();
        let x = if range.last_column < frozen_columns {
            f32::from(offset.x)
        } else {
            let (start, end) = span(content.slide_origin.x, shape.frame.x, &xs, range.first_column, range.last_column);
            // The scrolling pane starts after the frozen columns.
            let head = split_x.unwrap_or(if sheet.headings { ROW_HEADER_WIDTH } else { 0.0 });
            axis(-f32::from(offset.x), (start, end), head, f32::from(ws.size.width), f32::from(content.size.width))
        };
        let y = if range.last_row < frozen_rows {
            f32::from(offset.y)
        } else {
            let (start, end) = span(content.slide_origin.y, shape.frame.y, &ys, range.first_row, range.last_row);
            let head = split_y.unwrap_or(if sheet.headings { COLUMN_HEADER_HEIGHT } else { 0.0 });
            axis(-f32::from(offset.y), (start, end), head, f32::from(ws.size.height), f32::from(content.size.height))
        };
        self.workspace_scroll.set_offset(point(px(x), px(y)));
    }
}

/// The name box and the ジャンプ dialog, where a cell reference is typed to
/// select it, as in Excel.
pub(crate) struct CellJump {
    /// The name box, at the left of the formula bar.
    pub(crate) name_box: Entity<InputState>,
    /// What the name box was last set to show (the selection), so it is set
    /// again only when that changes.
    name_box_shown: String,
    /// The press under way put the keys in the name box: its release
    /// selects the reference, to be typed over, as in Excel (after the
    /// press has put the caret where it was).
    select_on_release: bool,
    /// The 参照先 field of the ジャンプ dialog.
    goto_input: Entity<InputState>,
    /// The ジャンプ dialog is open.
    pub(crate) goto_open: bool,
    /// Why the dialog's reference was not taken, shown in it.
    goto_error: Option<SharedString>,
    /// A jump was made before the workspace was laid out: its range is to
    /// be scrolled into view once it is.
    pub(crate) pending_scroll: bool,
    /// The name field over a sheet tab being renamed (a double-click on
    /// the tab, or シートの名前を変更).
    pub(crate) tab_rename: Entity<InputState>,
    /// The sheet being renamed.
    pub(crate) renaming: Option<SlideId>,
    _subscriptions: [Subscription; 3],
}

impl CellJump {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<SlideEditor>) -> Self {
        let name_box = cx.new(|cx| InputState::new(window, cx));
        let goto_input = cx.new(|cx| InputState::new(window, cx));
        let name_box_subscription = cx.subscribe_in(&name_box, window, |this, input, event: &InputEvent, window, cx| match event {
            InputEvent::PressEnter { .. } => {
                let text = input.read(cx).value().to_string();
                if this.go_to_reference(&text, window, cx) {
                    this.focus_handle.focus(window, cx);
                } else if super::names::valid_name(text.trim()) && this.name_selection(text.trim(), cx) {
                    // A new name for the selected range, as in Excel.
                    this.focus_handle.focus(window, cx);
                } else {
                    // As after Excel's error: the reference, selected, to be put right.
                    this.status = Some(crate::shell::i18n::format("_xlsx.invalid-reference-text", &[("text", &text.trim())]).into());
                    input.update(cx, |input, cx| input.select_all(window, cx));
                    cx.notify();
                }
            }
            _ => {}
        });
        let goto_subscription = cx.subscribe_in(&goto_input, window, |this, _, event: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                this.confirm_goto(window, cx);
            }
        });
        let tab_rename = cx.new(|cx| InputState::new(window, cx));
        let rename_subscription = cx.subscribe_in(&tab_rename, window, |this, _, event: &InputEvent, window, cx| match event {
            InputEvent::PressEnter { .. } | InputEvent::Blur => this.confirm_sheet_rename(window, cx),
            _ => {}
        });
        Self {
            name_box,
            name_box_shown: String::new(),
            select_on_release: false,
            goto_input,
            goto_open: false,
            goto_error: None,
            pending_scroll: false,
            tab_rename,
            renaming: None,
            _subscriptions: [name_box_subscription, goto_subscription, rename_subscription],
        }
    }
}

/// A place typed in the name box or the ジャンプ dialog: cells or a name, on
/// the sheet named or (`None`) the current one.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Reference {
    pub(crate) sheet: Option<String>,
    pub(crate) target: Target,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    /// A cell or a range, 0-based; whole columns or rows reach [`OPEN_END`].
    Cells(CellRange),
    /// A defined name, such as `Print_Area`.
    Name(String),
}

/// The far end of whole columns or rows (`A:C`, `3:5`), before it is cut
/// to the sheet's grid.
pub(crate) const OPEN_END: u32 = u32::MAX;
/// Excel's limits: columns A to XFD, rows 1 to 1,048,576.
const MAX_COLUMNS: u32 = 16_384;
const MAX_ROWS: u32 = 1_048_576;

/// Read a reference as Excel's name box does: `B12`, `$B$12`, `b12`,
/// `A1:C5`, `A:C`, `3:5`, `Sheet2!A1`, `'Sheet name'!A1` (a `'` in the
/// name doubled) or a name. `None` if it is none of these.
pub(crate) fn parse_reference(text: &str) -> Option<Reference> {
    let text = text.trim();
    let (sheet, rest) = if let Some(quoted) = text.strip_prefix('\'') {
        // Up to the closing quote; `''` stands for one.
        let mut name = String::new();
        let mut chars = quoted.char_indices().peekable();
        let rest = loop {
            match chars.next()? {
                (_, '\'') if chars.peek().is_some_and(|(_, c)| *c == '\'') => {
                    chars.next();
                    name.push('\'');
                }
                (i, '\'') => break &quoted[i + 1..],
                (_, c) => name.push(c),
            }
        };
        (Some(name), rest.strip_prefix('!')?)
    } else if let Some((name, rest)) = text.rsplit_once('!') {
        (Some(name.to_string()), rest)
    } else {
        (None, text)
    };
    if sheet.as_deref().is_some_and(str::is_empty) {
        return None;
    }
    let target = match parse_cells(rest) {
        Some(range) => Target::Cells(range),
        None if is_name(rest) => Target::Name(rest.to_string()),
        None => return None,
    };
    Some(Reference { sheet, target })
}

/// One end of a range: a cell (`B12`), a whole column (`B`) or a whole row
/// (`12`), 0-based.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RangeEnd {
    Cell(u32, u32),
    Column(u32),
    Row(u32),
}

/// `B12`, `$B$12`, `b12`, `B` or `12`, within Excel's limits.
fn parse_range_end(text: &str) -> Option<RangeEnd> {
    let text = text.strip_prefix('$').unwrap_or(text);
    let split = text.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(text.len());
    let (letters, digits) = text.split_at(split);
    let digits = if letters.is_empty() { digits } else { digits.strip_prefix('$').unwrap_or(digits) };
    if !digits.bytes().all(|b| b.is_ascii_digit()) || letters.len() > 3 {
        return None;
    }
    let column = letters.bytes().fold(0, |n, b| n * 26 + u32::from(b.to_ascii_uppercase() - b'A' + 1));
    if column > MAX_COLUMNS {
        return None;
    }
    let row = match digits {
        "" => 0,
        _ => digits.parse::<u32>().ok().filter(|r| (1..=MAX_ROWS).contains(r))?,
    };
    match (column, row) {
        (0, 0) => None,
        (0, row) => Some(RangeEnd::Row(row - 1)),
        (column, 0) => Some(RangeEnd::Column(column - 1)),
        (column, row) => Some(RangeEnd::Cell(column - 1, row - 1)),
    }
}

/// A cell or a range of cells, columns or rows.
fn parse_cells(text: &str) -> Option<CellRange> {
    let mut ends = text.split(':');
    let first = parse_range_end(ends.next()?)?;
    let second = ends.next().map(parse_range_end);
    if ends.next().is_some() {
        return None;
    }
    let range = |c0: u32, r0: u32, c1: u32, r1: u32| CellRange {
        first_row: r0.min(r1),
        first_column: c0.min(c1),
        last_row: r0.max(r1),
        last_column: c0.max(c1),
    };
    match (first, second) {
        (RangeEnd::Cell(c, r), None) => Some(range(c, r, c, r)),
        (RangeEnd::Cell(c0, r0), Some(Some(RangeEnd::Cell(c1, r1)))) => Some(range(c0, r0, c1, r1)),
        (RangeEnd::Column(c0), Some(Some(RangeEnd::Column(c1)))) => Some(range(c0, 0, c1, OPEN_END)),
        (RangeEnd::Row(r0), Some(Some(RangeEnd::Row(r1)))) => Some(range(0, r0, OPEN_END, r1)),
        _ => None,
    }
}

/// Whether `text` can be a defined name: a letter, `_` or `\` and then
/// letters, digits, `_` and `.`.
pub(crate) fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '\\')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

/// The print area's name, as the name box takes it (Excel keeps it as
/// `_xlnm.Print_Area`).
fn is_print_area(name: &str) -> bool {
    name.eq_ignore_ascii_case("Print_Area") || name.eq_ignore_ascii_case("_xlnm.Print_Area")
}

/// A range as the name box shows it: `B12`, or `A1:C5`.
fn range_name(range: CellRange) -> String {
    let first = cell_name(range.first_column, range.first_row);
    if range.first_column == range.last_column && range.first_row == range.last_row {
        first
    } else {
        format!("{first}:{}", cell_name(range.last_column, range.last_row))
    }
}

/// Where Ctrl+arrow goes along a row or column, from `at` a step `d` (±1)
/// at a time over `sizes` (0 for a hidden row or column), as Excel does:
/// from a filled cell with a filled one next to it, to the last filled one
/// before an empty one; otherwise to the next filled one, or the edge.
fn data_edge(at: u32, d: i32, sizes: &[i64], filled: impl Fn(u32) -> bool) -> u32 {
    let next = |i: u32| {
        let mut i = i as i64 + d as i64;
        while (0..sizes.len() as i64).contains(&i) && sizes[i as usize] == 0 {
            i += d as i64;
        }
        (0..sizes.len() as i64).contains(&i).then_some(i as u32)
    };
    let Some(mut n) = next(at) else { return at };
    if filled(at) && filled(n) {
        while let Some(m) = next(n).filter(|m| filled(*m)) {
            n = m;
        }
    } else {
        while !filled(n) {
            match next(n) {
                Some(m) => n = m,
                None => break,
            }
        }
    }
    n
}

/// A move of Excel's keys: to the edge of the data region (Ctrl+arrow),
/// column A (Home), the first cell (Ctrl+Home) or the last in use (Ctrl+End).
#[derive(Clone, Copy)]
enum Jump {
    Edge(i32, i32),
    RowStart,
    First,
    Last,
}

impl SlideEditor {
    /// The bindings of the keys that move about the cells and open ジャンプ.
    pub(crate) fn cell_jump_actions(d: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        d.on_action(cx.listener(|this, _: &GoTo, window, cx| this.open_goto(window, cx)))
            .on_action(cx.listener(|this, _: &InsertTable, window, cx| this.open_table_dialog(window, cx)))
            .on_action(cx.listener(|this, _: &JumpCellUp, _, cx| this.jump(Jump::Edge(0, -1), false, cx)))
            .on_action(cx.listener(|this, _: &JumpCellDown, _, cx| this.jump(Jump::Edge(0, 1), false, cx)))
            .on_action(cx.listener(|this, _: &JumpCellLeft, _, cx| this.jump(Jump::Edge(-1, 0), false, cx)))
            .on_action(cx.listener(|this, _: &JumpCellRight, _, cx| this.jump(Jump::Edge(1, 0), false, cx)))
            .on_action(cx.listener(|this, _: &ExtendJumpUp, _, cx| this.jump(Jump::Edge(0, -1), true, cx)))
            .on_action(cx.listener(|this, _: &ExtendJumpDown, _, cx| this.jump(Jump::Edge(0, 1), true, cx)))
            .on_action(cx.listener(|this, _: &ExtendJumpLeft, _, cx| this.jump(Jump::Edge(-1, 0), true, cx)))
            .on_action(cx.listener(|this, _: &ExtendJumpRight, _, cx| this.jump(Jump::Edge(1, 0), true, cx)))
            .on_action(cx.listener(|this, _: &SelectRowStart, _, cx| this.jump(Jump::RowStart, false, cx)))
            .on_action(cx.listener(|this, _: &ExtendRowStart, _, cx| this.jump(Jump::RowStart, true, cx)))
            .on_action(cx.listener(|this, _: &SelectFirstCell, _, cx| this.jump(Jump::First, false, cx)))
            .on_action(cx.listener(|this, _: &ExtendFirstCell, _, cx| this.jump(Jump::First, true, cx)))
            .on_action(cx.listener(|this, _: &SelectLastCell, _, cx| this.jump(Jump::Last, false, cx)))
            .on_action(cx.listener(|this, _: &ExtendLastCell, _, cx| this.jump(Jump::Last, true, cx)))
            .on_action(cx.listener(|this, _: &EditCell, window, cx| {
                if this.cells_take_typing(window, cx) && this.cell_edit().is_none() {
                    this.begin_cell_edit(super::cell_edit::EditMode::Edit, true, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &CellBackspace, window, cx| {
                if this.cells_take_typing(window, cx) {
                    this.cell_delete(false, cx);
                } else {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &CellBold, _, cx| {
                let on = !this.current_style_bold();
                this.format_cells(ibis_doc_model::StyleChange::Bold(on), cx)
            }))
            .on_action(cx.listener(|this, _: &CellItalic, _, cx| {
                let on = !this.current_style_italic();
                this.format_cells(ibis_doc_model::StyleChange::Italic(on), cx)
            }))
            .on_action(cx.listener(|this, _: &CellUnderline, _, cx| {
                let on = !this.current_style_underline();
                this.format_cells(ibis_doc_model::StyleChange::Underline(on), cx)
            }))
            // While typing into a cell, copy, cut and paste work on the text being typed.
            .on_action(cx.listener(|this, _: &CellCopy, _, cx| if this.cell_edit().is_some() { this.edit_copy(false, cx) } else { this.copy_cells(false, cx) }))
            .on_action(cx.listener(|this, _: &CellCut, _, cx| if this.cell_edit().is_some() { this.edit_copy(true, cx) } else { this.copy_cells(true, cx) }))
            .on_action(cx.listener(|this, _: &CellPaste, _, cx| if this.cell_edit().is_some() { this.edit_paste(cx) } else { this.paste_cells(false, cx) }))
            .on_action(cx.listener(|this, _: &CellPastePlain, _, cx| if this.cell_edit().is_some() { this.edit_paste(cx) } else { this.paste_cells(true, cx) }))
            .on_action(cx.listener(|this, _: &ClearCellFormats, _, cx| this.clear_cell_formats(cx)))
            .on_action(cx.listener(|this, _: &CellTab, window, cx| this.cell_tab(window, cx)))
            .on_action(cx.listener(|this, _: &SelectAllCells, _, cx| this.select_all_cells(cx)))
            .on_action(cx.listener(|this, _: &InsertLink, window, cx| this.open_link_panel(window, cx)))
            .on_action(cx.listener(|this, _: &CycleReference, _, cx| {
                this.cycle_reference(cx);
            }))
            .on_action(cx.listener(|this, _: &CellEnter, window, cx| this.cell_enter(window, cx)))
            .on_action(cx.listener(|this, _: &CellFillSelection, _, cx| this.fill_selection_with_entry(cx)))
            .on_action(cx.listener(|this, _: &FillDown, _, cx| {
                // With shapes selected, ⌘D duplicates them (as in Excel).
                if this.selected_shapes().is_empty() {
                    this.fill_selection_edge(true, cx);
                } else {
                    this.duplicate_selected(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &OpenCellFormat, window, cx| this.open_custom_format(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleFilter, _, cx| this.toggle_auto_filter(cx)))
            .on_action(cx.listener(|this, _: &FillRight, _, cx| this.fill_selection_edge(false, cx)))
            .on_action(cx.listener(|this, _: &InsertDate, _, cx| this.insert_now_stamp(true, cx)))
            .on_action(cx.listener(|this, _: &InsertTime, _, cx| this.insert_now_stamp(false, cx)))
            .on_action(cx.listener(|this, _: &SelectColumns, _, cx| this.select_selection_lines(Axis::Columns, cx)))
            .on_action(cx.listener(|this, _: &SelectRows, _, cx| this.select_selection_lines(Axis::Rows, cx)))
            .on_action(cx.listener(|this, _: &CellLineBreak, _, cx| this.cell_line_break(cx)))
            .on_action(cx.listener(|this, _: &FlashFill, _, cx| this.flash_fill_column(cx)))
            .on_action(cx.listener(|this, _: &HideRows, _, cx| this.hide_lines(Axis::Rows, true, cx)))
            .on_action(cx.listener(|this, _: &HideColumns, _, cx| this.hide_lines(Axis::Columns, true, cx)))
            .on_action(cx.listener(|this, _: &UnhideRows, _, cx| this.hide_lines(Axis::Rows, false, cx)))
            .on_action(cx.listener(|this, _: &UnhideColumns, _, cx| this.hide_lines(Axis::Columns, false, cx)))
            .on_action(cx.listener(|this, _: &CalculateNow, _, cx| {
                this.run_sheet(vec![ibis_doc_model::Command::Sheet(ibis_doc_model::SheetCommand::CalculateNow)], cx);
            }))
            .on_action(cx.listener(|this, _: &CalculateSheet, _, cx| {
                let slide = this.current_slide;
                this.run_sheet(vec![ibis_doc_model::Command::Sheet(ibis_doc_model::SheetCommand::CalculateSheet { slide })], cx);
            }))
            .on_action(cx.listener(|this, _: &CellDelete, window, cx| {
                if this.cells_take_typing(window, cx) {
                    this.cell_delete(true, cx);
                } else {
                    cx.propagate();
                }
            }))
    }

    /// Select the cell a key such as Ctrl+→ goes to, or (`extend`) stretch
    /// the selected range's far corner there.
    fn jump(&mut self, jump: Jump, extend: bool, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            return;
        }
        let (Some((_, sheet)), Some(range), Some((column, row))) = (self.current_sheet(), self.selected_range(), self.current_cell()) else {
            return;
        };
        let last = |sizes: &[i64]| sizes.len().saturating_sub(1) as u32;
        let (last_column, last_row) = (last(&sheet.columns), last(&sheet.rows));
        // From the far corner when stretching (as `extend_cells`), else from
        // the selected cell's edge on the side it moves to.
        let merged = sheet.merge_at(row, column).unwrap_or(CellRange { first_row: row, first_column: column, last_row: row, last_column: column });
        let from = if extend {
            let far_column = if column > range.first_column { range.first_column } else { range.last_column };
            let far_row = if row > range.first_row { range.first_row } else { range.last_row };
            (far_column, far_row)
        } else {
            (column, row)
        };
        let filled = |column: u32, row: u32| {
            // A merged range's cells are filled if its top-left one is.
            let (column, row) = sheet.merge_at(row, column).map_or((column, row), |m| (m.first_column, m.first_row));
            sheet.cell(row, column).is_some_and(|cell| !cell.text.is_empty())
        };
        let to = match jump {
            Jump::Edge(dx, 0) => {
                let at = match (extend, dx > 0) {
                    (true, _) => from.0,
                    (false, true) => merged.last_column,
                    (false, false) => merged.first_column,
                };
                (data_edge(at, dx, &sheet.columns, |c| filled(c, from.1)), from.1)
            }
            Jump::Edge(_, dy) => {
                let at = match (extend, dy > 0) {
                    (true, _) => from.1,
                    (false, true) => merged.last_row,
                    (false, false) => merged.first_row,
                };
                (from.0, data_edge(at, dy, &sheet.rows, |r| filled(from.0, r)))
            }
            Jump::RowStart => (0, from.1),
            Jump::First => {
                let (columns, rows) = sheet.frozen.unwrap_or((0, 0));
                (columns.min(last_column), rows.min(last_row))
            }
            Jump::Last => {
                let used = sheet.cells.iter().fold((0, 0), |(c, r), cell| (c.max(cell.column), r.max(cell.row)));
                (used.0.min(last_column), used.1.min(last_row))
            }
        };
        if extend {
            if to == (column, row) {
                let slide = self.current_slide;
                self.wb_mut().cell_ends.remove(&slide);
            } else {
                let slide = self.current_slide;
                self.wb_mut().cell_ends.insert(slide, to);
            }
        } else {
            self.selection = None;
            self.set_cell(to.0, to.1);
        }
        self.scroll_cell_into_view();
        cx.notify();
    }

    /// What the name box shows: the selected cell, or the selected range if
    /// it is more than that cell (or its merged range); nothing while a
    /// shape is selected.
    fn name_box_text(&self) -> String {
        let (None, Some((_, sheet)), Some((column, row)), Some(range)) =
            (self.selection, self.current_sheet(), self.current_cell(), self.selected_range())
        else {
            return String::new();
        };
        let cell = sheet.merge_at(row, column).unwrap_or(CellRange { first_row: row, first_column: column, last_row: row, last_column: column });
        if range == cell { cell_name(column, row) } else { range_name(range) }
    }

    /// Show the selection in the name box, unless something is being typed in it.
    fn sync_name_box(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.name_box_text();
        let typing = self.wb().jump.name_box.read(cx).focus_handle(cx).is_focused(window);
        if !typing && text != self.wb().jump.name_box_shown {
            self.wb().jump.name_box.update(cx, |input, cx| input.set_value(text.clone(), window, cx));
            self.wb_mut().jump.name_box_shown = text;
        }
    }

    /// The sheet and cells a reference points to: on the sheet named (by
    /// its tab's name, in any case) or the current one, cut to its grid.
    pub(crate) fn resolve_reference(&self, reference: &Reference) -> Option<(SlideId, CellRange)> {
        let slides = &self.presentation().slides;
        let slide = match &reference.sheet {
            Some(name) => {
                let name = name.to_lowercase();
                slides.iter().enumerate().find(|(index, slide)| {
                    let tab = slide.name.clone().unwrap_or_else(|| format!("Sheet{}", index + 1));
                    tab.to_lowercase() == name
                })?.1
            }
            None => self.slide(),
        };
        let (_, sheet) = sheet_of(slide)?;
        let range = match &reference.target {
            Target::Cells(range) => *range,
            Target::Name(name) if is_print_area(name) => sheet.print_area?,
            Target::Name(name) => {
                // A defined name: the range it stands for (the sheet's own name first).
                let index = self.presentation().workbook.as_ref().and_then(|w| w.sheets.iter().position(|s| s.slide == Some(slide.id)));
                let names = &self.presentation().workbook.as_ref()?.names;
                let defined = names.iter().find(|n| n.name.eq_ignore_ascii_case(name) && n.sheet.is_some() && n.sheet == index).or_else(|| names.iter().find(|n| n.name.eq_ignore_ascii_case(name) && n.sheet.is_none()))?;
                let target = parse_reference(&defined.formula)?;
                if matches!(target.target, Target::Name(_)) {
                    return None;
                }
                return self.resolve_reference(&target);
            }
        };
        let last_column = sheet.columns.len().checked_sub(1)? as u32;
        let last_row = sheet.rows.len().checked_sub(1)? as u32;
        Some((
            slide.id,
            CellRange {
                first_row: range.first_row.min(last_row),
                first_column: range.first_column.min(last_column),
                last_row: range.last_row.min(last_row),
                last_column: range.last_column.min(last_column),
            },
        ))
    }

    /// Select what `text` refers to, as the name box and ジャンプ do: the
    /// sheet shown if it names one, the range selected with its top-left
    /// cell as the selected one, and scrolled to the view's top left if it
    /// is out of view. False (nothing changed) if it refers to nothing.
    pub(crate) fn go_to_reference(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some((slide, range)) = parse_reference(text).and_then(|r| self.resolve_reference(&r)) else {
            return false;
        };
        self.commit_edit(window, cx);
        if slide != self.current_slide {
            self.select_slide(slide, window, cx);
        }
        self.selection = None;
        self.set_cell(range.first_column, range.first_row);
        if (range.last_column, range.last_row) != (range.first_column, range.first_row) {
            self.wb_mut().cell_ends.insert(slide, (range.last_column, range.last_row));
        }
        if self.workspace_bounds.is_some() {
            self.scroll_range_into_view(true);
        } else {
            self.wb_mut().jump.pending_scroll = true;
        }
        self.status = None;
        cx.notify();
        true
    }

    /// Ctrl+G or F5: open the ジャンプ dialog, its 参照先 empty.
    pub(crate) fn open_goto(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_workbook() || self.editing.is_some() {
            return cx.propagate();
        }
        self.wb_mut().jump.goto_open = true;
        self.wb_mut().jump.goto_error = None;
        self.wb().jump.goto_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    /// OK in the ジャンプ dialog: jump, or say the reference is wrong and
    /// keep the dialog open.
    fn confirm_goto(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.wb().jump.goto_input.read(cx).value().to_string();
        if self.go_to_reference(&text, window, cx) {
            self.close_goto(window, cx);
        } else {
            self.wb_mut().jump.goto_error = Some(crate::shell::i18n::text("_xlsx.invalid-reference").into());
            self.wb().jump.goto_input.update(cx, |input, cx| input.select_all(window, cx));
            cx.notify();
        }
    }

    fn close_goto(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.wb_mut().jump.goto_open = false;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    /// Esc in the name box or the ジャンプ dialog: leave it, the selection
    /// as it was. False if neither has the keys.
    pub(crate) fn cancel_cell_jump(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.is_workbook() {
            return false;
        }
        if self.completion_dismiss(cx) {
            return true;
        }
        if self.cancel_cell_edit(cx) {
            return true;
        }
        if self.wb().jump.goto_open {
            self.close_goto(window, cx);
            return true;
        }
        if self.wb().jump.name_box.read(cx).focus_handle(cx).is_focused(window) {
            // Shown again from the selection.
            self.wb_mut().jump.name_box_shown = String::new();
            self.focus_handle.focus(window, cx);
            cx.notify();
            return true;
        }
        false
    }

    /// The ジャンプ dialog, under the name box: 参照先 with OK and キャンセル.
    fn render_goto(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .id("goto")
            .absolute()
            .top(px(34.))
            .left(px(8.))
            .w(px(300.))
            .p_3()
            .gap_2()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_goto(window, cx)))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(crate::shell::i18n::text("_xlsx.go-to")))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().flex_shrink_0().child(crate::shell::i18n::text("_xlsx.reference")))
                    .child(div().flex_1().child(Input::new(&self.wb().jump.goto_input).small())),
            )
            .when_some(self.wb().jump.goto_error.clone(), |d, error| d.child(div().text_xs().text_color(theme.danger).child(error)))
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("goto-cancel")
                            .small()
                            .label(crate::shell::i18n::text("_shell.cancel"))
                            .on_click(cx.listener(|this, _, window, cx| this.close_goto(window, cx))),
                    )
                    .child(
                        Button::new("goto-ok")
                            .primary()
                            .small()
                            .label("OK")
                            .on_click(cx.listener(|this, _, window, cx| this.confirm_goto(window, cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{OPEN_END, Reference, Target, column_letters, data_edge, parse_reference, range_name};
    use ibis_doc_model::CellRange;

    fn cells(first_column: u32, first_row: u32, last_column: u32, last_row: u32) -> Target {
        Target::Cells(CellRange { first_row, first_column, last_row, last_column })
    }

    fn here(target: Target) -> Option<Reference> {
        Some(Reference { sheet: None, target })
    }

    #[test]
    fn references_are_read_as_excel_does() {
        assert_eq!(parse_reference("B12"), here(cells(1, 11, 1, 11)));
        assert_eq!(parse_reference("$B$12"), here(cells(1, 11, 1, 11)));
        assert_eq!(parse_reference("b$12"), here(cells(1, 11, 1, 11)));
        assert_eq!(parse_reference(" a201 "), here(cells(0, 200, 0, 200)));
        assert_eq!(parse_reference("A1:C5"), here(cells(0, 0, 2, 4)));
        // Corners in either order.
        assert_eq!(parse_reference("C5:A1"), here(cells(0, 0, 2, 4)));
        assert_eq!(parse_reference("XFD1048576"), here(cells(16_383, 1_048_575, 16_383, 1_048_575)));
        // Whole columns and rows.
        assert_eq!(parse_reference("B:D"), here(cells(1, 0, 3, OPEN_END)));
        assert_eq!(parse_reference("$3:$5"), here(cells(0, 2, OPEN_END, 4)));
    }

    #[test]
    fn references_may_name_a_sheet() {
        let on = |sheet: &str, target| Some(Reference { sheet: Some(sheet.into()), target });
        assert_eq!(parse_reference("Sheet2!A1"), on("Sheet2", cells(0, 0, 0, 0)));
        assert_eq!(parse_reference("'Sheet name'!$A$1:B2"), on("Sheet name", cells(0, 0, 1, 1)));
        assert_eq!(parse_reference("'売上 (2024)'!C3"), on("売上 (2024)", cells(2, 2, 2, 2)));
        assert_eq!(parse_reference("'It''s'!A1"), on("It's", cells(0, 0, 0, 0)));
        assert_eq!(parse_reference("集計!Print_Area"), on("集計", Target::Name("Print_Area".into())));
    }

    #[test]
    fn names_are_kept_and_the_rest_refused() {
        assert_eq!(parse_reference("Print_Area"), here(Target::Name("Print_Area".into())));
        for wrong in ["", "A1:B", "A:3", "A1:B2:C3", "!A1", "'Sheet!A1", "'Sheet'A1", "1A", "B 12", "$", "A1-"] {
            assert_eq!(parse_reference(wrong), None, "{wrong}");
        }
        // Past Excel's limits: no cells (only a name, which the workbook lacks).
        for past in ["A0", "XFE1", "A1048577"] {
            assert_eq!(parse_reference(past), here(Target::Name(past.into())), "{past}");
        }
    }

    #[test]
    fn ranges_are_named_as_in_the_name_box() {
        assert_eq!(range_name(CellRange { first_row: 11, first_column: 1, last_row: 11, last_column: 1 }), "B12");
        assert_eq!(range_name(CellRange { first_row: 0, first_column: 0, last_row: 4, last_column: 2 }), "A1:C5");
    }

    #[test]
    fn control_arrow_stops_at_the_data_region_edge() {
        // Rows 0-2 filled, 3-4 empty, 5-6 filled, 7 hidden, 8 filled, 9 empty.
        let sizes = [1, 1, 1, 1, 1, 1, 1, 0, 1, 1];
        let filled = |i: u32| [0, 1, 2, 5, 6, 8].contains(&i);
        // In a block: to its end.
        assert_eq!(data_edge(0, 1, &sizes, filled), 2);
        // At its end: to the next block.
        assert_eq!(data_edge(2, 1, &sizes, filled), 5);
        // Past the hidden row, as if it were not there.
        assert_eq!(data_edge(5, 1, &sizes, filled), 8);
        // From an empty cell, with nothing filled after it: to the edge.
        assert_eq!(data_edge(8, 1, &sizes, filled), 9);
        assert_eq!(data_edge(9, 1, &sizes, filled), 9);
        assert_eq!(data_edge(4, -1, &sizes, filled), 2);
        assert_eq!(data_edge(2, -1, &sizes, filled), 0);
    }

    #[test]
    fn column_letters_follow_excel() {
        assert_eq!(column_letters(0), "A");
        assert_eq!(column_letters(25), "Z");
        assert_eq!(column_letters(26), "AA");
        assert_eq!(column_letters(701), "ZZ");
        assert_eq!(column_letters(702), "AAA");
    }
}
