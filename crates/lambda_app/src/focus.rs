//! Keyboard focus order for the GPUI shell (Tab / Shift-Tab).
//!
//! GPUI 0.2.2 exposes `tab_index` / `window.focus_next()`, but this app uses custom
//! text fields and a write-confirm dialog that must trap focus. The shell therefore
//! owns an explicit [`FocusTarget`] cycle (still driven by GPUI `KeyBinding` actions).

use crate::state::ShellState;

/// One keyboard-focusable control in the shell.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FocusTarget {
    Host,
    Port,
    Unit,
    Connect,
    Disconnect,
    Refresh,
    Group(String),
    Filter,
    Draft(String),
    Write(String),
    ClearAllActive,
    ClearActive(String),
    DialogCancel,
    DialogConfirm,
}

impl FocusTarget {
    pub fn is_text_field(&self) -> bool {
        matches!(
            self,
            Self::Host | Self::Port | Self::Unit | Self::Filter | Self::Draft(_)
        )
    }

    pub fn is_dialog(&self) -> bool {
        matches!(self, Self::DialogCancel | Self::DialogConfirm)
    }

    /// Param id when this target edits a draft field.
    pub fn draft_param_id(&self) -> Option<&str> {
        match self {
            Self::Draft(id) => Some(id.as_str()),
            _ => None,
        }
    }
}

/// Build the active Tab cycle from shell state.
///
/// When a confirm dialog is open, only dialog buttons are included (focus trap).
/// Disabled actions (e.g. Verbinden while connected) stay in the order so Tab
/// order stays stable; activation is gated separately.
pub fn focus_order(state: &ShellState) -> Vec<FocusTarget> {
    if state.pending.is_some() {
        return vec![FocusTarget::DialogCancel, FocusTarget::DialogConfirm];
    }

    let mut order = vec![
        FocusTarget::Host,
        FocusTarget::Port,
        FocusTarget::Unit,
        FocusTarget::Connect,
        FocusTarget::Disconnect,
        FocusTarget::Refresh,
    ];

    for group in &state.groups {
        order.push(FocusTarget::Group(group.clone()));
    }

    order.push(FocusTarget::Filter);

    for row in state.filtered_parameters() {
        if row.writable {
            order.push(FocusTarget::Draft(row.param_id.clone()));
            order.push(FocusTarget::Write(row.param_id.clone()));
        }
    }

    order.push(FocusTarget::ClearAllActive);
    for entry in &state.active_entries {
        order.push(FocusTarget::ClearActive(entry.key.clone()));
    }

    order
}

/// Move focus forward or backward within `order`, wrapping at the ends.
pub fn cycle_focus(
    current: Option<&FocusTarget>,
    order: &[FocusTarget],
    forward: bool,
) -> Option<FocusTarget> {
    if order.is_empty() {
        return None;
    }
    let Some(cur) = current else {
        return if forward {
            order.first().cloned()
        } else {
            order.last().cloned()
        };
    };
    let idx = order.iter().position(|t| t == cur);
    let next = match (idx, forward) {
        (Some(i), true) => (i + 1) % order.len(),
        (Some(i), false) => {
            if i == 0 {
                order.len() - 1
            } else {
                i - 1
            }
        }
        (None, true) => 0,
        (None, false) => order.len() - 1,
    };
    order.get(next).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ActiveRow, PendingConfirm, ShellState};
    use crate::strings;

    fn state_with_writable() -> ShellState {
        let state = ShellState::new(true);
        // Ensure Allgemein (or first group) has at least the catalog params loaded.
        assert!(!state.parameters.is_empty());
        state
    }

    #[test]
    fn main_order_starts_with_connection_then_groups() {
        let state = state_with_writable();
        let order = focus_order(&state);
        assert_eq!(
            &order[..6],
            &[
                FocusTarget::Host,
                FocusTarget::Port,
                FocusTarget::Unit,
                FocusTarget::Connect,
                FocusTarget::Disconnect,
                FocusTarget::Refresh,
            ]
        );
        let groups: Vec<_> = order
            .iter()
            .filter_map(|t| match t {
                FocusTarget::Group(g) => Some(g.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(groups, strings::NAV_GROUPS.to_vec());
        assert!(order.contains(&FocusTarget::Filter));
        assert!(order.contains(&FocusTarget::ClearAllActive));
        assert!(!order.iter().any(|t| t.is_dialog()));
    }

    #[test]
    fn writable_params_add_draft_and_write_targets() {
        let state = state_with_writable();
        let order = focus_order(&state);
        let writable: Vec<_> = state
            .filtered_parameters()
            .into_iter()
            .filter(|r| r.writable)
            .map(|r| r.param_id.clone())
            .collect();
        assert!(
            !writable.is_empty(),
            "catalog should expose at least one RW param in the default group"
        );
        for id in writable {
            assert!(order.contains(&FocusTarget::Draft(id.clone())));
            assert!(order.contains(&FocusTarget::Write(id)));
        }
    }

    #[test]
    fn dialog_traps_focus_to_cancel_and_confirm() {
        let mut state = state_with_writable();
        state.pending = Some(PendingConfirm::ClearAllActive);
        assert_eq!(
            focus_order(&state),
            vec![FocusTarget::DialogCancel, FocusTarget::DialogConfirm]
        );
    }

    #[test]
    fn active_clear_actions_appear_in_order() {
        let mut state = state_with_writable();
        state.active_entries.push(ActiveRow {
            key: "p:1".into(),
            label: "Test".into(),
            display_value: 1.0,
            last_write_at: None,
            last_error: None,
        });
        let order = focus_order(&state);
        assert!(order.contains(&FocusTarget::ClearActive("p:1".into())));
    }

    #[test]
    fn cycle_wraps_forward_and_backward() {
        let order = vec![FocusTarget::Host, FocusTarget::Port, FocusTarget::Unit];
        assert_eq!(cycle_focus(None, &order, true), Some(FocusTarget::Host));
        assert_eq!(cycle_focus(None, &order, false), Some(FocusTarget::Unit));
        assert_eq!(
            cycle_focus(Some(&FocusTarget::Host), &order, true),
            Some(FocusTarget::Port)
        );
        assert_eq!(
            cycle_focus(Some(&FocusTarget::Unit), &order, true),
            Some(FocusTarget::Host)
        );
        assert_eq!(
            cycle_focus(Some(&FocusTarget::Host), &order, false),
            Some(FocusTarget::Unit)
        );
        assert_eq!(
            cycle_focus(Some(&FocusTarget::DialogCancel), &order, true),
            Some(FocusTarget::Host)
        );
    }

    #[test]
    fn cycle_on_empty_order_is_none() {
        assert_eq!(cycle_focus(None, &[], true), None);
        assert_eq!(cycle_focus(Some(&FocusTarget::Host), &[], false), None);
    }
}
