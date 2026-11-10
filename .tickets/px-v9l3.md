---
id: px-v9l3
status: open
deps: []
links: []
created: 2026-11-10T00:06:15Z
type: task
priority: 2
assignee: user
tags: [ci, macos]
---
# Decide: should CI upload the built set-mouse (Windows + macOS) for download?

Asked 2026-11-10: the user wants to "create the app with GitHub Actions". Today ci.yml builds and tests on windows-latest and macos-latest but keeps nothing. Option: add actions/upload-artifact after the release build (set-mouse.exe + set-mouse-startup.exe on Windows; set-mouse on macOS), downloadable from each run page for 90 days by default. Later option: GitHub Releases on a version tag. Helps px-e6p0 (Mac Studio) without building on the Mac. User decides; agent then edits ci.yml and checks one run.

