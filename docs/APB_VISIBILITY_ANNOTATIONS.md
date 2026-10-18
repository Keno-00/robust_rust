# F8 LOS annotation capture

## Live readable overlay

For the earlier scan-style click-through overlay, use:

```powershell
./scripts/run_apb_component_overlay.ps1 -OutputDirectory C:/Users/Admin/Documents/APB_F8_SCANS/component-overlay-new
```

The launcher and visible waiting window show actual administrator status.
Focus APB and press F8 to show the overlay. F8 starts/stops live projection,
F4 chooses the on-screen character origin nearest the centre reticle, F5 cycles
candidates, and F6 cycles three readable detail pages. The selected character
is tracked by its current instance fingerprint;
expired instances require reselection. F1 labels visible, F2 shooting blocked
(including solid glass), F3 partial/cursor clear and F7 partial/cursor blocked.
These labels describe operator observations, not automatic LOS predictions.

Aim the red reticle at a body region and press F9 to capture the selected
character's raw context, annotated overlay and scene image. Enter a word comment
in the dialog, then refocus APB. Live reads pause while the dialog is open and
when APB loses foreground. F10 pauses/resumes; F11 exits and retains evidence.
The live overlay requires windowed/borderless display if exclusive fullscreen
prevents Windows layered windows from appearing, using the existing overlay
implementation. No alternative renderer is activated.

The first readable card shows world position, registered links, checked
ownership, vehicle attachment, collision height/radius, experimental mission
role and user LOS labels. The second shows eye-height/stance and motion-vector
leads, paired scalar values, character flag bits and numeric health/configuration
candidates. Their meanings remain hypotheses, with no automatic health, death,
velocity or stance classifications. The third shows checked collision component
origin, half-size, half-height and radius, or an explicit unavailable diagnostic.
P markers show character origins. C markers show checked position components,
which can overlap the pawn marker; they are not anatomical joints. The pink
eight-corner box uses the checked collision component's actual world bounds,
with type, signature, reciprocal owner links and dimensions rechecked.
It is a collision box, not a mesh wireframe or skeleton.
Unknown component geometry, bones and triangle topology remain unverified.
Raw contexts are retained for review; screen comments are not native bone IDs.
The marker-294 lifecycle values and readable strings from large unrelated
probes in the earlier raw-field review are not attributed to this selected pawn;
their separate type/ownership requirements remain unresolved. No player name
is guessed from neighboring text.
Bounded candidate surveys refresh every ten seconds and may pause projection
briefly. Incomplete discovery and failed candidate reads remain in events.jsonl;
essential camera/ownership errors visibly pause the overlay until corrected.

Launch from the repository in an elevated PowerShell with APB in a district:

```powershell
./scripts/run_apb_visibility_f8.ps1 -OutputDirectory C:/Users/Admin/Documents/APB_F8_SCANS/visibility-new-session
```

The output parent must exist and the session directory must be new. The existing
Session verifies the APB executable hash and opens EchoDevice. All APB memory
reads use EchoDriver; Windows APIs supply process/window metadata, key state and
cursor coordinates. Screenshots use the existing client capture implementation.
There is no alternate memory reader, visibility implementation or skeleton source.

1. Keep APB foreground and the camera/witness stationary. F8 captures one frame.
2. Wait for CAPTURE COMPLETE, then Alt-tab to the annotation panel.
3. Select the numbered pawn corresponding to the witness, and click **Set selected
   pawn as Current Player**. This denotes the annotation subject, not the local
   controlled pawn. Local pawn candidates are excluded. Numbers mark projected
   pawn origins, not heads or bounding boxes. Overlapping/uncertain candidates
   must be identified independently; leave uncertain observations unknown.
4. Choose an observation: `visible`, `invisible`, `partial_cursor_visible`, or
   `partial_cursor_invisible`. These describe the captured instant, not the
   later scene while editing. The latter two mean partially visible with the
   aim cursor on a visible or invisible part respectively. Observation choices
   determine the saved LOS state; unknown observations use the explicit LOS field.
5. Optionally label head, left/right arm, left/right leg and torso, plus mission
   role and witness/obstruction notes. Labels start unknown on every frame.
6. Save the annotations, then return to APB and press F8 for the next view.
   Close the panel to finish. Unsaved drafts are retained as unfinished evidence.

The selected Current Player carries across frames only by exact address and
instance fingerprint. Missing/changed instances require explicit reselection.
No timer or continuous memory recording runs between F8 captures. A pending
frame blocks another capture until saved. Each frame includes before/after PNGs,
the bounded raw survey and pawn/pointer context, camera snapshots, rejected reads,
ownership exclusions, annotations and timestamps. Capture failures remain visible
in the panel, terminal log and events.jsonl. A failed frame is never labelled as a
successful capture. Review before/after images for motion; reads are sequential,
not atomic, and a still frame is necessary for comparisons.

## Later implementation gates

### Cursor comments and related raw view

After capture, select a pawn and use **Current / Show raws**. This sets the
annotation subject and opens a read-only inspector of the saved pawn probe,
pointer contexts, explicit failed probes, and available registered component
survey bodies. Related pointer contexts are unclassified and can include shared
world state or other actors; they are not assumed to be skeleton components.
No additional live reads are made while inspecting or commenting.

Click the captured image to mark a region in red and type a word comment. Image
zoom and scroll retain coordinates in original screenshot pixels. Select a raw
region, click a hex row or enter a decimal/0x-prefixed byte offset, and inspect
its absolute address and numerical interpretations. These interpretations do
not establish the native field type. **Attach word comment to raw address**
records a raw-only annotation. With a raw offset selected, an image comment
also records that explicit raw association. This is an operator-chosen evidence
link, not a verified screen-point-to-bone/address mapping; comments never change
memory. Save the frame to commit both comment types with captured bytes and
timestamps. Closing retains unfinished comments as drafts. Original screenshots
and memory probes remain unchanged.

Yellow P markers identify recorded pawn origins. The component view lists
registered links with their provenance; it does not invent geometry, skeletal
indices, or a wireframe for unverified component bytes.

The desired settings are target condition **Opponents / Mission players / All
players**, **Visible only**, **Target Visible bone**, and **Sight** under distance.
This collector does not change product targeting or visuals. Role annotations
remain independent of LOS annotations; typed pawn ownership does not establish
human identity. A mission role resolver needs its own verified evidence.

Per-bone visibility requires verified skeletal component ownership, bone
names/indices, parent hierarchy, transform layout and component-to-world
composition. Head/arm/leg observations here describe visible body regions; they
do not identify native engine bones. Validate actual bone positions against
screenshots through movement, animation, occlusion, respawn and different meshes.
No guessed offsets or fixed head-height substitutes qualify as bone evidence.
Rendering recency and inside-screen projection do not establish obstruction LOS.

Define the LOS origin explicitly (camera or firing origin) and verify obstruction
evidence at each target bone. Partial visibility can permit visible-only eligibility
if a verified eligible bone is visible; the aim cursor's current location is a
separate annotation. Fully obstructed targets fail visible-only. Unknown states
must remain visible diagnostics and cannot be accepted as verified visibility.
Sight may show **No LOS** below distance only for verified obstruction, with no
LOS text for verified visibility. Unknown needs an explicit unavailable status;
do not display No LOS or silently treat it as visible. Partial visibility and
eligible bone choices need a documented rule before product implementation.

For offline damage/reference investigation, `scripts/apb_damage_reference_review.py`
searches saved surveys and annotation contexts for exact pawn and candidate-field
address patterns, records changing linked objects and words, and identifies
registered embedded headers that can expose neighboring data in oversized probes.
It also reviews the selected pawn's countdown byte at `+0x707` and state-value
leads reached through `+0x988`. These are unconfirmed diagnostic leads, not product
health/stamina offsets. The command requires a new JSON output path and preserves
the original captures; see the session's `DAMAGE_REFERENCES.md` for the readable
results and reproducible command. It performs no live memory reads.

Syntax/import checks (no new automated tests):

```powershell
python -B -c "import ast, pathlib; ast.parse(pathlib.Path('scripts/apb_visibility_f8.py').read_text())"
python -B scripts/apb_visibility_f8.py --help
```
