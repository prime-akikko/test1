---
name: gpui-modbus-ui
description: Implement or change the GPUI desktop UI for Lambda Modbus (German strings, connection panel, groups HK1/HK2, write confirmation, active setpoints). Use when editing lambda_app, views, dialogs, or UI↔core command/event wiring.
---

# Skill: GPUI Modbus UI

## When to use

- Scaffolding or changing `lambda_app` / GPUI views
- Wiring UI to Internal API commands/events
- Write-confirm dialog, Active Setpoints panel, German copy

## UI principles (v1)

- Desktop window, Windows-primary; GPUI pinned (no floating `main`).
- Layout: connection | group nav | main parameter area | Active Setpoints panel | status bar.
- Defaults visible/editable: IP `10.0.0.6`, Port `502`, Unit ID `1`.
- Buttons: Verbinden / Trennen / Jetzt aktualisieren.
- All user-facing text **German**.
- Groups include at least: Allgemein, E-Manager, Wärmepumpe, Warmwasser, Puffer, Solar, **Heizkreis 1**, **Heizkreis 2**.

## Parameter list

Per row (minimum): name, address, type, scale, unit, raw, scaled display, R/W, quality (`Good` / `Stale` / `Error`).
Search/filter recommended. Enums: readable DE label **and** raw number. RO rows not editable.

## Write flow (mandatory)

1. Validate against catalog (range, type, writable).
2. Show dialog „Wirklich schreiben?“
3. On confirm → `WriteRegister` / `WriteRegisterGroup` with `confirmed: true`.
4. On cancel → no Modbus traffic.
5. After success → refresh affected params; if `cyclic`, appear in Active Setpoints.
6. Joint write groups → one dialog, one group command.
7. Cyclic 60s repeats: **no** new dialog; show status in Active panel.
8. Clear active setpoint stops further rewrites (no Modbus “delete” required).

## Commands / events (UI must not open sockets)

Commands: `Connect`, `Disconnect`, `SaveConnectionSettings`, `ReadAll`, `ReadGroup`, `WriteRegister`, `WriteRegisterGroup`, `ClearActiveSetpoint`, `ClearAllActiveSetpoints`.

Key events: `ConnectionChanged`, `PollStarted`/`PollCompleted`, `ParameterUpdated`/`ParameterError`, `WriteSucceeded`/`WriteFailed`/`WriteRejected`, `ActiveSetpointsChanged`, `CyclicWriteStarted`/`CyclicWriteCompleted`.

## Implementation notes

- Keep views thin: no direct TCP; map events → entity/state updates.
- Do not block the UI thread on Modbus I/O (background executor / channels).
- Persist connection settings and prefs via core stores (`%APPDATA%\LambdaModbusGui\`).
- Prefer mock backend feature for Cloud/CI smoke; GUI AC is local Windows.

## Done checks

- [ ] German strings throughout
- [ ] Confirm dialog on first write; RO locked
- [ ] HK1 and HK2 navigable
- [ ] Active Setpoints panel present
- [ ] Status shows connection + last poll / rewrite hints
