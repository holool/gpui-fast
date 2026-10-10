//! XI2 contact forwarding, adapted from holool/gpui-arabic 1734541a88.
use std::collections::HashMap;

use gpui::{Pixels, Point, TouchEvent, TouchId, TouchPhase, point, px};

struct Contact {
    id: TouchId,
    window: u32,
    position: Point<Pixels>,
}

/// Native touch IDs may be reused after release. GPUI IDs must remain unique.
#[derive(Default)]
pub(crate) struct X11Touches {
    next_id: u64,
    contacts: HashMap<(u16, u32), Contact>,
}

impl X11Touches {
    pub fn begin(
        &mut self,
        source: u16,
        native_id: u32,
        window: u32,
        position: Point<Pixels>,
    ) -> Option<TouchEvent> {
        if self.contacts.contains_key(&(source, native_id)) {
            return None;
        }
        self.next_id = self.next_id.checked_add(1).expect("touch ID exhausted");
        let id = TouchId(self.next_id);
        self.contacts.insert(
            (source, native_id),
            Contact {
                id,
                window,
                position,
            },
        );
        Some(touch_event(id, TouchPhase::Started, position))
    }

    pub fn update(
        &mut self,
        source: u16,
        native_id: u32,
        phase: TouchPhase,
        position: Point<Pixels>,
    ) -> Option<(u32, TouchEvent)> {
        let contact = self.contacts.get_mut(&(source, native_id))?;
        contact.position = position;
        let result = (contact.window, touch_event(contact.id, phase, position));
        if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            self.contacts.remove(&(source, native_id));
        }
        Some(result)
    }

    pub fn cancel_window(&mut self, window: u32) -> Vec<TouchEvent> {
        let mut cancelled = Vec::new();
        self.contacts.retain(|_, contact| {
            if contact.window == window {
                cancelled.push(touch_event(
                    contact.id,
                    TouchPhase::Cancelled,
                    contact.position,
                ));
                false
            } else {
                true
            }
        });
        cancelled
    }

    pub fn cancel_source(&mut self, source: u16) -> Vec<(u32, TouchEvent)> {
        let mut cancelled = Vec::new();
        self.contacts.retain(|(device, _), contact| {
            if *device == source {
                cancelled.push((
                    contact.window,
                    touch_event(contact.id, TouchPhase::Cancelled, contact.position),
                ));
                false
            } else {
                true
            }
        });
        cancelled
    }
}

fn touch_event(id: TouchId, phase: TouchPhase, position: Point<Pixels>) -> TouchEvent {
    TouchEvent {
        id,
        phase,
        position,
        predicted_position: None,
        force: None,
    }
}

pub(crate) fn touch_position(x: i32, y: i32, scale_factor: f32) -> Point<Pixels> {
    // XI2 uses signed 16.16 fixed-point coordinates, including outside-window positions.
    point(
        px(x as f32 / 65536.0 / scale_factor),
        px(y as f32 / 65536.0 / scale_factor),
    )
}

#[cfg(test)]
mod tests {
    use super::{X11Touches, touch_position};
    use gpui::{TouchPhase, point, px};

    #[test]
    fn contact_keeps_its_id_and_window_until_release() {
        let mut touches = X11Touches::default();
        let start = touches
            .begin(9, 42, 100, point(px(12.0), px(20.0)))
            .unwrap();
        let (window, moved) = touches
            .update(9, 42, TouchPhase::Moved, point(px(15.0), px(30.0)))
            .unwrap();
        assert_eq!(window, 100);
        assert_eq!(moved.id, start.id);
        assert_eq!(moved.phase, TouchPhase::Moved);
        let (_, ended) = touches
            .update(9, 42, TouchPhase::Ended, moved.position)
            .unwrap();
        assert_eq!(ended.id, start.id);
        assert!(
            touches
                .update(9, 42, TouchPhase::Moved, moved.position)
                .is_none()
        );
    }

    #[test]
    fn device_ids_and_reused_native_ids_do_not_merge_contacts() {
        let mut touches = X11Touches::default();
        let position = point(px(1.0), px(2.0));
        let first = touches.begin(9, 1, 100, position).unwrap();
        let second = touches.begin(10, 1, 100, position).unwrap();
        assert_ne!(first.id, second.id);
        assert!(touches.begin(9, 1, 100, position).is_none());
        touches.update(9, 1, TouchPhase::Ended, position).unwrap();
        let reused = touches.begin(9, 1, 100, position).unwrap();
        assert_ne!(reused.id, first.id);
    }

    #[test]
    fn focus_loss_cancels_only_the_affected_window() {
        let mut touches = X11Touches::default();
        let position = point(px(1.0), px(2.0));
        let first = touches.begin(9, 1, 100, position).unwrap();
        touches.begin(9, 2, 200, position).unwrap();
        let cancelled = touches.cancel_window(100);
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].id, first.id);
        assert_eq!(cancelled[0].phase, TouchPhase::Cancelled);
        assert!(touches.update(9, 1, TouchPhase::Ended, position).is_none());
        assert!(touches.update(9, 2, TouchPhase::Moved, position).is_some());
    }

    #[test]
    fn device_removal_cancels_its_contacts_without_affecting_other_devices() {
        let mut touches = X11Touches::default();
        let position = point(px(1.0), px(2.0));
        touches.begin(9, 1, 100, position).unwrap();
        touches.begin(10, 1, 100, position).unwrap();
        let cancelled = touches.cancel_source(9);
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].0, 100);
        assert_eq!(cancelled[0].1.phase, TouchPhase::Cancelled);
        assert!(touches.update(9, 1, TouchPhase::Ended, position).is_none());
        assert!(touches.update(10, 1, TouchPhase::Moved, position).is_some());
    }

    #[test]
    fn fixed_point_coordinates_keep_sign_and_follow_display_scale() {
        assert_eq!(
            touch_position(65536 * 100 + 32768, -65536 * 4, 2.0),
            point(px(50.25), px(-2.0))
        );
    }
}

use crate::linux::{X11Client, X11ClientStatePtr};
use std::collections::HashSet;
use x11rb::{
    connection::Connection,
    protocol::{
        Event,
        xinput::{self, ConnectionExt as _},
    },
};

pub(crate) struct TouchState {
    enabled: bool,
    contacts: X11Touches,
    direct_devices: HashSet<u16>,
}
impl TouchState {
    pub(crate) fn new(major: u16, minor: u16, connection: &impl Connection) -> Self {
        let mut state = Self {
            enabled: major > 2 || (major == 2 && minor >= 2),
            contacts: X11Touches::default(),
            direct_devices: HashSet::new(),
        };
        state.refresh(connection);
        state
    }
    fn refresh(&mut self, connection: &impl Connection) {
        if !self.enabled {
            return;
        }
        if let Ok(cookie) = connection.xinput_xi_query_device(0u16) {
            if let Ok(reply) = cookie.reply() {
                self.direct_devices = reply
                    .infos
                    .into_iter()
                    .filter(|info| {
                        info.classes.iter().any(|class| {
                            class
                                .data
                                .as_touch()
                                .is_some_and(|touch| touch.mode == xinput::TouchMode::DIRECT)
                        })
                    })
                    .map(|info| info.deviceid)
                    .collect();
            }
        }
    }
    pub(crate) fn forget_window(&mut self, window: u32) {
        self.contacts.cancel_window(window);
    }
}
pub(crate) fn mask(client: &X11ClientStatePtr, mask: xinput::XIEventMask) -> xinput::XIEventMask {
    if client
        .get_client()
        .is_some_and(|c| c.0.borrow().fast_touches.enabled)
    {
        mask | xinput::XIEventMask::TOUCH_BEGIN
            | xinput::XIEventMask::TOUCH_UPDATE
            | xinput::XIEventMask::TOUCH_END
    } else {
        mask
    }
}
fn cancel_window(client: &X11Client, id: u32) {
    let touches = client
        .0
        .borrow_mut()
        .fast_touches
        .contacts
        .cancel_window(id);
    if let Some(window) = client.get_window(id) {
        for touch in touches {
            window.handle_input(gpui::PlatformInput::Touch(touch));
        }
    }
}
fn suppress(client: &X11Client, source: u16, flags: xinput::PointerEventFlags) -> bool {
    let state = client.0.borrow();
    state.fast_touches.enabled
        && state.fast_touches.direct_devices.contains(&source)
        && flags.contains(xinput::PointerEventFlags::POINTER_EMULATED)
}
pub(crate) fn handle_event(client: &X11Client, event: &Event) -> bool {
    let (event, phase) = match event {
        Event::XinputTouchBegin(e) => (e, TouchPhase::Started),
        Event::XinputTouchUpdate(e) => (e, TouchPhase::Moved),
        Event::XinputTouchEnd(e) => (e, TouchPhase::Ended),
        Event::XinputButtonPress(e) | Event::XinputButtonRelease(e) => {
            return suppress(client, e.sourceid, e.flags);
        }
        Event::XinputMotion(e) => return suppress(client, e.sourceid, e.flags),
        Event::FocusOut(e) => {
            cancel_window(client, e.event);
            return false;
        }
        Event::UnmapNotify(e) => {
            cancel_window(client, e.window);
            return false;
        }
        Event::XinputHierarchy(e) => {
            let mut state = client.0.borrow_mut();
            let mut cancelled = Vec::new();
            for info in &e.infos {
                cancelled.extend(state.fast_touches.contacts.cancel_source(info.deviceid));
            }
            let connection = state.xcb_connection.clone();
            state.fast_touches.refresh(&*connection);
            drop(state);
            for (id, touch) in cancelled {
                if let Some(window) = client.get_window(id) {
                    window.handle_input(gpui::PlatformInput::Touch(touch));
                }
            }
            return false;
        }
        Event::XinputDeviceChanged(_) => {
            let mut state = client.0.borrow_mut();
            let connection = state.xcb_connection.clone();
            state.fast_touches.refresh(&*connection);
            return false;
        }
        _ => return false,
    };
    if client.get_window(event.event).is_none() {
        return true;
    }
    let mut state = client.0.borrow_mut();
    let position = touch_position(event.event_x, event.event_y, state.scale_factor);
    let touch = if phase == TouchPhase::Started {
        state
            .fast_touches
            .contacts
            .begin(event.sourceid, event.detail, event.event, position)
            .map(|t| (event.event, t))
    } else {
        state
            .fast_touches
            .contacts
            .update(event.sourceid, event.detail, phase, position)
    };
    let composing = phase == TouchPhase::Started && state.composing && state.ximc.is_some();
    let preedit = if phase == TouchPhase::Started && !composing {
        state.pre_edit_text.take()
    } else {
        None
    };
    if preedit.is_some() {
        if let Some(compose) = state.compose_state.as_mut() {
            compose.reset();
        }
    }
    drop(state);
    if let Some((id, touch)) = touch {
        if let Some(window) = client.get_window(id) {
            if composing {
                client.reset_ime();
                window.handle_ime_unmark();
            } else if let Some(text) = preedit {
                window.handle_ime_commit(text);
            }
            window.handle_input(gpui::PlatformInput::Touch(touch));
        }
    }
    true
}
