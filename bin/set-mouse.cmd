@echo off
rem set-mouse: the Rust version (target\release\set-mouse.exe) for everything except the portal,
rem which is still Python (ticket px-01cy). Without a Rust build, the Python does it all.
if /i "%~1"=="portal" goto python
if not exist "%~dp0..\target\release\set-mouse.exe" goto python
"%~dp0..\target\release\set-mouse.exe" %*
exit /b %errorlevel%
:python
python "%~dp0..\set_mouse.py" %*
