# Radar

Enable **Visuals > Radar**. Filter: Opponents, Mission Players (teammates and opponents), or All Players. Dot colors use the ESP color settings. White marks your character at the cross center.

The horizontal/vertical cross stays fixed on screen; only nearby dots rotate with camera heading. Range is 100 meters on the horizontal plane; actors outside the range are omitted. Actors behind the camera remain eligible. Positions come from verified local and remote pawn ownership, independently of screen projection and body bounds.

Placement supports eight viewport anchors, Center, and Custom. Custom X/Y are center coordinates in percent of the current APB client viewport. Scale adjusts radar size from 0.5 to 2.0 (default 1.0); at 1.0, the cross spans 22% of the shorter viewport dimension. The 100-meter range stays unchanged. Its full footprint, including dots and an edge margin, stays within the viewport at every rotation. Window moves, viewport size changes, and monitor changes follow the existing overlay client rectangle.

Radar has its own enable setting and filter and works with ESP and Aim Visuals disabled. Settings persist in existing profiles; profiles without radar settings start with radar disabled. Missing local character position withholds the memory frame and overlay while verified ownership is reacquired, with the reason recorded in the diagnostic log. Camera changes during the final frame recheck also discard the frame and use the existing camera reacquisition path. Persistent unexpected read failures still stop processing with a visible error after the existing 30-second limit. An invalid camera heading stops processing with a visible error and diagnostic log. Lost focus withholds the overlay through the existing memory pipeline.

Validation: `cargo check --all-targets`.

Live verification: enable radar in a populated district; turn through 360 degrees, check nearby teammate/opponent colors and actors behind the camera, change filters, confirm the cross stays fixed, try every placement including Center, adjust Scale across its range, set Custom to 0/100 percent at each edge, resize/move the game window, and confirm the overlay hides when APB loses focus.
