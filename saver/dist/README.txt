Proof Saver 0.1.0
=================

Strange attractors from Proof Engine as a Windows screensaver. Every point
on screen is moved by the attractor's equations, integrated with RK4 each
frame. Nothing is a video.

Install
-------
1. Put proof-saver.scr somewhere it can stay (Windows runs it from there).
2. Right-click proof-saver.scr and choose "Install".
   Screen Saver Settings opens with Proof Saver selected. Click OK.
3. "Settings..." in that dialog opens Proof Saver's own settings.
   "Preview" there runs it fullscreen; move the mouse or press a key to end.

To try it without installing: right-click proof-saver.scr and choose "Test".

Windows may show a SmartScreen warning because the file is not code-signed.
The source is at https://github.com/Mattbusel/proof-engine (folder saver/).

Settings
--------
Attractor (cycle through all ten, or one), time on each, motion speed,
frame rate cap (24/30/60), what other monitors show, and whether the
equations appear. Saved in %APPDATA%\ProofSaver\settings.toml.

Live art
--------
proof-saver.exe in this zip is the same program. Double-click it for the
settings, or from a command prompt:
  proof-saver.exe --window        Esc quits, F11 fullscreen, Right arrow skips
  proof-saver.exe --fullscreen    Esc quits
  proof-saver.exe --help

Needs a GPU with OpenGL 3.3.

Uninstall
---------
Choose another screensaver (or None) in Screen Saver Settings, then delete
proof-saver.scr and the folder %APPDATA%\ProofSaver.

MIT licensed. Matthew Busel.
