//! The preview editor: what a click, drag or key on the strip preview
//! means, and the edits it sends. Lives on the UI thread.
//!
//! The window shows the layout the daemon last sent. A drag changes only
//! what is drawn; letting go sends one edit. The daemon applies it, the
//! strip and the preview's frame change, and the layout is fetched again,
//! so nothing here has to guess what the daemon did.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use std::sync::mpsc::Sender;

use nexus_control::{Client, EditRequest, ImportRequest, Layout, LayoutEntry, SignalEntry, Status};
use nexus_core::config::{Config, STARTERS, Takeover, Temperature, Widget};
use nexus_store::{Edit, Item, Row};
use slint::{
    ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, Timer,
    TimerMode, VecModel, Weak,
};

use crate::files;
use crate::gallery;
use crate::geometry::{self, STRIP};
use crate::inspector::{self, Context, Field, Kind, Missing, Outcome, Reading, Values};
use crate::layouts;
use crate::names;
use crate::news;
use crate::settings;
use crate::signals;
use crate::simulate;
use crate::takeovers;
use crate::thumbs::{self, Done, Job};
use crate::{
    AppWindow, FeedRow, Field as FieldRow, LayoutRow, PickRow, SignalRow, Starter, StripRow, Tile,
    Trigger, Zone,
};

/// A takeover being edited is shown on the strip for this long, and asked
/// for again every `LEASE_RENEW`. If the window goes away, it ends soon.
const LEASE: f64 = 15.0;
const LEASE_RENEW: Duration = Duration::from_secs(5);

/// The row of widgets being edited.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shown {
    Page(usize),
    Takeover(usize),
}

/// What the settings under the strip belong to.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Form {
    Widget(i64),
    Takeover(i64),
    NewTakeover,
}

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
    /// After adding a page: its place, to show it on the strip.
    show_after: Option<usize>,
    /// A rename sent: the old and new name, so the editor keeps following
    /// the page before the status poll reports the new name.
    renamed: Option<(String, String)>,
    /// The takeover being edited, by name: shown on the strip on a lease.
    editing: Option<String>,
    lease: Timer,
    /// A takeover being added: its form, until it has what it needs.
    adding: Option<Values>,
    /// After adding a takeover: its name, to start editing it.
    edit_after: Option<String>,
    /// A click on a strip not being edited: once it is on the strip, the
    /// widget at this x (strip pixels) is selected. Keyed by "page:Name"
    /// or "takeover:Name".
    select_at: Option<(String, f32)>,
    /// Whether nexusd answers; editing waits while it does not.
    connected: bool,
    /// What the last file action did, shown as the caption until the
    /// next change.
    notice: String,
    /// A request that could not reach nexusd, sent again once it answers.
    unsent: Option<String>,
    /// Changes made while one was still on its way, in order.
    waiting: VecDeque<Waiting>,
    /// Try mode: the preview acts as the strip.
    trying: bool,
    /// In Try mode, where and when the current press began.
    touch: Option<(f32, std::time::Instant)>,

    /// The inspector's rows, kept so a changed value updates its row in
    /// place rather than rebuilding every control.
    fields: Rc<VecModel<FieldRow>>,
    shown: Vec<Field>,
    /// What `shown` belongs to.
    fields_for: Option<Form>,
    /// Inspector values not sent yet (see `inspector::apply`), and why.
    drafts: Values,
    draft_hint: String,
    /// The last value sent for each field, so the row the person just
    /// typed in is not rebuilt (and does not lose focus) when the daemon
    /// confirms it.
    committed: HashMap<String, String>,
    signals: HashMap<String, Reading>,

    /// The Signals drawer's rows, updated in place.
    signal_rows: Rc<VecModel<SignalRow>>,
    /// The picker: the field it was opened for, what is typed, its rows.
    picking: Option<String>,
    pick_filter: String,
    picks: Rc<VecModel<PickRow>>,
    triggers: Rc<VecModel<Trigger>>,

    /// The storyboard's rows, updated in place so the settings under the
    /// strip being edited keep their focus.
    rows: Rc<VecModel<StripRow>>,
    /// Pictures of every page and takeover, drawn by `thumbs`.
    page_pictures: Vec<Image>,
    takeover_pictures: Vec<Image>,
    thumbs: Option<Sender<Job>>,
    epoch: u64,
    /// The gallery's tiles and starters' pictures.
    tiles: Rc<VecModel<Tile>>,
    starters: Rc<VecModel<Starter>>,
    /// While a resize is dragged: the x it snapped to.
    snap_x: Option<u32>,
    /// Whether the first-run choice was offered already.
    first_run_seen: bool,
    /// Feeds' titles, from checking them when they were added.
    feed_titles: HashMap<String, String>,
    /// The latest feed check: its number (an older one's answer is
    /// dropped), and an address that failed but may be added anyway.
    feed_try: u64,
    feed_pending: Option<String>,
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
            show_after: None,
            renamed: None,
            editing: None,
            lease: Timer::default(),
            adding: None,
            edit_after: None,
            select_at: None,
            connected: false,
            notice: String::new(),
            unsent: None,
            waiting: VecDeque::new(),
            trying: false,
            touch: None,
            fields: Rc::new(VecModel::default()),
            shown: Vec::new(),
            fields_for: None,
            drafts: Values::new(),
            draft_hint: String::new(),
            committed: HashMap::new(),
            signals: HashMap::new(),
            signal_rows: Rc::new(VecModel::default()),
            picking: None,
            pick_filter: String::new(),
            picks: Rc::new(VecModel::default()),
            triggers: Rc::new(VecModel::default()),
            rows: Rc::new(VecModel::default()),
            page_pictures: Vec::new(),
            takeover_pictures: Vec::new(),
            thumbs: None,
            epoch: 0,
            tiles: Rc::new(VecModel::default()),
            starters: Rc::new(VecModel::default()),
            snap_x: None,
            first_run_seen: crate::first_run::seen(),
            feed_titles: HashMap::new(),
            feed_try: 0,
            feed_pending: None,
        }))
    }

    /// Wires the window's callbacks to the editor.
    pub fn connect(editor: &Shared, app: &AppWindow) {
        macro_rules! on {
            ($setter:ident, |$e:ident $(, $arg:ident)*| $body:expr) => {{
                let shared = editor.clone();
                app.$setter(move |$($arg),*| {
                    let mut $e = shared.borrow_mut();
                    $body
                });
            }};
        }
        on!(on_pressed, |e, x, reach| e.pressed(x, reach));
        on!(on_dragged, |e, x| e.dragged(x));
        on!(on_released, |e, x| e.released(x));
        on!(on_cancelled, |e| e.cancel());
        let e = editor.clone();
        app.on_edge_at(move |x, reach| {
            // Called while drawing; a drag in progress already holds the
            // editor, and the cursor does not change mid-drag anyway.
            e.try_borrow()
                .ok()
                .and_then(|e| e.edge_at(x, reach))
                .map_or(-1, |i| i as i32)
        });
        on!(on_select_step, |e, step| e.select_step(step));
        on!(on_move_step, |e, step| e.move_step(step));
        let e = editor.clone();
        app.on_resize_step(move |px| Editor::resize_step(&e, px));
        on!(on_deselect, |e| {
            e.selected = None;
            e.show();
        });
        app.set_fields(ModelRc::from(editor.borrow().fields.clone()));
        app.set_rows(ModelRc::from(editor.borrow().rows.clone()));
        app.set_picks(ModelRc::from(editor.borrow().picks.clone()));
        app.set_triggers(ModelRc::from(editor.borrow().triggers.clone()));
        app.set_signal_rows(ModelRc::from(editor.borrow().signal_rows.clone()));
        app.set_tiles(ModelRc::from(editor.borrow().tiles.clone()));
        app.set_starters(ModelRc::from(editor.borrow().starters.clone()));
        let groups: Vec<SharedString> = gallery::GROUPS
            .iter()
            .map(|g| crate::i18n::shown(g).into())
            .collect();
        app.set_groups(ModelRc::new(VecModel::from(groups)));
        on!(on_field_changed, |e, key, value| e
            .field_changed(key.as_str(), value.as_str()));
        on!(on_field_chosen, |e, key, index| e
            .field_chosen(key.as_str(), index));
        on!(on_open_picker, |e, key| e.open_picker(key.as_str()));
        on!(on_picker_filter, |e, text| {
            e.pick_filter = text.into();
            e.show_picks();
        });
        on!(on_picked, |e, key, id| e.picked(key.as_str(), id.as_str()));
        on!(on_remove_trigger, |e, id| e.remove_trigger(id.as_str()));
        on!(on_open_gallery, |e| e.open_gallery());
        on!(on_add_tile, |e, i| e.add_tile(i as usize));
        on!(on_delete_widget, |e| e.delete_widget());
        on!(on_equal_widths, |e| e.equal_widths());
        on!(on_strip_clicked, |e, kind, index, x| e.strip_clicked(
            kind.as_str(),
            index as usize,
            x
        ));
        on!(on_add_page, |e| e.add_page());
        on!(on_rename_page, |e, name| e.rename_page(name.as_str()));
        on!(on_move_page, |e, from, to| e
            .move_page(from as usize, to as usize));
        on!(on_delete_page, |e| e.delete_page());
        on!(on_add_takeover, |e| e.add_takeover());
        on!(on_create_takeover, |e| e.create_takeover());
        on!(on_delete_takeover, |e| e.delete_takeover());
        on!(on_use_layout, |e, i| e.use_layout(i as usize));
        on!(on_delete_layout, |e, i| e.delete_layout(i as usize));
        on!(on_rename_layout, |e, name| e.rename_layout(name.as_str()));
        on!(on_duplicate_layout, |e| e.add_layout(layouts::COPY));
        on!(on_new_layout, |e, i| e.new_layout(i));
        on!(on_open_starters, |e| e.open_starters());
        on!(on_choose_starter, |e, i| {
            e.first_run_done();
            e.new_layout(i);
        });
        on!(on_dismiss_first_run, |e| e.first_run_done());
        on!(on_set_pref, |e, key, value| e
            .set_pref(key.as_str(), value.as_str()));
        on!(on_add_feed, |e, text| e.add_feed(text.as_str()));
        on!(on_add_feed_anyway, |e| e.add_feed_anyway());
        on!(on_remove_feed, |e, i| e.remove_feed(i as usize));
        on!(on_move_feed, |e, from, to| e
            .move_feed(from as usize, to as usize));
        on!(on_news_choice, |e, key, i| e
            .news_choice(key.as_str(), i as usize));
        app.on_import_layout(import_layout);
        app.on_export_layout(export_layout);
        on!(on_set_trying, |e, on| e.set_trying(on));
        on!(on_try_press, |e, x| e.touch =
            Some((x, std::time::Instant::now())));
        on!(on_try_release, |e, x| e.try_release(x));
        on!(on_try_cancel, |e| e.touch = None);
        on!(on_undo, |e| e.step("undo"));
        on!(on_redo, |e| e.step("redo"));
    }

    /// Where pictures of strips are drawn (see `thumbs`).
    pub fn set_thumbs(&mut self, jobs: Sender<Job>) {
        self.thumbs = Some(jobs);
        self.draw_layout();
    }

    /// nexusd answers, or stopped answering.
    pub fn set_connected(&mut self, connected: bool) {
        if self.connected != connected {
            self.connected = connected;
            if !connected {
                self.drag = Drag::None;
                self.preview = None;
            }
            self.show();
        }
    }

    /// A new status from the poll. Fetches the layout when it changed.
    pub fn status(editor: &Shared, status: &Status) {
        let mut e = editor.borrow_mut();
        // nexusd answers again: send what it missed.
        if !e.busy
            && let Some(line) = e.unsent.take()
        {
            e.request(line);
        }
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
            e.arrived();
            e.show();
        }
        if stale {
            fetch_layout(e.app.clone());
        }
    }

    fn set_layout(&mut self, layout: Layout) {
        if let Some((old, new)) = self.renamed.take()
            && self.page == old
            && layout.config.pages.iter().any(|p| p.name == new)
        {
            self.page = new;
        }
        crate::set_accent(layout.config.settings.accent);
        self.layout = Some(layout);
        self.show_prefs();
        // The units may have changed, or this is the first layout.
        self.show_signals();
        self.draw_layout();
        self.offer_first_run();
        self.preview = None;
        if !matches!(self.drag, Drag::Pressed { .. }) {
            self.drag = Drag::None;
        }
        if let Some(at) = self.select_after.take() {
            self.selected = self.widget_ids().get(at).copied();
        }
        if let Some(at) = self.show_after.take() {
            request_page(at);
        }
        if let Some(name) = self.edit_after.take() {
            self.start_editing(name);
        }
        // Gone, by undo or from elsewhere: back to the page.
        if self.editing.is_some() && self.editing_takeover().is_none() {
            self.stop_editing();
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
        if self.selected.is_some() || self.editing.is_some() {
            self.show_fields();
        }
        self.show_signals();
        self.show_picks();
        self.draw_layout();
    }

    /// Puts the Signals drawer's rows in the window, in place while the
    /// names stay the same, which is every second.
    fn show_signals(&mut self) {
        // Values are shown in the layout's units; readings arrive before
        // the layout does, and would show in °C for a moment.
        if self.layout.is_none() {
            return;
        }
        let rows: Vec<SignalRow> = signals::rows(&self.signals, "", &self.context())
            .into_iter()
            .map(|r| {
                let named = names::named(&r.name);
                SignalRow {
                    id: r.name.into(),
                    name: named.name.into(),
                    icon: named.icon.to_string().into(),
                    value: r.value.into(),
                }
            })
            .collect();
        update_in_place(&self.signal_rows, rows, |a, b| a.id == b.id);
    }

    /// The picker opened on field `key`.
    fn open_picker(&mut self, key: &str) {
        self.picking = Some(key.to_string());
        self.pick_filter.clear();
        self.show_picks();
    }

    /// The picker's rows: every signal published, and the known ones even
    /// before they are, matching what is typed; known ones first.
    fn show_picks(&mut self) {
        let Some(key) = self.picking.clone() else {
            return;
        };
        if key == "icon" {
            let now = self
                .shown
                .iter()
                .find(|f| f.key == key)
                .map(|f| f.value.trim().to_string())
                .unwrap_or_default();
            let filter = self.pick_filter.trim().to_lowercase();
            let own = PickRow {
                chosen: now.is_empty(),
                id: SharedString::new(),
                name: "The widget's own".into(),
                icon: self
                    .own_icon()
                    .map(|c| c.to_string())
                    .unwrap_or_default()
                    .into(),
                value: SharedString::new(),
            };
            let rows: Vec<PickRow> = std::iter::once(own)
                .filter(|_| filter.is_empty())
                .chain(
                    nexus_core::icons::NAMES
                        .iter()
                        .filter(|(n, _)| filter.is_empty() || n.replace('-', " ").contains(&filter))
                        .map(|(n, c)| PickRow {
                            chosen: now == *n,
                            id: (*n).into(),
                            name: icon_title(n).into(),
                            icon: c.to_string().into(),
                            value: SharedString::new(),
                        }),
                )
                .collect();
            update_in_place(&self.picks, rows, |a, b| a.id == b.id);
            return;
        }
        let ctx = self.context();
        let chosen: Vec<String> = if key == takeovers::ON_CHANGE {
            self.takeover_values()
                .map(|v| takeovers::triggers(&v))
                .unwrap_or_default()
        } else {
            self.shown
                .iter()
                .find(|f| f.key == key)
                .map(|f| vec![f.value.clone()])
                .unwrap_or_default()
        };
        let mut ids: Vec<String> = self.signals.keys().cloned().collect();
        for known in names::known() {
            if !ids.iter().any(|i| i == known) {
                ids.push(known.into());
            }
        }
        ids.retain(|id| names::matches(id, &self.pick_filter));
        ids.sort_by_key(|id| names::order(id));
        let rows: Vec<PickRow> = ids
            .into_iter()
            .map(|id| {
                let named = names::named(&id);
                let value = self
                    .signals
                    .get(&id)
                    .map(|r| inspector::show_reading(r, &ctx))
                    .unwrap_or_else(|| "no reading yet".into());
                PickRow {
                    chosen: chosen.contains(&id),
                    id: id.into(),
                    name: named.name.into(),
                    icon: named.icon.to_string().into(),
                    value: value.into(),
                }
            })
            .collect();
        update_in_place(&self.picks, rows, |a, b| a.id == b.id);
    }

    /// A signal picked for field `key`.
    fn picked(&mut self, key: &str, id: &str) {
        self.picking = None;
        let value = if key == takeovers::ON_CHANGE {
            let list = self
                .takeover_values()
                .and_then(|v| v.get(key).cloned())
                .unwrap_or_default();
            takeovers::add_signal(&list, id)
        } else {
            id.to_string()
        };
        self.field_changed(key, &value);
        // Not typed in the row, so the row must show the new name when the
        // daemon confirms it (see `committed`).
        self.committed.remove(key);
    }

    /// A trigger chip's cross.
    fn remove_trigger(&mut self, id: &str) {
        let list = self
            .takeover_values()
            .and_then(|v| v.get(takeovers::ON_CHANGE).cloned())
            .unwrap_or_default();
        self.field_changed(takeovers::ON_CHANGE, &takeovers::remove_signal(&list, id));
        self.committed.remove(takeovers::ON_CHANGE);
    }

    fn set_problem(&mut self, problem: String) {
        self.problem = problem;
        self.show();
    }

    /// The selected widget's own icon, as the window draws it.
    fn own_icon(&self) -> Option<char> {
        let i = self.selected_index()?;
        self.widgets().get(i).map(widget_icon)
    }

    /// Index of the page on the strip in the layout.
    fn page_index(&self) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        layout.config.pages.iter().position(|p| p.name == self.page)
    }

    /// The row being edited: the takeover chosen, once the strip shows
    /// it; else the page, unless a takeover covers it.
    fn shown_row(&self) -> Option<Shown> {
        let l = self.layout.as_ref()?;
        match &self.editing {
            Some(name) => {
                if self.takeover.as_ref() != Some(name) {
                    return None;
                }
                l.config
                    .takeovers
                    .iter()
                    .position(|t| &t.name == name)
                    .map(Shown::Takeover)
            }
            None if self.takeover.is_some() => None,
            None => self.page_index().map(Shown::Page),
        }
    }

    /// The store's ID of the row being edited.
    fn row_id(&self) -> Option<Row> {
        let l = self.layout.as_ref()?;
        Some(match self.shown_row()? {
            Shown::Page(i) => Row::Page(l.ids.pages[i].id),
            Shown::Takeover(i) => Row::Takeover(l.ids.takeovers[i].id),
        })
    }

    fn widgets(&self) -> &[Widget] {
        match (self.layout.as_ref(), self.shown_row()) {
            (Some(l), Some(Shown::Page(i))) => &l.config.pages[i].widgets,
            (Some(l), Some(Shown::Takeover(i))) => &l.config.takeovers[i].widgets,
            _ => &[],
        }
    }

    fn widget_ids(&self) -> &[i64] {
        match (self.layout.as_ref(), self.shown_row()) {
            (Some(l), Some(Shown::Page(i))) => &l.ids.pages[i].widgets,
            (Some(l), Some(Shown::Takeover(i))) => &l.ids.takeovers[i].widgets,
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
        self.connected && !self.trying && !self.store_error && self.shown_row().is_some()
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
                let widths = self.layout_widths();
                let (delta, snapped) = geometry::snap(&widths, edge, (x - from).round() as i32);
                let preview = geometry::resize(&widths, edge, delta);
                self.snap_x = snapped.then(|| geometry::lefts(&preview)[edge] + preview[edge]);
                self.preview = Some(preview);
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
        self.snap_x = None;
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
        self.snap_x = None;
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
        // Kept as the edit, not the line: the revision it is made against
        // is the one current when it is sent, after the one before it.
        if self.busy {
            self.waiting.push_back(Waiting::Edit(Box::new(edit)));
            return;
        }
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
            self.waiting.push_back(Waiting::Line(line));
            return;
        }
        self.busy = true;
        // A problem lasts until the next change is tried.
        self.problem.clear();
        self.notice.clear();
        let app = self.app.clone();
        thread::spawn(move || {
            let answer = match Client::connect() {
                Err(_) => Err(Failure::Unsent),
                Ok(mut c) => match c.request(&line) {
                    Err(_) => Err(Failure::Lost),
                    Ok(reply) => reply.map_err(Failure::Refused),
                },
            };
            let layout = fetch();
            let _ = app.upgrade_in_event_loop(move |_| {
                with_editor(|e| {
                    e.busy = false;
                    match answer {
                        Ok(_) => {}
                        Err(Failure::Refused(problem)) => e.set_problem(problem),
                        Err(failure) => {
                            if keep(&failure, &line) {
                                e.unsent = Some(line);
                                e.set_problem(
                                    "nexusd cannot be reached. This change is kept and is sent once it answers again."
                                        .into(),
                                );
                            } else {
                                e.set_problem("The connection to nexusd was lost during this change; check whether it was made.".into());
                            }
                        }
                    }
                    match layout {
                        Some(l) => e.set_layout(l),
                        None => {
                            e.preview = None;
                            e.show();
                        }
                    }
                    // The next change, unless this one failed: then the
                    // ones after it may depend on it, so they are dropped
                    // and the problem says why.
                    if e.problem.is_empty() {
                        e.send_next();
                    } else {
                        e.waiting.clear();
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
        self.show_rows(&app);
        let rows: Vec<LayoutRow> = self
            .layout
            .iter()
            .flat_map(|l| {
                l.layouts.iter().map(|e| LayoutRow {
                    name: e.name.as_str().into(),
                    active: e.id == l.id,
                })
            })
            .collect();
        app.set_layouts(ModelRc::new(VecModel::from(rows)));
        app.set_layout_name(
            self.layout
                .as_ref()
                .map(|l| l.name.as_str())
                .unwrap_or_default()
                .into(),
        );
        app.set_page_name(self.page.as_str().into());
        app.set_editable(self.editable());
        let selected = self.selected_index();
        app.set_selected(selected.map_or(-1, |i| i as i32));
        app.set_problem(self.problem.as_str().into());
        app.set_undo_label(label(self.layout.as_ref().and_then(|l| l.undo.as_deref())));
        app.set_redo_label(label(self.layout.as_ref().and_then(|l| l.redo.as_deref())));
        if !matches!(self.drag, Drag::Moving { .. }) {
            app.set_ghost_width(0);
        }
        app.set_snap_x(self.snap_x.map_or(-1, |x| x as i32));
        // Room for at least a key, without taking any widget below
        // where it reads.
        let mins: Vec<u32> = self.widgets().iter().map(geometry::readable).collect();
        let room = geometry::make_room_above(
            &self.layout_widths(),
            &mins,
            nexus_core::config::MIN_WIDGET_WIDTH,
            nexus_core::config::MIN_WIDGET_WIDTH,
        )
        .is_some();
        app.set_can_add(room);
        app.set_add_reason(if room {
            SharedString::new()
        } else {
            "Every widget here is as narrow as it reads, so nothing more fits. Remove one, or add a page.".into()
        });
        app.set_caption(self.caption(&widths, selected).into());
        self.show_fields();
    }

    /// The storyboard: every page, then every takeover, each with its
    /// picture, and which one is on the strip and which is being edited.
    fn show_rows(&mut self, app: &AppWindow) {
        let Some(l) = &self.layout else {
            self.rows.set_vec(Vec::new());
            return;
        };
        let page_now = self.page_index();
        let picture = |list: &[Image], i: usize| list.get(i).cloned().unwrap_or_default();
        let pages = &l.config.pages;
        let takeovers = &l.config.takeovers;
        let mut rows: Vec<StripRow> = pages
            .iter()
            .enumerate()
            .map(|(i, p)| StripRow {
                kind: "page".into(),
                index: i as i32,
                name: p.name.as_str().into(),
                frame: picture(&self.page_pictures, i),
                summary: match p.widgets.len() {
                    0 => "Empty".into(),
                    1 => "1 widget".into(),
                    n => format!("{n} widgets").into(),
                },
                live: self.takeover.is_none() && page_now == Some(i),
                active: self.editing.is_none() && self.adding.is_none() && page_now == Some(i),
                empty: p.widgets.is_empty(),
                first_takeover: false,
                last_page: i + 1 == pages.len(),
            })
            .collect();
        for (i, t) in takeovers.iter().enumerate() {
            rows.push(StripRow {
                kind: "takeover".into(),
                index: i as i32,
                name: t.name.as_str().into(),
                frame: picture(&self.takeover_pictures, i),
                summary: takeover_summary(t).into(),
                live: self.takeover.as_ref() == Some(&t.name),
                active: self.editing.as_ref() == Some(&t.name),
                empty: t.widgets.is_empty(),
                first_takeover: i == 0,
                last_page: false,
            });
        }
        if let Some(v) = &self.adding {
            rows.push(StripRow {
                kind: "new-takeover".into(),
                index: takeovers.len() as i32,
                name: v
                    .get(takeovers::NAME)
                    .map(String::as_str)
                    .unwrap_or("New takeover")
                    .into(),
                frame: Image::default(),
                summary: "New: not added yet".into(),
                live: false,
                active: true,
                empty: true,
                first_takeover: takeovers.is_empty(),
                last_page: false,
            });
        }
        app.set_page_count(pages.len() as i32);
        app.set_takeover_count((takeovers.len() + usize::from(self.adding.is_some())) as i32);
        update_in_place(&self.rows, rows, |_, _| true);
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
        if self.trying {
            return "Trying it: click, hold or drag on the strip above and each is sent to the real strip as a touch. Switch to Edit to change things.".into();
        }
        if !self.connected {
            return "Editing waits until the service answers again.".into();
        }
        if !self.notice.is_empty() {
            return self.notice.clone();
        }
        if self.adding.is_some() {
            return "Choose the reading whose change shows it, then add it. It starts with a clock across it, to change like any page.".into();
        }
        if let Some(name) = &self.editing
            && self.takeover.as_ref() != Some(name)
        {
            return format!("Putting “{name}” on the strip…");
        }
        if let (None, Some(t)) = (&self.editing, &self.takeover) {
            return format!(
                "“{t}” is covering the page for a moment. Editing goes on when it ends."
            );
        }
        if self.store_error {
            return "Changes cannot be saved right now; the status in the header says why.".into();
        }
        if self.layout.is_some() && self.shown_row().is_none() {
            return String::new();
        }
        let widgets = self.widgets();
        if widgets.is_empty() {
            return "Nothing here yet.".into();
        }
        let sized = |i: usize| {
            format!(
                "{} {} px, {}",
                describe(&widgets[i]),
                widths[i],
                geometry::class(widths[i]).to_lowercase()
            )
        };
        match (self.drag, selected) {
            (Drag::Resizing { edge, .. }, _) | (_, Some(edge)) if self.preview.is_some() => {
                let mut parts: Vec<String> = [edge, edge + 1]
                    .into_iter()
                    .filter(|&i| i < widgets.len())
                    .map(sized)
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
            // At the strip's end there is no grip: its edge there cannot
            // move, and the grip would sit on the edge flag.
            (_, Some(i)) if widths.iter().sum::<u32>() >= STRIP && i + 1 == widgets.len() => {
                if i == 0 {
                    format!(
                        "{}, the whole strip. Make it narrower in Width below to put another beside it.",
                        sized(i)
                    )
                } else {
                    format!(
                        "{}, {} of {}, to the strip's end. Drag it to move it; resize it in Width below.",
                        sized(i),
                        i + 1,
                        widgets.len()
                    )
                }
            }
            (_, Some(i)) => format!(
                "{}, {} of {}. Drag it to move it; drag the grip on its right edge to resize.",
                sized(i),
                i + 1,
                widgets.len()
            ),
            _ => "Click a widget to change it.".into(),
        }
    }
}

/// "When the track changes · 5 s · right end".
fn takeover_summary(t: &Takeover) -> String {
    let what: Vec<String> = t
        .on_change
        .iter()
        .map(|s| names::named(s).name.to_lowercase())
        .collect();
    let when = match what.as_slice() {
        [] => "Never shown: no reading chosen".to_string(),
        [one] => format!("When the {one} changes"),
        [rest @ .., last] => format!("When the {} or {last} changes", rest.join(", ")),
    };
    let secs = if t.seconds.fract() == 0.0 {
        format!("{:.0} s", t.seconds)
    } else {
        format!("{} s", t.seconds)
    };
    let side = match t.align {
        nexus_core::config::Align::Full => String::new(),
        nexus_core::config::Align::Left => "  ·  left end".into(),
        nexus_core::config::Align::Right => "  ·  right end".into(),
    };
    format!("{when}  ·  {secs}{side}")
}

/// Replaces a model's rows, in place when there are as many and each is
/// `same` as before, so the views made from them are kept (with their
/// focus and scroll position); only rows that differ are set.
fn update_in_place<T: Clone + PartialEq + 'static>(
    model: &VecModel<T>,
    rows: Vec<T>,
    same: impl Fn(&T, &T) -> bool,
) {
    let keep =
        rows.len() == model.row_count() && rows.iter().zip(model.iter()).all(|(a, b)| same(a, &b));
    if !keep {
        model.set_vec(rows);
        return;
    }
    for (i, row) in rows.into_iter().enumerate() {
        if model.row_data(i).as_ref() != Some(&row) {
            model.set_row_data(i, row);
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
            feeds: layout
                .config
                .news
                .as_ref()
                .map(|n| n.feeds.clone())
                .unwrap_or_default(),
        }
    }

    fn note(&self, index: usize) -> String {
        match (&self.layout, self.shown_row()) {
            (Some(l), Some(Shown::Page(p))) => l.notes.page(p).widget(index).to_string(),
            (Some(l), Some(Shown::Takeover(t))) => l.notes.takeover(t).widget(index).to_string(),
            _ => String::new(),
        }
    }

    /// The takeover being edited: its place and store ID.
    fn editing_takeover(&self) -> Option<(usize, i64)> {
        let l = self.layout.as_ref()?;
        let name = self.editing.as_ref()?;
        let i = l.config.takeovers.iter().position(|t| &t.name == name)?;
        Some((i, l.ids.takeovers[i].id))
    }

    /// The takeover form's values: a new one's draft, or the edited one's.
    fn takeover_values(&self) -> Option<Values> {
        if let Some(v) = &self.adding {
            return Some(v.clone());
        }
        let (i, _) = self.editing_takeover()?;
        let mut v = takeovers::flatten(&self.layout.as_ref()?.config.takeovers[i]);
        v.extend(self.drafts.clone());
        Some(v)
    }

    /// Puts the settings of what is selected under the strip: the widget,
    /// else the takeover being edited or added, else the page. Rows whose
    /// value changed are rebuilt so their controls show it, except a row
    /// whose new value is the one just sent from it.
    fn show_fields(&mut self) {
        let selected = self.selected_index().filter(|_| self.editable());
        let form = match selected {
            Some(i) => self.widget_ids().get(i).copied().map(Form::Widget),
            None if self.adding.is_some() => Some(Form::NewTakeover),
            None => self.editing_takeover().map(|(_, id)| Form::Takeover(id)),
        };
        if form != self.fields_for {
            self.fields_for = form;
            self.drafts.clear();
            self.draft_hint.clear();
            self.committed.clear();
            self.shown.clear();
            self.fields.set_vec(Vec::new());
            self.picking = None;
        }
        // A new takeover is a draft until it has its signals.
        if let Some(v) = &self.adding {
            self.draft_hint = match takeovers::build(v, &[]) {
                Err(Missing::Incomplete(why)) => why,
                _ => String::new(),
            };
        }
        let ctx = self.context();
        let fields = match selected {
            Some(i) => {
                let mut values = inspector::flatten(&self.widgets()[i], &ctx);
                values.extend(self.drafts.clone());
                let mut fields = inspector::fields(&values, &ctx);
                let (min, max) = geometry::width_bounds(&self.layout_widths(), i);
                if let Some(width) = fields.iter_mut().find(|f| f.key == "width") {
                    width.kind = Kind::Stepper { min, max };
                    // The range is beside the field.
                    width.help = if min == max {
                        "Nothing beside it can give it room: make a neighbour narrower first."
                            .into()
                    } else {
                        if i + 1 < self.layout_widths().len() {
                            "The arrow keys step by 1, with Shift by 10. The widget on its right gives or takes the difference, as when the grip on the strip is dragged.".into()
                        } else {
                            "The arrow keys step by 1, with Shift by 10. It grows into the free room at the strip's end, and narrower leaves room there.".into()
                        }
                    };
                }
                fields.push(Field {
                    key: "note".into(),
                    label: "Note".into(),
                    kind: Kind::Note,
                    value: self.note(i),
                    help: "For you: kept with the widget, and shown as a comment in an exported layout."
                        .into(),
                });
                fields
            }
            None => self
                .takeover_values()
                .map(|v| takeovers::fields(&v))
                .unwrap_or_default(),
        };
        let own = self.own_icon();
        let make = |f: &Field| {
            let mut r = row(f, &ctx);
            // "The widget's own" shows the glyph it is.
            if matches!(f.kind, Kind::Icon)
                && f.value.trim().is_empty()
                && let Some(c) = own
            {
                r.icon = c.to_string().into();
            }
            r
        };
        let same_keys = fields.len() == self.shown.len()
            && fields.iter().zip(&self.shown).all(|(a, b)| a.key == b.key);
        if !same_keys {
            self.fields
                .set_vec(fields.iter().map(make).collect::<Vec<_>>());
        } else {
            for (i, (new, old)) in fields.iter().zip(&self.shown).enumerate() {
                let fresh = make(new);
                if self.fields.row_data(i).as_ref() == Some(&fresh) {
                    continue;
                }
                let just_sent = self.committed.get(&new.key) == Some(&new.value);
                if new.value != old.value && !just_sent {
                    self.fields.remove(i);
                    self.fields.insert(i, fresh);
                } else {
                    self.fields.set_row_data(i, fresh);
                }
            }
        }
        self.shown = fields;
        let triggers: Vec<Trigger> = self
            .takeover_values()
            .map(|v| takeovers::triggers(&v))
            .unwrap_or_default()
            .into_iter()
            .map(|id| {
                let named = names::named(&id);
                Trigger {
                    id: id.into(),
                    name: named.name.into(),
                    icon: named.icon.to_string().into(),
                }
            })
            .collect();
        update_in_place(&self.triggers, triggers, |a, b| a.id == b.id);
        let (kind, title, icon, note) = match (self.fields_for, selected) {
            (Some(Form::Widget(_)), Some(i)) => {
                let w = &self.widgets()[i];
                let width = self.widths().get(i).copied().unwrap_or_default();
                (
                    "widget",
                    describe(w),
                    widget_icon(w).to_string(),
                    format!("{}, {width} px", geometry::class(width)),
                )
            }
            (Some(Form::NewTakeover), _) => (
                "new-takeover",
                "New takeover".to_string(),
                String::new(),
                String::new(),
            ),
            (Some(Form::Takeover(_)), _) => (
                "takeover",
                self.editing.clone().unwrap_or_default(),
                String::new(),
                "Takeover".to_string(),
            ),
            _ => (
                "page",
                self.page.clone(),
                String::new(),
                self.page_index()
                    .map(|i| format!("Page {} of {}", i + 1, self.page_names().len()))
                    .unwrap_or_default(),
            ),
        };
        if let Some(app) = self.app.upgrade() {
            app.set_draft_hint(self.draft_hint.as_str().into());
            app.set_panel_kind(kind.into());
            app.set_panel_title(title.into());
            app.set_panel_icon(icon.into());
            app.set_panel_note(note.into());
        }
    }

    fn field_changed(&mut self, key: &str, value: &str) {
        if self.busy {
            self.queued.push((key.into(), value.into()));
            return;
        }
        if key.starts_with("takeover.") {
            self.takeover_changed(key, value);
            return;
        }
        if key.starts_with("settings.") {
            self.settings_changed(key, value);
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

    /// The gallery is opening: draw its pictures with the latest readings.
    fn open_gallery(&mut self) {
        let presets = gallery::presets();
        let mut slots = [0i32; gallery::GROUPS.len()];
        let tiles: Vec<Tile> = presets
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let slot = slots[p.group];
                slots[p.group] += 1;
                Tile {
                    name: crate::i18n::shown(p.name).into(),
                    about: crate::i18n::shown(p.about).into(),
                    image: self.tiles.row_data(i).map(|t| t.image).unwrap_or_default(),
                    width: p.widget.width() as i32,
                    group: p.group as i32,
                    slot,
                }
            })
            .collect();
        update_in_place(&self.tiles, tiles, |a, b| a.name == b.name);
        if let Some(app) = self.app.upgrade() {
            app.set_group_sizes(ModelRc::new(VecModel::from(slots.to_vec())));
            let at = self.selected_index().map_or_else(
                || "It goes at the end".to_string(),
                |i| format!("It goes after {}", describe(&self.widgets()[i])),
            );
            app.set_gallery_note(
                format!(
                    "{at}; the widgets already there make room for it. Pictures use your readings."
                )
                .into(),
            );
        }
        if let (Some(jobs), Some(l)) = (&self.thumbs, &self.layout) {
            let _ = jobs.send(Job::Widgets {
                widgets: presets.into_iter().map(|p| p.widget).collect(),
                config: Box::new(l.config.clone()),
                readings: thumbs::with_demo(self.readings()),
            });
        }
    }

    /// Adds gallery item `index` after the selection (or at the end),
    /// taking the room from empty strip first and then from the others in
    /// proportion. Add is offered only when something fits, so this does
    /// not fail on a full page.
    fn add_tile(&mut self, index: usize) {
        let Some(preset) = gallery::presets().into_iter().nth(index) else {
            return;
        };
        let Some(on) = self.row_id() else {
            return;
        };
        let widths = self.layout_widths();
        let mins: Vec<u32> = self.widgets().iter().map(geometry::readable).collect();
        let Some((others, width)) = geometry::make_room_above(
            &widths,
            &mins,
            preset.widget.width(),
            geometry::readable(&preset.widget),
        ) else {
            self.set_problem(format!(
                "{} does not fit here without making a widget too narrow to read. Remove one, or add a page.",
                preset.name
            ));
            return;
        };
        let at = self.selected_index().map_or(widths.len(), |i| i + 1);
        let mut edits: Vec<Edit> = self
            .widgets()
            .iter()
            .zip(self.widget_ids())
            .zip(&others)
            .filter(|((w, _), new)| w.width() != **new)
            .map(|((w, &id), &new)| Edit::SetWidget {
                widget: id,
                value: w.clone().with_width(new),
            })
            .collect();
        edits.push(Edit::AddWidget {
            on,
            at: Some(at),
            widget: preset.widget.with_width(width),
        });
        self.select_after = Some(at);
        self.send(Edit::Batch {
            label: format!("Add {}", preset.name),
            edits,
        });
    }

    /// Every widget on the row the same width, sharing the whole strip.
    fn equal_widths(&mut self) {
        self.flush();
        let equal = geometry::equal(self.widgets().len());
        let edits: Vec<Edit> = self
            .widgets()
            .iter()
            .zip(self.widget_ids())
            .zip(&equal)
            .filter(|((w, _), new)| w.width() != **new)
            .map(|((w, &id), &new)| Edit::SetWidget {
                widget: id,
                value: w.clone().with_width(new),
            })
            .collect();
        if !edits.is_empty() {
            self.send(Edit::Batch {
                label: "Equal widths".into(),
                edits,
            });
        }
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

/// Pages: shown, added, renamed, moved and deleted from the tabs and the
/// page settings.
impl Editor {
    fn page_names(&self) -> Vec<&str> {
        self.layout
            .iter()
            .flat_map(|l| &l.config.pages)
            .map(|p| p.name.as_str())
            .collect()
    }

    /// The page on the strip: its place and ID.
    fn current_page(&self) -> Option<(usize, i64)> {
        let i = self.page_index()?;
        Some((i, self.layout.as_ref()?.ids.pages[i].id))
    }

    fn show_page(&mut self, index: usize) {
        self.stop_editing();
        self.adding = None;
        self.selected = None;
        self.show();
        request_page(index);
    }

    /// Adds an empty page at the end, named after its place, and shows it.
    fn add_page(&mut self) {
        self.stop_editing();
        self.adding = None;
        let names = self.page_names();
        let name = (names.len() + 1..)
            .map(|n| format!("Page {n}"))
            .find(|n| !names.contains(&n.as_str()))
            .expect("some page number is free");
        let at = names.len();
        self.selected = None;
        self.show_after = Some(at);
        self.send(Edit::AddPage {
            layout: None,
            name,
            at: Some(at),
        });
    }

    fn rename_page(&mut self, name: &str) {
        let Some((i, page)) = self.current_page() else {
            return;
        };
        let name = name.trim();
        let names = self.page_names();
        if name == names[i] {
            return;
        }
        let refused = if name.is_empty() {
            Some("A page needs a name.".to_string())
        } else if names.contains(&name) {
            Some(format!("There is already a page called “{name}”."))
        } else {
            None
        };
        if let Some(why) = refused {
            if let Some(app) = self.app.upgrade() {
                app.set_resets(app.get_resets() + 1);
            }
            self.set_problem(why);
            return;
        }
        self.renamed = Some((names[i].to_string(), name.to_string()));
        self.problem.clear();
        self.send(Edit::RenamePage {
            page,
            name: name.into(),
        });
    }

    /// Moves page `from` to place `to` (both from 0), as dragging its
    /// handle in the storyboard does.
    fn move_page(&mut self, from: usize, to: usize) {
        let Some(l) = &self.layout else {
            return;
        };
        let Some(page) = l.ids.pages.get(from).map(|p| p.id) else {
            return;
        };
        if from == to || to >= l.config.pages.len() {
            return;
        }
        self.problem.clear();
        self.send(Edit::MovePage { page, to });
    }

    fn delete_page(&mut self) {
        let Some((_, page)) = self.current_page() else {
            return;
        };
        if self.page_names().len() < 2 {
            return;
        }
        self.problem.clear();
        self.send(Edit::DeletePage { page });
    }
}

/// Takeovers: chosen from the tabs to edit, added from a form, deleted.
impl Editor {
    fn edit_takeover(&mut self, index: usize) {
        let Some(name) = self
            .layout
            .as_ref()
            .and_then(|l| l.config.takeovers.get(index))
            .map(|t| t.name.clone())
        else {
            return;
        };
        self.adding = None;
        self.selected = None;
        self.start_editing(name);
    }

    /// Shows takeover `name` on the strip, and keeps it there.
    fn start_editing(&mut self, name: String) {
        self.editing = Some(name);
        self.renew_lease();
        self.lease.start(TimerMode::Repeated, LEASE_RENEW, || {
            with_editor(|e| e.renew_lease());
        });
        self.show();
    }

    fn renew_lease(&self) {
        if let Some(name) = self.editing.clone() {
            thread::spawn(move || {
                if let Ok(mut c) = Client::connect() {
                    let _ = c.request(&format!("show-takeover {LEASE} {name}"));
                }
            });
        }
    }

    /// Stops editing a takeover and puts the page back.
    fn stop_editing(&mut self) {
        if self.editing.take().is_some() {
            self.lease.stop();
            thread::spawn(|| {
                if let Ok(mut c) = Client::connect() {
                    let _ = c.request("end-takeover");
                }
            });
        }
    }

    fn add_takeover(&mut self) {
        self.stop_editing();
        self.selected = None;
        let names: Vec<&str> = self
            .layout
            .iter()
            .flat_map(|l| &l.config.takeovers)
            .map(|t| t.name.as_str())
            .collect();
        self.adding = Some(takeovers::new_values(&names));
        self.show();
    }

    /// "Add takeover" in a new takeover's settings.
    fn create_takeover(&mut self) {
        let Some(v) = self.adding.clone() else {
            return;
        };
        let taken: Vec<&str> = self
            .layout
            .iter()
            .flat_map(|l| &l.config.takeovers)
            .map(|t| t.name.as_str())
            .collect();
        match takeovers::build(&v, &taken) {
            Ok(s) => {
                // It starts with a clock across it, to be changed like any
                // widget.
                let takeover = Takeover {
                    name: s.name.clone(),
                    on_change: s.on_change,
                    seconds: s.seconds,
                    align: s.align,
                    width: s.width,
                    widgets: vec![Widget::Clock {
                        width: s.width.unwrap_or(STRIP),
                        icon: None,
                        tap: None,
                        hold: None,
                    }],
                };
                self.adding = None;
                self.draft_hint.clear();
                self.edit_after = Some(s.name);
                self.send(Edit::AddTakeover {
                    layout: None,
                    takeover,
                });
            }
            Err(Missing::Incomplete(why) | Missing::Invalid(why)) => self.set_problem(why),
        }
    }

    fn delete_takeover(&mut self) {
        if self.adding.take().is_some() {
            self.draft_hint.clear();
            self.show();
            return;
        }
        let Some((_, id)) = self.editing_takeover() else {
            return;
        };
        self.stop_editing();
        self.send(Edit::DeleteTakeover { takeover: id });
    }

    /// A field of the takeover form changed.
    fn takeover_changed(&mut self, key: &str, value: &str) {
        let Some(mut v) = self.takeover_values() else {
            return;
        };
        v.insert(key.into(), value.into());
        // Moved to one end: half the strip until another width is chosen.
        if key == takeovers::ALIGN
            && !matches!(value, "full" | "Whole strip")
            && v.get(takeovers::WIDTH).is_none_or(|w| w.trim().is_empty())
        {
            v.insert(takeovers::WIDTH.into(), "Half".into());
        }
        let editing = self.editing_takeover();
        let taken: Vec<&str> = self
            .layout
            .iter()
            .flat_map(|l| l.config.takeovers.iter().enumerate())
            .filter(|(i, _)| Some(*i) != editing.map(|e| e.0))
            .map(|(_, t)| t.name.as_str())
            .collect();
        let built = takeovers::build(&v, &taken);
        self.committed.insert(key.into(), value.into());
        self.problem.clear();
        match (built, editing) {
            (Ok(_), None) => {
                // A new one is added by its own button, once it is whole.
                self.adding = Some(v);
                self.draft_hint.clear();
            }
            (Ok(s), Some((i, id))) => {
                self.drafts.clear();
                self.draft_hint.clear();
                let old = &self.layout.as_ref().expect("editing one").config.takeovers[i];
                let same = old.name == s.name
                    && old.on_change == s.on_change
                    && old.seconds == s.seconds
                    && old.align == s.align
                    && old.width == s.width;
                if !same {
                    if old.name != s.name {
                        // Followed by its new name once the layout says so.
                        self.edit_after = Some(s.name.clone());
                    }
                    self.send(Edit::SetTakeover {
                        takeover: id,
                        name: s.name,
                        on_change: s.on_change,
                        seconds: s.seconds,
                        align: s.align,
                        width: s.width,
                    });
                }
            }
            (Err(Missing::Incomplete(why)), _) => {
                if self.adding.is_some() {
                    self.adding = Some(v);
                } else {
                    self.drafts.insert(key.into(), value.into());
                }
                self.draft_hint = why;
            }
            (Err(Missing::Invalid(why)), _) => self.problem = why,
        }
        self.show();
    }
}

impl Editor {
    /// A field of the strip's settings changed.
    fn settings_changed(&mut self, key: &str, value: &str) {
        let Some(layout) = &self.layout else {
            return;
        };
        let mut v = settings::flatten(&layout.config);
        v.insert(key.into(), value.into());
        self.committed.insert(key.into(), value.into());
        self.problem.clear();
        match settings::build(&v) {
            Ok((s, weather)) => {
                if s != layout.config.settings || weather != layout.config.weather {
                    self.send(Edit::SetSettings {
                        settings: s,
                        weather,
                    });
                }
            }
            Err(Missing::Invalid(why) | Missing::Incomplete(why)) => self.problem = why,
        }
        self.show();
    }
}

impl Editor {
    /// A choice field's option was picked, by its position in the list.
    fn field_chosen(&mut self, key: &str, index: i32) {
        let option = self
            .shown
            .iter()
            .find(|f| f.key == key)
            .and_then(|f| match &f.kind {
                Kind::Choice(options) => options.get(usize::try_from(index).ok()?).cloned(),
                _ => None,
            });
        if let Some(option) = option {
            self.field_changed(key, &option);
        }
    }
}

/// Layouts: switching, adding, renaming and deleting.
impl Editor {
    fn layout_entry(&self, i: usize) -> Option<LayoutEntry> {
        self.layout.as_ref()?.layouts.get(i).cloned()
    }

    /// Before the strip changes layout: nothing of the old one stays chosen.
    fn leave_layout(&mut self) {
        self.stop_editing();
        self.adding = None;
        self.selected = None;
    }

    fn use_layout(&mut self, i: usize) {
        let Some(entry) = self.layout_entry(i) else {
            return;
        };
        if self.layout.as_ref().is_some_and(|l| l.id != entry.id) {
            self.leave_layout();
            self.send(Edit::Activate { layout: entry.id });
        }
    }

    fn delete_layout(&mut self, i: usize) {
        if let Some(entry) = self.layout_entry(i) {
            self.send(Edit::DeleteLayout { layout: entry.id });
        }
    }

    /// Renames the layout on the strip.
    fn rename_layout(&mut self, name: &str) {
        let Some(l) = &self.layout else {
            return;
        };
        let name = name.trim();
        if name == l.name {
            return;
        }
        let refused = if name.is_empty() {
            Some("A layout needs a name.".to_string())
        } else if l.layouts.iter().any(|e| e.name == name) {
            Some(format!("There is already a layout called “{name}”."))
        } else {
            None
        };
        if let Some(why) = refused {
            if let Some(app) = self.app.upgrade() {
                app.set_resets(app.get_resets() + 1);
            }
            self.set_problem(why);
            return;
        }
        let layout = l.id;
        self.send(Edit::RenameLayout {
            layout,
            name: name.into(),
        });
    }

    /// Adds a layout from `choice` (see `layouts::choices`) and puts it on
    /// the strip, in one request.
    fn add_layout(&mut self, choice: &str) {
        let Some(l) = &self.layout else {
            return;
        };
        let Some((base, text)) = layouts::start(choice, (&l.name, &l.config, &l.notes)) else {
            return;
        };
        let taken: Vec<&str> = l.layouts.iter().map(|e| e.name.as_str()).collect();
        let name = layouts::unique_name(&base, &taken);
        self.leave_layout();
        let import = ImportRequest {
            name,
            text,
            activate: true,
        };
        self.request(format!(
            "import {}",
            serde_json::to_string(&import).unwrap_or_default()
        ));
    }
}

/// Try mode.
impl Editor {
    fn set_trying(&mut self, on: bool) {
        if on == self.trying {
            return;
        }
        if on {
            // A takeover being edited would cover what is tried.
            self.stop_editing();
            self.adding = None;
            self.selected = None;
            self.drag = Drag::None;
        }
        self.trying = on;
        self.touch = None;
        if let Some(app) = self.app.upgrade() {
            app.set_trying(on);
        }
        self.show();
    }

    /// The press ended: send it to the daemon as a touch on the strip.
    fn try_release(&mut self, x: f32) {
        let Some((from, at)) = self.touch.take() else {
            return;
        };
        let line = simulate::gesture(from, x, at.elapsed());
        thread::spawn(move || {
            if let Ok(mut client) = Client::connect() {
                let _ = client.request(&line);
            }
        });
    }
}

/// The storyboard: a strip not being edited was clicked.
impl Editor {
    /// Puts that page or takeover on the strip to edit it. With `x` (strip
    /// pixels, negative for none) the widget there is selected once it is.
    fn strip_clicked(&mut self, kind: &str, index: usize, x: f32) {
        let Some(l) = &self.layout else {
            return;
        };
        let name = match kind {
            "page" => l.config.pages.get(index).map(|p| p.name.clone()),
            _ => l.config.takeovers.get(index).map(|t| t.name.clone()),
        };
        let Some(name) = name else {
            return;
        };
        self.select_at = (x >= 0.0 && !self.trying).then(|| (format!("{kind}:{name}"), x));
        if kind == "page" {
            self.show_page(index);
        } else {
            self.edit_takeover(index);
        }
        self.arrived();
    }

    /// The strip shows what was clicked: select the widget under the click.
    fn arrived(&mut self) {
        let Some((target, x)) = self.select_at.clone() else {
            return;
        };
        let here = match (&self.editing, self.shown_row()) {
            (Some(t), Some(Shown::Takeover(_))) => format!("takeover:{t}"),
            (None, Some(Shown::Page(_))) => format!("page:{}", self.page),
            _ => return,
        };
        if here == target {
            self.select_at = None;
            self.selected = self
                .zone_at(x)
                .and_then(|i| self.widget_ids().get(i).copied());
        }
    }
}

/// Pictures of the strips not on it.
impl Editor {
    /// The latest readings, as the engine takes them.
    fn readings(&self) -> Vec<(String, nexus_core::signal::Value)> {
        self.signals
            .iter()
            .map(|(name, r)| {
                (
                    name.clone(),
                    thumbs::value(&r.text, r.number, r.unit.as_deref()),
                )
            })
            .collect()
    }

    /// Asks for pictures of every page and takeover.
    fn draw_layout(&mut self) {
        let (Some(jobs), Some(l)) = (&self.thumbs, &self.layout) else {
            return;
        };
        self.epoch += 1;
        let _ = jobs.send(Job::Layout {
            config: Box::new(l.config.clone()),
            readings: self.readings(),
            epoch: self.epoch,
        });
    }

    /// Pictures arrived from `thumbs`.
    pub fn pictures(&mut self, done: Done) {
        let image = |px: &[u8], w: u32| {
            Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(px, w, 48))
        };
        match done {
            Done::Layout {
                pages,
                takeovers,
                epoch,
            } => {
                // An older layout's pictures, overtaken by an edit: the
                // newer ones are on their way.
                if epoch < self.epoch {
                    return;
                }
                self.page_pictures = pages.iter().map(|p| image(p, 640)).collect();
                self.takeover_pictures = takeovers.iter().map(|p| image(p, 640)).collect();
                if let Some(app) = self.app.upgrade() {
                    self.show_rows(&app);
                }
            }
            Done::Widgets(list) => {
                let presets = gallery::presets();
                for (i, (px, p)) in list.iter().zip(&presets).enumerate() {
                    let (cropped, w) = thumbs::crop(px, p.widget.width());
                    if let Some(mut tile) = self.tiles.row_data(i) {
                        tile.image = image(&cropped, w);
                        self.tiles.set_row_data(i, tile);
                    }
                }
            }
            Done::Starters(list) => {
                for (i, px) in list.iter().enumerate() {
                    if let Some(mut s) = self.starters.row_data(i) {
                        s.image = image(px, 640);
                        self.starters.set_row_data(i, s);
                    }
                }
            }
        }
    }
}

/// Starting layouts: on first run, and from "New layout…".
impl Editor {
    /// Lists the starters and asks for their pictures.
    fn open_starters(&mut self) {
        let mut list: Vec<Starter> = STARTERS
            .iter()
            .map(|(name, _)| Starter {
                name: crate::i18n::shown(name).into(),
                about: crate::i18n::shown(starter_about(name)).into(),
                image: Image::default(),
            })
            .collect();
        list.push(Starter {
            name: crate::i18n::shown("Empty").into(),
            about: crate::i18n::shown("One empty page, to build from nothing").into(),
            image: Image::default(),
        });
        update_in_place(&self.starters, list, |a, b| a.name == b.name);
        let mut configs: Vec<Config> = STARTERS
            .iter()
            .filter_map(|(_, text)| Config::parse(text).ok())
            .collect();
        let mut empty = Config::default_layout();
        empty.pages.truncate(1);
        if let Some(p) = empty.pages.first_mut() {
            p.widgets.clear();
        }
        configs.push(empty);
        if let Some(l) = &self.layout {
            for c in &mut configs {
                c.settings = l.config.settings.clone();
            }
        }
        if let Some(jobs) = &self.thumbs {
            let _ = jobs.send(Job::Starters {
                configs,
                readings: thumbs::with_demo(self.readings()),
            });
        }
    }

    /// Adds starter `i` (past the starters: an empty layout) and puts it
    /// on the strip.
    fn new_layout(&mut self, i: i32) {
        let choice = match usize::try_from(i).ok().and_then(|i| STARTERS.get(i)) {
            Some((name, _)) => format!("Starter: {name}"),
            None => layouts::EMPTY.to_string(),
        };
        self.add_layout(&choice);
    }

    /// A fresh store: one layout, the default one, never changed. The
    /// starters are offered once.
    fn offer_first_run(&mut self) {
        // Test-only, for captures: offer it once whatever the store holds.
        static FORCED: std::sync::Once = std::sync::Once::new();
        let mut forced = false;
        if std::env::var_os("NEXUS_UI_FIRST_RUN").is_some() {
            FORCED.call_once(|| forced = true);
        }
        if self.first_run_seen && !forced {
            return;
        }
        let Some(l) = &self.layout else {
            return;
        };
        let fresh = forced
            || (l.layouts.len() <= 1
                && l.undo.is_none()
                && l.config.pages == Config::default_layout().pages);
        if !fresh {
            // Not a first run: never offered on this computer again.
            self.first_run_done();
            return;
        }
        self.first_run_seen = true;
        self.open_starters();
        if let Some(app) = self.app.upgrade() {
            app.set_first_run(true);
            app.invoke_show_sheet("starters".into());
        }
    }

    fn first_run_done(&mut self) {
        self.first_run_seen = true;
        crate::first_run::mark_seen();
        if let Some(app) = self.app.upgrade() {
            app.set_first_run(false);
        }
    }
}

/// What a starter is for, in a line.
fn starter_about(name: &str) -> &'static str {
    match name {
        "Everything" => "Time, music, your machine and launchers, over five pages",
        "Monitor" => "Temperatures and load, for keeping an eye on the machine",
        "Music" => "What is playing, with play, skip and volume",
        "Minimal" => "Just the time and the weather",
        _ => "",
    }
}

/// Settings.
impl Editor {
    fn set_pref(&mut self, key: &str, value: &str) {
        let Some(l) = &self.layout else {
            return;
        };
        let mut v = settings::flatten(&l.config);
        if settings::set(&mut v, key, value).is_none() {
            return;
        }
        let Some((k, val)) = v
            .iter()
            .find(|(k, val)| settings::flatten(&l.config).get(*k) != Some(*val))
            .map(|(k, v)| (k.clone(), v.clone()))
        else {
            return;
        };
        self.settings_changed(&k, &val);
    }

    /// The Settings sheet's values.
    fn show_prefs(&self) {
        self.show_news();
        let (Some(app), Some(l)) = (self.app.upgrade(), &self.layout) else {
            return;
        };
        let p = settings::prefs(&l.config);
        app.set_pref_temperature(i32::from(p.fahrenheit));
        app.set_pref_clock(i32::from(p.twelve_hour));
        app.set_pref_accent(p.accent.as_str().into());
        app.set_accent_current(p.preset.map_or(-1, |i| i as i32));
        let picked = l.config.settings.accent.unwrap_or(crate::accent::DEFAULT);
        app.set_accent_note(if crate::accent::snapped(picked) {
            crate::i18n::shown("This colour sits too close to the strip's warning amber or alarm red, so the strip moves it to the nearest hue it can use: the one shown here.").into()
        } else {
            SharedString::new()
        });
        app.set_pref_weather(p.weather.as_str().into());
        app.set_pref_idle_dim(p.idle_dim);
        app.set_pref_night(!p.night_from.is_empty());
        app.set_pref_night_from(p.night_from.as_str().into());
        app.set_pref_night_to(p.night_to.as_str().into());
        app.set_fonts(ModelRc::new(VecModel::from(
            p.fonts
                .iter()
                .map(|f| SharedString::from(f.as_str()))
                .collect::<Vec<_>>(),
        )));
        app.set_pref_font(p.font as i32);
    }
}

/// Settings, News: the feeds and how headlines show.
impl Editor {
    fn news(&self) -> nexus_core::config::NewsConfig {
        news::current(self.layout.as_ref().and_then(|l| l.config.news.as_ref()))
    }

    fn send_news(&mut self, c: nexus_core::config::NewsConfig) {
        let to = news::to_send(c);
        if self.layout.as_ref().map(|l| &l.config.news) != Some(&to) {
            self.send(Edit::SetNews { news: to });
        }
    }

    /// The News section's values.
    fn show_news(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let c = self.news();
        let rows: Vec<FeedRow> = c
            .feeds
            .iter()
            .map(|url| FeedRow {
                name: news::name(url, self.feed_titles.get(url).map(String::as_str)).into(),
                address: url.as_str().into(),
            })
            .collect();
        app.set_news_feeds(ModelRc::new(VecModel::from(rows)));
        let rotate = news::rotate_index(&c);
        let age = news::max_age_index(&c);
        app.set_news_rotate(rotate.map_or(-1, |i| i as i32));
        app.set_news_max_age(age.map_or(-1, |i| i as i32));
        // A duration set in a layout file that is none of the choices: said
        // under them, since no choice is shown as picked.
        let note = |chosen: Option<usize>, d| match chosen {
            Some(_) => String::new(),
            None => format!("Now {}, from the layout file.", news::spoken(d)),
        };
        app.set_news_rotate_note(note(rotate, c.rotate).into());
        app.set_news_age_note(note(age, c.max_age).into());
    }

    fn feed_result(&self, state: &str, title: &str, detail: &str, force: bool) {
        if let Some(app) = self.app.upgrade() {
            app.set_feed_state(state.into());
            app.set_feed_title(title.into());
            app.set_feed_detail(detail.into());
            app.set_feed_force(force);
        }
    }

    /// Add: the address is checked once, off the UI thread, and added if it
    /// is a feed. Its title and newest headline, or why not, show under it.
    fn add_feed(&mut self, typed: &str) {
        let url = news::address(typed);
        if url.is_empty() {
            return;
        }
        self.feed_pending = None;
        if self.news().feeds.iter().any(|f| f.trim() == url) {
            self.feed_result("problem", "That feed is already in the list.", "", false);
            return;
        }
        self.feed_try += 1;
        let try_no = self.feed_try;
        self.feed_result("checking", "Checking the feed…", &url, false);
        let checked = url.clone();
        let _ = thread::Builder::new()
            .name("feed-check".into())
            .spawn(move || {
                let result = nexus_sources::news::check_feed(&checked);
                let _ = slint::invoke_from_event_loop(move || {
                    with_editor(|e| e.feed_checked(try_no, &checked, result));
                });
            });
    }

    fn feed_checked(
        &mut self,
        try_no: u64,
        url: &str,
        result: Result<nexus_sources::news::Checked, nexus_sources::news::Problem>,
    ) {
        if try_no != self.feed_try {
            return; // a newer address was typed since
        }
        match result {
            Ok(found) => {
                self.feed_titles
                    .insert(url.to_string(), found.title.clone());
                let detail = match &found.newest {
                    Some(h) => format!("Newest: {h}"),
                    None => "It has no headlines yet; they show when it posts some.".into(),
                };
                self.feed_result("added", &format!("Added {}", found.title), &detail, false);
                if let Some(c) = news::with_feed(&self.news(), url) {
                    self.send_news(c);
                }
                if let Some(app) = self.app.upgrade() {
                    app.set_feed_epoch(app.get_feed_epoch() + 1);
                }
            }
            Err(p) => {
                let (what, next, force) = news::problem(&p);
                self.feed_pending = force.then(|| url.to_string());
                self.feed_result("problem", &what, &next, force);
            }
        }
    }

    /// "Add anyway": a site that did not answer may only be down for now.
    fn add_feed_anyway(&mut self) {
        let Some(url) = self.feed_pending.take() else {
            return;
        };
        if let Some(c) = news::with_feed(&self.news(), &url) {
            self.send_news(c);
        }
        self.feed_result(
            "added",
            &format!("Added {}", news::name(&url, None)),
            "Its headlines show once the site answers.",
            false,
        );
        if let Some(app) = self.app.upgrade() {
            app.set_feed_epoch(app.get_feed_epoch() + 1);
        }
    }

    fn remove_feed(&mut self, index: usize) {
        if let Some(c) = news::without_feed(&self.news(), index) {
            self.feed_result("", "", "", false);
            self.send_news(c);
        }
    }

    fn move_feed(&mut self, from: usize, to: usize) {
        if let Some(c) = news::moved_feed(&self.news(), from, to) {
            self.send_news(c);
        }
    }

    fn news_choice(&mut self, key: &str, index: usize) {
        if let Some(c) = news::with_choice(&self.news(), key, index) {
            self.send_news(c);
        }
    }
}

/// A change made while another was on its way.
enum Waiting {
    Edit(Box<Edit>),
    Line(String),
}

impl Editor {
    fn send_next(&mut self) {
        match self.waiting.pop_front() {
            Some(Waiting::Edit(edit)) => self.send(*edit),
            Some(Waiting::Line(line)) => self.request(line),
            None => {}
        }
    }
}

/// Why a request did not go through.
enum Failure {
    /// nexusd could not be reached: nothing was sent.
    Unsent,
    /// The connection broke after sending: it may or may not have been made.
    Lost,
    /// nexusd said no, and why.
    Refused(String),
}

/// Whether a failed request is kept to send again. One never sent always
/// is. One that may have arrived only if it is an edit: an edit names the
/// revision it was made against, so if it did arrive, sending it again is
/// refused rather than made twice. Anything else could be done twice.
fn keep(failure: &Failure, line: &str) -> bool {
    match failure {
        Failure::Unsent => true,
        Failure::Lost => line.starts_with("edit "),
        Failure::Refused(_) => false,
    }
}

/// Import from file…: asks for a layout file, checks it, and adds it as
/// a new layout on the strip, named after the file.
fn import_layout() {
    let Some(dialog) = file_dialog() else {
        return;
    };
    let _ = slint::spawn_local(async move {
        let Some(file) = dialog
            .set_title("Import a layout")
            .add_filter("Nexus Open layout", &["toml"])
            .pick_file()
            .await
        else {
            return; // cancelled
        };
        let read = files::read(file.path());
        with_editor(|e| match read {
            Err(why) => e.set_problem(why),
            Ok((base, text)) => {
                let Some(l) = &e.layout else {
                    return;
                };
                let taken: Vec<&str> = l.layouts.iter().map(|x| x.name.as_str()).collect();
                let name = layouts::unique_name(&base, &taken);
                e.leave_layout();
                let import = ImportRequest {
                    name,
                    text,
                    activate: true,
                };
                e.request(format!(
                    "import {}",
                    serde_json::to_string(&import).unwrap_or_default()
                ));
            }
        });
    });
}

/// Export…: saves the layout on the strip as a file to share.
fn export_layout() {
    let Some(dialog) = file_dialog() else {
        return;
    };
    let mut current = None;
    with_editor(|e| {
        current = e
            .layout
            .as_ref()
            .map(|l| (l.name.clone(), files::export_text(&l.config, &l.notes)));
    });
    let Some((name, text)) = current else {
        return;
    };
    let _ = slint::spawn_local(async move {
        let Some(file) = dialog
            .set_title("Export the layout")
            .set_file_name(files::file_name(&name))
            .add_filter("Nexus Open layout", &["toml"])
            .save_file()
            .await
        else {
            return; // cancelled
        };
        let path = file.path().to_path_buf();
        let written = std::fs::write(&path, text);
        with_editor(|e| match written {
            Ok(()) => {
                e.problem.clear();
                e.notice = format!("Saved “{name}” to {}.", path.display());
                e.show();
            }
            Err(why) => e.set_problem(format!("Cannot save {}: {why}", path.display())),
        });
    });
}

/// A file dialog that opens over the settings window.
fn file_dialog() -> Option<rfd::AsyncFileDialog> {
    let mut app = None;
    with_editor(|e| app = e.app.upgrade());
    let app = app?;
    Some(rfd::AsyncFileDialog::new().set_parent(&app.window().window_handle()))
}

/// Shows page `index` (from 0) on the strip. Off the UI thread, so a slow
/// daemon never freezes the window; the status poll picks up the change.
fn request_page(index: usize) {
    thread::spawn(move || {
        if let Ok(mut client) = Client::connect() {
            let _ = client.request(&format!("page {}", index + 1));
        }
    });
}

/// A field as the window takes it; a signal field also carries the
/// signal's human name, icon and live value.
fn row(f: &Field, ctx: &Context) -> FieldRow {
    let (kind, choices) = match &f.kind {
        Kind::Text => ("text", Vec::new()),
        Kind::Signal => ("signal", Vec::new()),
        Kind::Number => ("number", Vec::new()),
        Kind::Note => ("note", Vec::new()),
        Kind::Triggers => ("triggers", Vec::new()),
        Kind::Icon => ("icon", Vec::new()),
        Kind::Feeds => ("feeds", Vec::new()),
        Kind::Stepper { min, max } => ("stepper", vec![min.to_string(), max.to_string()]),
        Kind::Choice(options) => ("choice", options.clone()),
        Kind::Segmented(options) => ("segmented", options.clone()),
    };
    let index = choices
        .iter()
        .position(|c| c == &f.value)
        .map_or(-1, |i| i as i32);
    let (shown, icon, reading) = if matches!(f.kind, Kind::Signal) {
        let named = names::named(&f.value);
        let reading = ctx
            .signals
            .get(f.value.trim())
            .map(|r| inspector::show_reading(r, ctx))
            .unwrap_or_else(|| {
                if f.value.trim().is_empty() {
                    String::new()
                } else {
                    "no reading yet".into()
                }
            });
        (named.name, named.icon.to_string(), reading)
    } else if matches!(f.kind, Kind::Icon) {
        let name = f.value.trim();
        match nexus_core::icons::by_name(name) {
            Some(glyph) => (icon_title(name), glyph.to_string(), String::new()),
            None => Default::default(),
        }
    } else if matches!(f.kind, Kind::Feeds) {
        (
            f.value.clone(),
            nexus_core::icons::NEWSPAPER.to_string(),
            String::new(),
        )
    } else {
        Default::default()
    };
    FieldRow {
        key: f.key.as_str().into(),
        label: crate::i18n::shown(&f.label).into(),
        kind: kind.into(),
        value: f.value.as_str().into(),
        choices: ModelRc::new(VecModel::from(
            choices
                .iter()
                .map(|c| SharedString::from(crate::i18n::shown(c)))
                .collect::<Vec<_>>(),
        )),
        index,
        help: crate::i18n::shown(&f.help).into(),
        shown: shown.into(),
        icon: icon.into(),
        reading: reading.into(),
    }
}

/// "globe-simple" as a person would say it: "Globe simple".
fn icon_title(name: &str) -> String {
    let mut s = name.replace('-', " ");
    if let Some(first) = s.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    s
}

/// An undo or redo label, cut short: the buttons' width counts toward
/// the window's minimum width, so a long name must not widen the window.
fn label(s: Option<&str>) -> SharedString {
    const MAX: usize = 26;
    let s = s.unwrap_or_default();
    if s.chars().count() <= MAX {
        return s.into();
    }
    let cut: String = s.chars().take(MAX - 1).collect();
    format!("{}…", cut.trim_end()).into()
}

/// A widget's name as people see it: its label, or what it is.
pub fn describe(w: &Widget) -> String {
    match w {
        Widget::Clock { .. } => "Clock".into(),
        Widget::Media { .. } => "Now playing".into(),
        Widget::News { .. } => "News".into(),
        Widget::Key { icon, label, .. } => label
            .iter()
            .chain(icon)
            .find(|s| !s.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| "Key".into()),
        Widget::Value { label, signal, .. } if label.trim().is_empty() => names::named(signal).name,
        Widget::Rate { label, .. } if label.trim().is_empty() => "Network".into(),
        Widget::Slider { label, .. } if label.trim().is_empty() => "Slider".into(),
        Widget::Value { label, .. } | Widget::Rate { label, .. } | Widget::Slider { label, .. } => {
            label.clone()
        }
    }
}

/// The icon a widget is shown with in the window: its reading's, or its
/// kind's.
fn widget_icon(w: &Widget) -> char {
    match w {
        Widget::Clock { .. } => names::icon::CLOCK,
        Widget::Media { .. } => names::icon::MUSIC,
        Widget::Key { .. } => names::icon::KEY,
        Widget::Value { signal, .. } => names::named(signal).icon,
        Widget::Rate { .. } => names::icon::DOWN,
        Widget::Slider { .. } => names::icon::SLIDERS,
        Widget::News { .. } => nexus_core::icons::NEWSPAPER,
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

/// Something that went wrong outside the editor (Help), shown where its
/// problems are.
pub fn problem(why: String) {
    with_editor(|e| e.set_problem(why));
}

/// The daemon is reachable again. It may have restarted, a new version
/// with more to say, while the store's revision stayed the same, so the
/// layout is fetched again rather than waiting for a revision to move.
pub fn reconnected() {
    with_editor(|e| fetch_layout(e.app.clone()));
}

pub fn status(status: &Status) {
    let editor = EDITOR.with(|e| e.borrow().clone());
    if let Some(editor) = editor {
        Editor::status(&editor, status);
    }
}

/// nexusd answers again, or stopped answering.
pub fn connected(up: bool) {
    with_editor(|e| e.set_connected(up));
}

/// Pictures from `thumbs`, for the installed editor.
pub fn pictures(done: Done) {
    with_editor(|e| e.pictures(done));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_changes_that_cannot_happen_twice_are_sent_again() {
        assert!(keep(&Failure::Unsent, "import {}"));
        assert!(keep(&Failure::Lost, "edit {}"));
        assert!(!keep(&Failure::Lost, "import {}"));
        assert!(!keep(&Failure::Lost, "undo"));
        assert!(!keep(&Failure::Refused("no".into()), "edit {}"));
    }

    #[test]
    fn long_undo_labels_are_cut_short() {
        assert_eq!(label(Some("Add page Page 6")), "Add page Page 6");
        assert_eq!(
            label(Some("Change takeover A very long takeover name")),
            "Change takeover A very lo…"
        );
        assert_eq!(label(None), "");
    }
}
