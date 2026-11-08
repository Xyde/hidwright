---
id: px-k3jg
status: open
deps: []
links: []
created: 2026-11-08T22:50:01Z
type: feature
priority: 2
assignee: agent
tags: [dumang, keyboard]
---
# Dumang DK6: toggle keyboard layers from a mouse button

User 2026-11-08 wants Dumang 'layer toggling' bindable to the mouse. The DK6 is 0483:5710 (product 'DuMang KeyBoard DK6'). Layer state lives in the keyboard firmware, and a mouse can only send mouse buttons, keys and media keys, so it needs the computer in the middle. Feasible: mayanez/dumang-keyboard-ctrl (GPL-3.0, Python, maintained) has BoardSyncPacket [cmd, 0x01, 0x03 active|0x02 inactive, 0x00, ID, layer_info, layer_info&3, 0x00], which its sync tool sends host→keyboard to switch a board's layer (that's how split halves share layers). Layer key codes: LAYER_n 0xD0-D3, momentary LAYER_TOGGLE_n 0xD4-D7, long-press LAYER_KEY_n 0xDC-DF. Design: bind a mouse button (X2A can send keys) to an unused key such as F13-F24; a small listener (Raw Input with INPUTSINK, observe only, nothing intercepted) sends BoardSync to the DK6. Costs: a small background process while the feature is on (user dislikes background processes, so make it opt-in); works for both mice. Licence: GPL-3.0 upstream; reimplement from the documented packet format rather than porting code, or our repo must be GPL. Verify on the real DK6 first: board count (one USB device here), what ID and layer_info mean, momentary vs latched.

