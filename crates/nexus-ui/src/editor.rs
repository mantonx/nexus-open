//! The preview editor: what a click, drag or key on the strip preview
//! means, and the edits it sends. Lives on the UI thread.
//!
//! The window shows the layout the daemon last sent. A drag changes only
//! what is drawn; letting go sends one edit. The daemon applies it, the
//! strip and the preview's frame change, and the layout is fetched again,
//! so nothing here has to guess what the daemon did.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use nexus_control::{Client, EditRequest, Layout, SignalEntry, Status};
use nexus_core::config::{MIN_WIDGET_WIDTH, Temperature, Widget};
use nexus_store::{Edit, Item, Row};
use slint::{ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode, VecModel, Weak};

use crate::geometry::{self, STRIP};
use crate::inspector::{self, Context, Field, Kind, Outcome, Reading, Values};
use crate::{AppWindow, Field as FieldRow, Zone};

/// How far a press must move before it is a drag rather than a click, in
/// strip pixels.
const DRAG_START: f32 = 3.0;
/// Key presses that resize within this long of each other are one edit,
/// so holding Shift+→ is one step to undo.
const KEY_SETTLE: Duration = Duration::from_millis(600);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    None,
    /// Pressed on widget `index` at `from`; not yet moved far enough.
    Pressed {
        index: usize,
        from: f32,
    },
    Moving {
        index: usize,
        from: f32,
        to: usize,
    },
    Resizing {
        edge: usize,
        from: f32,
    },
}

pub struct Editor {
    app: Weak<AppWindow>,
    layout: Option<Layout>,
    /// Which page and takeover the strip shows, from the last status.
    page: String,
    takeover: Option<String>,
    store_error: bool,
    /// Kept by ID so it follows the widget when it moves.
    selected: Option<i64>,
    drag: Drag,
    /// Widths drawn instead of the layout's while a resize is dragged or
    /// waiting for the daemon; dropped when the next layout arrives.
    preview: Option<Vec<u32>>,
    /// A keyboard resize waiting to settle: the edge last resized. Its
    /// widths are in `preview`, and every edge resized before it settles
    /// goes in the same edit.
    key_resize: Option<usize>,
    settle: Timer,
    problem: String,
    /// An edit is with the daemon. Until its answer and the new layout
    /// arrive, the layout here is out of date, so nothing new is sent.
    busy: bool,
    /// Field changes made while busy, applied once the layout arrives.
    queued: Vec<(String, String)>,
    /// After an add: the place of the new widget, to select it.
    select_after: Option<usize>,

    /// The inspector's rows, kept so a changed value updates its row in
    /// place rather than rebuilding every control.
    fields: Rc<VecModel<FieldRow>>,
    shown: Vec<Field>,
    /// The widget `shown` belongs to.
    fields_for: Option<i64>,
    /// Inspector values not sent yet (see `inspector::apply`), and why.
    drafts: Values,
    draft_hint: String,
    /// The last value sent for each field, so the row the person just
    /// typed in is not rebuilt (and does not lose focus) when the daemon
    /// confirms it.
    committed: HashMap<String, String>,
    signals: HashMap<String, Reading>,
}

pub type Shared = Rc<RefCell<Editor>>;

impl Editor {
    pub fn new(app: &AppWindow) -> Shared {
        Rc::new(RefCell::new(Editor {
            app: app.as_weak(),
            layout: None,
            page: String::new(),
            takeover: None,
            store_error: false,
            selected: None,
            drag: Drag::None,
            preview: None,
            key_resize: None,
            settle: Timer::default(),
            problem: String::new(),
            busy: false,
            queued: Vec::new(),
            select_after: None,
            fields: Rc::new(VecModel::default()),
            shown: Vec::new(),
            fields_for: None,
            drafts: Values::new(),
            draft_hint: String::new(),
            committed: HashMap::new(),
            signals: HashMap::new(),
        }))
    }

    /// Wires the window's callbacks to the editor.
    pub fn connect(editor: &Shared, app: &AppWindow) {
        let e = editor.clone();
        app.on_pressed(move |x, reach| e.borrow_mut().pressed(x, reach));
        let e = editor.clone();
        app.on_dragged(move |x| e.borrow_mut().dragged(x));
        let e = editor.clone();
        app.on_released(move |x| e.borrow_mut().released(x));
        let e = editor.clone();
        app.on_cancelled(move || e.borrow_mut().cancel());
        let e = editor.clone();
        app.on_edge_at(move |x, reach| {
            // Called while drawing; a drag in progress already holds the
            // editor, and the cursor does not change mid-drag anyway.
            e.try_borrow()
                .ok()
                .and_then(|e| e.edge_at(x, reach))
                .map_or(-1, |i| i as i32)
        });
        let e = editor.clone();
        app.on_select_step(move |step| e.borrow_mut().select_step(step));
        let e = editor.clone();
        app.on_move_step(move |step| e.borrow_mut().move_step(step));
        let e = editor.clone();
        app.on_resize_step(move |px| Editor::resize_step(&e, px));
        let e = editor.clone();
        app.on_deselect(move || {
            let mut e = e.borrow_mut();
            e.selected = None;
            e.show();
        });
        app.set_fields(ModelRc::from(editor.borrow().fields.clone()));
        let e = editor.clone();
        app.on_field_changed(move |key, value| {
            e.borrow_mut().field_changed(key.as_str(), value.as_str());
        });
        let e = editor.clone();
        app.on_add_widget(move || e.borrow_mut().add_widget());
        let e = editor.clone();
        app.on_delete_widget(move || e.borrow_mut().delete_widget());
        let e = editor.clone();
        app.on_undo(move || e.borrow_mut().step("undo"));
        let e = editor.clone();
        app.on_redo(move || e.borrow_mut().step("redo"));
    }

    /// A new status from the poll. Fetches the layout when it changed.
    pub fn status(editor: &Shared, status: &Status) {
        let mut e = editor.borrow_mut();
        let stale = e
            .layout
            .as_ref()
            .is_none_or(|l| l.revision != status.revision);
        let moved = e.page != status.page || e.takeover != status.takeover;
        let store_error = status.store_error.is_some();
        // Redrawn only on a change: rebuilding the outlines four times a
        // second would make a screen reader lose its place in the list.
        if moved || e.store_error != store_error {
            e.page = status.page.clone();
            e.takeover = status.takeover.clone();
            e.store_error = store_error;
            // The strip changed under a drag; whatever it was aimed at is
            // no longer shown.
            e.drag = Drag::None;
            e.preview = None;
            e.show();
        }
        if stale {
            fetch_layout(e.app.clone());
        }
    }

    fn set_layout(&mut self, layout: Layout) {
        self.layout = Some(layout);
        self.preview = None;
        if !matches!(self.drag, Drag::Pressed { .. }) {
            self.drag = Drag::None;
        }
        if let Some(at) = self.select_after.take() {
            self.selected = self.widget_ids().get(at).copied();
        }
        self.show();
        for (key, value) in std::mem::take(&mut self.queued) {
            self.field_changed(&key, &value);
        }
    }

    /// The latest signals, for the inspector's "Now …" lines.
    fn set_signals(&mut self, entries: Vec<SignalEntry>) {
        self.signals = entries
            .into_iter()
            .map(|e| {
                let reading = Reading {
                    text: e.value,
                    number: e.number,
                    unit: e.unit,
                };
                (e.name, reading)
            })
            .collect();
        if self.selected.is_some() {
            self.show_fields();
        }
    }

    fn set_problem(&mut self, problem: String) {
        self.problem = problem;
        self.show();
    }

    /// Index of the page on the strip in the layout.
    fn page_index(&self) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        layout.config.pages.iter().position(|p| p.name == self.page)
    }

    fn widgets(&self) -> &[Widget] {
        match (self.layout.as_ref(), self.page_index()) {
            (Some(l), Some(i)) => &l.config.pages[i].widgets,
            _ => &[],
        }
    }

    fn widget_ids(&self) -> &[i64] {
        match (self.layout.as_ref(), self.page_index()) {
            (Some(l), Some(i)) => &l.ids.pages[i].widgets,
            _ => &[],
        }
    }

    fn layout_widths(&self) -> Vec<u32> {
        self.widgets().iter().map(Widget::width).collect()
    }

    /// What is drawn: the preview during a resize, else the layout.
    fn widths(&self) -> Vec<u32> {
        self.preview.clone().unwrap_or_else(|| self.layout_widths())
    }

    fn editable(&self) -> bool {
        self.takeover.is_none() && !self.store_error && self.page_index().is_some()
    }

    fn selected_index(&self) -> Option<usize> {
        let id = self.selected?;
        self.widget_ids().iter().position(|&w| w == id)
    }

    fn zone_at(&self, x: f32) -> Option<usize> {
        let widths = self.widths();
        geometry::lefts(&widths)
            .iter()
            .zip(&widths)
            .position(|(&left, &w)| x >= left as f32 && x < (left + w) as f32)
    }

    fn edge_at(&self, x: f32, reach: f32) -> Option<usize> {
        if !self.editable() {
            return None;
        }
        let widths = self.widths();
        let mut right = 0;
        widths.iter().position(|w| {
            right += w;
            (x - right as f32).abs() <= reach
        })
    }

    /// `reach`: how near an edge counts as on it, in strip pixels.
    fn pressed(&mut self, x: f32, reach: f32) {
        if !self.editable() {
            return;
        }
        self.problem.clear();
        self.flush();
        self.drag = if let Some(edge) = self.edge_at(x, reach) {
            self.selected = self.widget_ids().get(edge).copied();
            Drag::Resizing { edge, from: x }
        } else if let Some(index) = self.zone_at(x) {
            self.selected = self.widget_ids().get(index).copied();
            Drag::Pressed { index, from: x }
        } else {
            self.selected = None;
            Drag::None
        };
        self.show();
    }

    fn dragged(&mut self, x: f32) {
        match self.drag {
            Drag::Pressed { index, from } if (x - from).abs() >= DRAG_START => {
                let to = self.drop_target(index, from, x);
                self.drag = Drag::Moving { index, from, to };
            }
            Drag::Moving { index, from, .. } => {
                let to = self.drop_target(index, from, x);
                self.drag = Drag::Moving { index, from, to };
            }
            Drag::Resizing { edge, from } => {
                let delta = (x - from).round() as i32;
                self.preview = Some(geometry::resize(&self.layout_widths(), edge, delta));
            }
            _ => return,
        }
        self.show_drag(x);
    }

    fn drop_target(&self, index: usize, from: f32, x: f32) -> usize {
        let widths = self.widths();
        let left = geometry::lefts(&widths)[index] as f32 + (x - from);
        geometry::drop_index(&widths, index, left)
    }

    fn released(&mut self, x: f32) {
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::Moving { index, from, .. } => {
                let to = self.drop_target(index, from, x);
                self.commit_move(index, to);
            }
            Drag::Resizing { edge, .. } => self.commit_resize(edge),
            _ => {}
        }
        self.show();
    }

    fn cancel(&mut self) {
        self.drag = Drag::None;
        self.preview = None;
        self.show();
    }

    fn select_step(&mut self, step: i32) {
        let n = self.widgets().len();
        if n == 0 {
            return;
        }
        let i = match self.selected_index() {
            Some(i) => (i as i32 + step).clamp(0, n as i32 - 1) as usize,
            None if step < 0 => n - 1,
            None => 0,
        };
        self.selected = self.widget_ids().get(i).copied();
        self.show();
    }

    /// Sends a keyboard resize that has not settled yet, so a click or a
    /// move does not lose it.
    fn flush(&mut self) {
        if let Some(edge) = self.key_resize {
            self.settle.stop();
            self.commit_resize(edge);
        }
    }

    fn move_step(&mut self, step: i32) {
        self.flush();
        let Some(i) = self.selected_index() else {
            return;
        };
        let to = i as i32 + step;
        if to >= 0 && (to as usize) < self.widgets().len() {
            self.commit_move(i, to as usize);
        }
    }

    /// Shift+arrow: resizes the selected widget's right edge now, and
    /// sends the edit once the keys have been still for a moment.
    fn resize_step(editor: &Shared, px: i32) {
        let mut e = editor.borrow_mut();
        let Some(edge) = e.selected_index() else {
            return;
        };
        if e.busy {
            return;
        }
        let widths = e.widths();
        e.preview = Some(geometry::resize(&widths, edge, px));
        e.key_resize = Some(edge);
        let later = Rc::downgrade(editor);
        e.settle.start(TimerMode::SingleShot, KEY_SETTLE, move || {
            if let Some(editor) = later.upgrade() {
                let mut e = editor.borrow_mut();
                if let Some(edge) = e.key_resize.take() {
                    e.commit_resize(edge);
                }
            }
        });
        e.show();
    }

    fn commit_move(&mut self, index: usize, to: usize) {
        let Some(&widget) = self.widget_ids().get(index) else {
            return;
        };
        if to != index {
            self.send(Edit::MoveWidget {
                widget,
                on: None,
                to,
            });
        }
    }

    /// Sends the preview's widths for the widgets either side of `edge`.
    fn commit_resize(&mut self, edge: usize) {
        self.key_resize = None;
        let Some(widths) = self.preview.clone() else {
            return;
        };
        let edits: Vec<Edit> = self
            .widgets()
            .iter()
            .zip(self.widget_ids())
            .zip(&widths)
            .filter(|((w, _), new)| w.width() != **new)
            .map(|((w, &id), &new)| Edit::SetWidget {
                widget: id,
                value: w.clone().with_width(new),
            })
            .collect();
        if edits.is_empty() {
            self.preview = None;
            return;
        }
        let name = self
            .widgets()
            .get(edge)
            .map_or_else(|| "widget".into(), describe);
        self.send(Edit::Batch {
            label: format!("Resize {name}"),
            edits,
        });
    }

    /// Sends an edit made against the layout on show, then fetches the
    /// layout again. A refusal is shown and the preview drops back.
    fn send(&mut self, edit: Edit) {
        let base = self.layout.as_ref().map(|l| l.revision);
        let line = format!(
            "edit {}",
            serde_json::to_string(&EditRequest { edit, base }).unwrap_or_default()
        );
        self.request(line);
    }

    fn step(&mut self, word: &str) {
        self.request(word.to_string());
    }

    fn request(&mut self, line: String) {
        if self.busy {
            return;
        }
        self.busy = true;
        let app = self.app.clone();
        thread::spawn(move || {
            let answer = Client::connect()
                .map_err(|e| format!("cannot reach nexusd: {e}"))
                .and_then(|mut c| {
                    c.request(&line)
                        .map_err(|e| format!("lost the connection to nexusd: {e}"))
                })
                .and_then(|reply| reply);
            let layout = fetch();
            let _ = app.upgrade_in_event_loop(move |_| {
                with_editor(|e| {
                    e.busy = false;
                    if let Err(problem) = answer {
                        e.set_problem(problem);
                    }
                    match layout {
                        Some(l) => e.set_layout(l),
                        None => {
                            e.preview = None;
                            e.show();
                        }
                    }
                });
            });
        });
    }

    /// Pushes the editor's state into the window.
    fn show(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let widths = self.widths();
        let zones: Vec<Zone> = geometry::lefts(&widths)
            .into_iter()
            .zip(&widths)
            .zip(self.widgets())
            .map(|((x, &width), w)| Zone {
                x: x as i32,
                width: width as i32,
                name: describe(w).into(),
            })
            .collect();
        app.set_zones(ModelRc::new(VecModel::from(zones)));
        app.set_editable(self.editable());
        let selected = self.selected_index();
        app.set_selected(selected.map_or(-1, |i| i as i32));
        app.set_problem(self.problem.as_str().into());
        app.set_undo_label(label(self.layout.as_ref().and_then(|l| l.undo.as_deref())));
        app.set_redo_label(label(self.layout.as_ref().and_then(|l| l.redo.as_deref())));
        if !matches!(self.drag, Drag::Moving { .. }) {
            app.set_ghost_width(0);
        }
        app.set_caption(self.caption(&widths, selected).into());
        self.show_fields();
    }

    /// While moving: the dragged widget follows the pointer and the gap it
    /// would drop into is marked.
    fn show_drag(&mut self, x: f32) {
        self.show();
        let (Drag::Moving { index, from, to }, Some(app)) = (self.drag, self.app.upgrade()) else {
            return;
        };
        let widths = self.widths();
        let left = geometry::lefts(&widths)[index] as f32 + (x - from);
        app.set_ghost_x(left);
        app.set_ghost_width(widths[index] as i32);
        app.set_drop_x(geometry::drop_x(&widths, index, to) as i32);
    }

    fn caption(&self, widths: &[u32], selected: Option<usize>) -> String {
        if let Some(t) = &self.takeover {
            return format!("“{t}” is showing over the page; edit again once it ends.");
        }
        if self.store_error {
            return "The store cannot be written, so the layout cannot be changed. Run nexusctl doctor.".into();
        }
        if self.layout.is_some() && self.page_index().is_none() {
            return String::new();
        }
        let widgets = self.widgets();
        match (self.drag, selected) {
            (Drag::Resizing { edge, .. }, _) | (_, Some(edge)) if self.preview.is_some() => {
                let mut parts: Vec<String> = [edge, edge + 1]
                    .into_iter()
                    .filter_map(|i| Some(format!("{} {} px", describe(widgets.get(i)?), widths[i])))
                    .collect();
                if edge + 1 == widgets.len() {
                    let free = STRIP - widths.iter().sum::<u32>();
                    parts.push(format!("{free} px free"));
                }
                parts.join("  ·  ")
            }
            (Drag::Moving { index, to, .. }, _) => format!(
                "Moving {} to place {} of {}",
                describe(&widgets[index]),
                to + 1,
                widgets.len()
            ),
            (_, Some(i)) => format!(
                "{}, {} px wide, {} of {}. Drag to move it, drag its right edge to resize.",
                describe(&widgets[i]),
                widths[i],
                i + 1,
                widgets.len()
            ),
            _ if widgets.is_empty() => String::new(),
            _ => "Click a widget to select it.".into(),
        }
    }
}

/// The inspector: the selected widget's fields, and what changing one
/// sends.
impl Editor {
    fn context(&self) -> Context {
        let Some(layout) = &self.layout else {
            return Context::default();
        };
        Context {
            pages: layout.config.pages.iter().map(|p| p.name.clone()).collect(),
            fahrenheit: layout.config.settings.temperature == Temperature::Fahrenheit,
            signals: self.signals.clone(),
        }
    }

    fn note(&self, index: usize) -> String {
        match (&self.layout, self.page_index()) {
            (Some(l), Some(p)) => l.notes.page(p).widget(index).to_string(),
            _ => String::new(),
        }
    }

    /// Puts the selected widget's fields in the window. Rows whose value
    /// changed are rebuilt so their controls show it, except a row whose
    /// new value is the one just sent from it.
    fn show_fields(&mut self) {
        let selected = self.selected_index().filter(|_| self.editable());
        let id = selected.and_then(|i| self.widget_ids().get(i).copied());
        if id != self.fields_for {
            self.fields_for = id;
            self.drafts.clear();
            self.draft_hint.clear();
            self.committed.clear();
            self.shown.clear();
            self.fields.set_vec(Vec::new());
        }
        let fields = match selected {
            Some(i) => {
                let ctx = self.context();
                let mut values = inspector::flatten(&self.widgets()[i], &ctx);
                values.extend(self.drafts.clone());
                let mut fields = inspector::fields(&values, &ctx);
                fields.push(Field {
                    key: "note".into(),
                    label: "Note".into(),
                    kind: Kind::Note,
                    value: self.note(i),
                    help: "For you: kept with the widget, and shown as a comment in nexusctl edit."
                        .into(),
                });
                fields
            }
            None => Vec::new(),
        };
        let same_keys = fields.len() == self.shown.len()
            && fields.iter().zip(&self.shown).all(|(a, b)| a.key == b.key);
        if !same_keys {
            self.fields
                .set_vec(fields.iter().map(row).collect::<Vec<_>>());
        } else {
            for (i, (new, old)) in fields.iter().zip(&self.shown).enumerate() {
                if new == old {
                    continue;
                }
                let just_sent = self.committed.get(&new.key) == Some(&new.value);
                if new.value != old.value && !just_sent {
                    self.fields.remove(i);
                    self.fields.insert(i, row(new));
                } else {
                    self.fields.set_row_data(i, row(new));
                }
            }
        }
        self.shown = fields;
        if let Some(app) = self.app.upgrade() {
            app.set_draft_hint(self.draft_hint.as_str().into());
        }
    }

    fn field_changed(&mut self, key: &str, value: &str) {
        if self.busy {
            self.queued.push((key.into(), value.into()));
            return;
        }
        let Some(i) = self.selected_index() else {
            return;
        };
        let id = self.widget_ids()[i];
        let widget = self.widgets()[i].clone();
        self.committed.insert(key.into(), value.into());
        self.problem.clear();
        match key {
            "width" => match value.trim().parse::<u32>() {
                Ok(new) => {
                    let delta = new as i32 - widget.width() as i32;
                    self.preview = Some(geometry::resize(&self.layout_widths(), i, delta));
                    self.commit_resize(i);
                }
                Err(_) => self.problem = "The width must be a whole number of pixels.".into(),
            },
            "note" => {
                if value != self.note(i) {
                    self.send(Edit::SetNote {
                        item: Item::Widget(id),
                        note: value.into(),
                    });
                }
            }
            _ => {
                // A draft is finished by the fields it is about; a change
                // elsewhere goes out on its own and leaves the draft be.
                let group = |k: &str| k.split('.').next().unwrap_or(k).to_string();
                let in_draft = self.drafts.keys().any(|k| group(k) == group(key));
                let drafts = if in_draft {
                    self.drafts.clone()
                } else {
                    Values::new()
                };
                match inspector::apply(&widget, &drafts, key, value, &self.context()) {
                    Outcome::Send(new) => {
                        if in_draft {
                            self.drafts.clear();
                            self.draft_hint.clear();
                        }
                        self.send(Edit::Batch {
                            label: format!("Change {}", describe(&widget)),
                            edits: vec![Edit::SetWidget {
                                widget: id,
                                value: new,
                            }],
                        });
                    }
                    Outcome::Draft(draft, why) => {
                        if in_draft {
                            self.drafts = draft;
                        } else {
                            self.drafts.extend(draft);
                        }
                        self.draft_hint = why;
                    }
                    Outcome::Invalid(why) => self.problem = why,
                    Outcome::Unchanged => {
                        if in_draft {
                            self.drafts.clear();
                            self.draft_hint.clear();
                        }
                    }
                }
            }
        }
        self.show();
    }

    /// Adds a clock after the selected widget (or at the end) in the
    /// empty strip; with none left, splits the selected widget in two.
    fn add_widget(&mut self) {
        let (Some(layout), Some(p)) = (&self.layout, self.page_index()) else {
            return;
        };
        let page = layout.ids.pages[p].id;
        let widths = self.layout_widths();
        let free = STRIP - widths.iter().sum::<u32>();
        let selected = self.selected_index();
        let at = selected.map_or(widths.len(), |i| i + 1);
        let clock = |width| Widget::Clock {
            width,
            tap: None,
            hold: None,
        };
        let mut edits = Vec::new();
        let width = if free >= MIN_WIDGET_WIDTH {
            free
        } else if let Some(i) = selected.filter(|&i| widths[i] >= 2 * MIN_WIDGET_WIDTH) {
            let half = widths[i] / 2;
            edits.push(Edit::SetWidget {
                widget: self.widget_ids()[i],
                value: self.widgets()[i].clone().with_width(widths[i] - half),
            });
            half
        } else {
            self.problem = format!(
                "There is no room on this page. Narrow a widget to leave {MIN_WIDGET_WIDTH} px free, or select one at least {} px wide to split it.",
                2 * MIN_WIDGET_WIDTH
            );
            self.show();
            return;
        };
        edits.push(Edit::AddWidget {
            on: Row::Page(page),
            at: Some(at),
            widget: clock(width),
        });
        self.select_after = Some(at);
        self.send(Edit::Batch {
            label: "Add widget".into(),
            edits,
        });
    }

    fn delete_widget(&mut self) {
        let Some(i) = self.selected_index() else {
            return;
        };
        let id = self.widget_ids()[i];
        let name = describe(&self.widgets()[i]);
        self.selected = None;
        self.send(Edit::Batch {
            label: format!("Delete {name}"),
            edits: vec![Edit::DeleteWidget { widget: id }],
        });
        self.show();
    }
}

/// A field as the window takes it.
fn row(f: &Field) -> FieldRow {
    let (kind, choices) = match &f.kind {
        Kind::Text => ("text", Vec::new()),
        Kind::Number => ("number", Vec::new()),
        Kind::Note => ("note", Vec::new()),
        Kind::Choice(options) => ("choice", options.clone()),
    };
    let index = choices
        .iter()
        .position(|c| c == &f.value)
        .map_or(-1, |i| i as i32);
    FieldRow {
        key: f.key.as_str().into(),
        label: f.label.as_str().into(),
        kind: kind.into(),
        value: f.value.as_str().into(),
        choices: ModelRc::new(VecModel::from(
            choices
                .into_iter()
                .map(SharedString::from)
                .collect::<Vec<_>>(),
        )),
        index,
        help: f.help.as_str().into(),
    }
}

fn label(s: Option<&str>) -> SharedString {
    s.unwrap_or_default().into()
}

/// A widget's name as people see it: its label, or what it is.
pub fn describe(w: &Widget) -> String {
    match w {
        Widget::Clock { .. } => "Clock".into(),
        Widget::Media { .. } => "Now playing".into(),
        Widget::Value { label, .. } | Widget::Rate { label, .. } | Widget::Slider { label, .. } => {
            if label.trim().is_empty() {
                match w {
                    Widget::Value { signal, .. } => signal.clone(),
                    Widget::Rate { .. } => "Network".into(),
                    _ => "Slider".into(),
                }
            } else {
                label.clone()
            }
        }
    }
}

thread_local! {
    /// The editor, reachable from closures posted to the UI thread.
    static EDITOR: RefCell<Option<Shared>> = const { RefCell::new(None) };
}

/// The latest signals, for the installed editor.
pub fn signals(entries: Vec<SignalEntry>) {
    let editor = EDITOR.with(|e| e.borrow().clone());
    if let Some(editor) = editor {
        editor.borrow_mut().set_signals(entries);
    }
}

/// A new status from the poll, for the installed editor.
pub fn status(status: &Status) {
    let editor = EDITOR.with(|e| e.borrow().clone());
    if let Some(editor) = editor {
        Editor::status(&editor, status);
    }
}

pub fn install(editor: &Shared) {
    EDITOR.with(|e| *e.borrow_mut() = Some(editor.clone()));
}

fn with_editor(f: impl FnOnce(&mut Editor)) {
    EDITOR.with(|e| {
        if let Some(editor) = e.borrow().as_ref() {
            f(&mut editor.borrow_mut());
        }
    });
}

/// Asks for the layout; `None` if the daemon cannot answer.
fn fetch() -> Option<Layout> {
    let mut client = Client::connect().ok()?;
    let data = client.request("layout").ok()?.ok()?;
    serde_json::from_value(data).ok()
}

fn fetch_layout(app: Weak<AppWindow>) {
    thread::spawn(move || {
        if let Some(layout) = fetch() {
            let _ = app.upgrade_in_event_loop(move |_| {
                with_editor(|e| e.set_layout(layout));
            });
        }
    });
}
