---
id: px-5juk
status: open
deps: []
links: []
created: 2026-11-10T00:02:53Z
type: task
priority: 4
assignee: agent
parent: px-w1a0
tags: [viper, buttons]
---
# Viper: map side button ids 04-07 to physical positions

Known: left front = forward, left back = back, right front = forward, right back = back; table 04 back, 05 forward, 06 back, 07 forward. So {04,06} are the two back buttons and {05,07} the two forward buttons, but which side each id is on is unknown. Low value: both sides do the same. Only needed if the user wants different functions per side, then the labels side04-side07 in viper::BUTTONS should become e.g. left-back/right-front. Method (2 presses): temporarily bind 04 to middle and 05 to middle (set-mouse razer bind side04 middle), run viper_press_guided.py "LEFT side, BACK button" "LEFT side, FRONT button" (middle shows as an event), then set both back to default. Ask the user first; they prefer few manual steps.


## Notes

**2026-11-10T00:03:01Z**

Correction to the method: viper_press_guided.py ignores left/right/middle clicks, so binding to middle shows nothing. Use opposite functions instead: set-mouse razer bind side04 forward; set-mouse razer bind side05 back; run viper_press_guided.py "LEFT side, BACK button" "LEFT side, FRONT button". Left back = forward means 04 is left back (else 06); left front = back means 05 is left front (else 07). Then bind side04 default and side05 default.
