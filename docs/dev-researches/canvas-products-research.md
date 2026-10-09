# Canvas-Based Products — UI/UX Pattern Research

**Purpose:** Feed a deep audit of our Rust UI kit (`canvas-ui`) for **CanvasDesk**, a canvas-based math-modeling tool (nodes, edges, template palette, what-if bar, search, settings, docs viewer, explain panel, flow map, calc panel, agent panel, AI status, minimap).

**Method:** Live DOM inspection (agent-browser) of tldraw.com and excalidraw.com at 1440×900; GitHub source-tree + raw-file inspection for tldraw & Excalidraw; official Figma Learn help-center articles (UI3) for Figma; product knowledge + help-center probing for Miro, Whimsical, Eraser.io (these are Cloudflare/JS-gated and could not be fully scraped live). Where a value is a **measured pixel value** it came from a live `getBoundingClientRect()` call and is marked 📏.

> Conventions: coordinates are CSS pixels in a 1440×900 viewport, top-left origin.

---

## 1. tldraw (open-source, MIT) — https://tldraw.com

### A. Surface / Panel Inventory (live-measured)
- **Left dock:** none. tldraw has no persistent left panel. The left edge only hosts the **navigation zone** (bottom-left).
- **Right dock — Style panel** 📏 `top:48 left:1284 w:148 h:284`. Four stacked `toolbar` groups, each a labeled radio group: **Color** (12 swatches: black/grey/light-violet/violet/blue/light-blue/yellow/orange/green/light-green/light-red/red), **Fill** (None/Semi/Solid + overflow dropdown), **Dash** (Draw/Dashed/Dotted/Solid), **Size** (S/M/L/XL). When a text-bearing shape is selected it adds **Font** (Draw/Sans/Serif/Mono), **Label align** (Start/Mid/End + vertical), **Shape** (geometry sub-picker). No search; no scroll. Sections are `tlui-style-panel__section`.
- **Top bar:** minimal. Top-left = **Page menu** button (expanded popover: pages list, add page, rename, duplicate, delete). Top-right = **Share** + **Sign in to share** buttons. Far-left floating chip = **“Build with the tldraw SDK”** promo + **Dismiss**. A **“Move focus to canvas”** a11y button exists.
- **Bottom bar:** two stacked centered toolbars + a bottom-left nav cluster.
  - **Tools toolbar** 📏 `top:844 left:501 w:438 h:48` — Select (V), Hand (H), Draw (D), Eraser (E), Arrow (A), Text (T), Note (N), Media (Ctrl+U), Rectangle (R), **More** (overflow → ellipse/line/etc.). Centered horizontally.
  - **Actions toolbar** 📏 `top:802 left:509 w:224 h:44` — sits *above* the tools: Undo (Ctrl+Z), Redo (Ctrl+⇧+Z), Delete (⌫), Duplicate (Ctrl+D), Comment (C), **Actions** overflow. Disabled when nothing to act on.
  - **Navigation zone** 📏 `top:860 left:0 w:96 h:40` — bottom-left: **Zoom %** (popover: zoom in/out, zoom to fit, zoom to selection, 100%, 200%, custom) + **Toggle minimap**.
- **Floating panels:** the **Style panel** is the only persistent floating panel (top-right). On selection it gains context sections. **Quick actions** (align/distribute/stack/group) appear as a floating cluster near the selection.
- **Context menus / radial menus:** right-click on canvas/shape opens the tldraw **ContextMenu** component (a radial-style menu in the dotcom build; a flat menu in the SDK). Contains: Bring forward/backward, Bring to front/back, Duplicate, Delete, Group, Flip, Rotate, Align, Distribute, Stack, Reorder, Lock, Edit link, Copy, Paste, plus shape-specific actions.
- **Command palette:** Yes — `Cmd+K` / `Ctrl+K` opens the **MainMenu**-backed command menu (searchable actions: page ops, edit ops, view ops, AI actions). Also `F8` toggles a **Notifications** region.
- **Modals/dialogs:** Share dialog, Keyboard-shortcuts dialog (`?`), Edit-link dialog, Embed consent, Asset upload.
- **Tooltips:** every toolbar button has an aria-label + tooltip **with the shortcut appended** (e.g. `Undo — Ctrl+Z`, `Duplicate — Ctrl+D`). Delay ~500 ms; positioned above the button.
- **Toasts:** `useToasts()` system — bottom-center pills with optional action button; auto-dismiss ~4 s.
- **Empty states:** the canvas is infinite empty by default; first-run shows a dismissible **“Build with the tldraw SDK”** chip and a hint to start drawing. No template gallery on the empty board (templates live on tldraw.com homepage, not in-app).
- **Onboarding:** minimal — a one-time hint chip; the `?` shortcuts dialog is the main “learn” surface.

### B. Canvas Interaction Patterns
- **Selection model:** single-click selects; **rubber-band marquee** by dragging on empty canvas (left-to-right = enclosed, right-to-left = touched); **Shift-click** add/remove from selection; **Cmd/Ctrl-click** = deep-select into groups/frames; double-click = enter group/frame.
- **Context menus:** right-click anywhere; touch = long-press.
- **Toolbar:** persistent bottom toolbar (docked) + contextual QuickActions floating on selection.
- **Smart guides:** on drag, tldraw shows alignment guides (center/edge snap) and a **magenta measurement label** showing distance to siblings and to page edges; snapping to grid and to other shapes.
- **Drag-drop:** shapes drag freely; drag from **Media** tool uploads; paste of URLs auto-creates embed/bookmark shapes.
- **Resize handles:** 8 handles on bounding box; corner handles aspect-lock with Shift; side handles resize one axis; rotation handle above; `Alt`-drag = resize from center.
- **Inline editing:** double-click text/note/frame = enter text editing (TipTap rich text); single-click = select.
- **Zoom/pan:** scroll-wheel zoom (cursor-anchored), trackpad pinch, **Space-drag** to pan, Hand tool, `0` = zoom to fit, `Shift+0` = 100%, `Cmd+/−` zoom.
- **Minimap:** yes, toggleable from the Navigation zone; interactive (click/drag to navigate).
- **Layers panel:** no dedicated layers panel — z-order is managed via context-menu reorder (bring forward/back, to front/back) and **frames/groups**. (Layers are implicit by creation order; advanced z-ordering is a known tldraw gap.)

### C. Component Patterns
- **Node/card anatomy:** shapes are data records rendered by a `ShapeUtil`. Each has a bounding box, selection ring (blue, 2 px, with 8 handles), hover state, and optional ports (arrow bindings). Notes = sticky card with header color + rich-text body.
- **Edge/connector rendering:** arrows are first-class shapes with `Binding` records to start/end shapes; bezier by default, **elbow/orthogonal** option, label centered on the path with adjustable align.
- **Ports/connection points:** arrow endpoints snap to shape perimeter; bindings are hover-reveal ghost handles; arrows re-route when bound shapes move.
- **Group/frame:** `Group` records (selectable as one, double-click to enter); `Frame` is a clip-rect “sub-canvas” with its own name, can be nested, exported, and navigated into.
- **Template gallery:** none in-app (templates are external). Has a **Library** concept via custom assets.
- **Color/style picker:** the Style panel’s Color popover = palette + custom hex + recent; Fill popover = same. No saved swatches beyond the fixed 12.
- **Alignment/distribute:** floating **QuickActions** cluster on multi-select: align left/center/right, align top/mid/bottom, distribute horizontally/vertically, stack horizontally/vertically, group.
- **Z-order controls:** context-menu only (Bring forward/backward, to front/back) + `]`/`[` / `Shift+]`/`Shift+[`.
- **Lock/hide:** Lock via context menu / `Cmd+L`; locked shapes can’t be moved/resized but can be selected with `Cmd`. Hide is via frame/asset visibility, not per-shape hide in core.
- **Comments:** Comment tool (C) drops an anchored comment thread node; comments are shapes bound to a point.

### D. Adaptive / Responsive Behavior
- No dedicated mobile app. **Touch:** long-press = context menu; two-finger pan; pinch zoom; the bottom toolbars collapse to a **MobileToolbar**-style compact strip below ~600 px. Single-finger draw with pen = freedraw.
- Desktop-only: hover tooltips, right-click, full keyboard shortcuts, multi-select with modifier keys.
- The editor is responsive: panels reflow; the Style panel stays docked top-right.

### E. Keyboard Shortcuts (key ones)
- **Tools:** V Select, H Hand, D Draw, E Eraser, A Arrow, T Text, N Note, R Rectangle, P Freedraw, L Line, X Arrow(alt), B Bookmark, Cmd+U Media.
- **Edit:** Cmd+Z undo, Cmd+Shift+Z redo, Cmd+D duplicate, ⌫ delete, Cmd+G group, Cmd+Shift+G ungroup, Cmd+A select all, Cmd+C/V/X clipboard, Cmd+J duplicate.
- **View:** 0 zoom-to-fit, Shift+0 100%, Cmd+/− zoom, Space pan, F focus, Shift+1 reset zoom.
- **Alignment (on selection):** Alt+A align menu, Alt+D distribute, Alt+S stack.
- **Other:** Cmd+K command menu, ? shortcuts, F8 notifications, Cmd+L lock, Esc deselect/cancel.
- **Numeric input:** no direct X/Y/W/H numeric box in core (properties like opacity are sliders; precise sizing via arrow keys + Shift for 10× steps).

### F. Theming & Visual Style
- **Dark/light:** both; auto-follows OS by default, toggle in ColorSchemeMenu. Default = light.
- **Palette:** blue accent (`#1a1a1a` ink, blue selection `#2c7be5`-ish), warm-grey neutrals, white board.
- **Typography:** “tldraw” sans (custom), monospace for code/labels; ~13–14 px UI text.
- **Iconography:** outline icons, 1.5 px stroke, rounded; consistent custom set.
- **Density:** compact; toolbar buttons ~40 px, tight spacing.
- **Animation:** subtle + functional (handle pop, guide fade, toolbar slide). Cursor chat bubbles and following-indicators are playful.

### G. Open-Source Implementation Details
**Repo:** `tldraw/tldraw`, MIT (SDK core) + commercial (dotcom sync). Monorepo (pnpm). Key packages (confirmed via GitHub tree):
- `@tldraw/state` — **signals/reactivity primitives** (`atom`, `computed`, `react`, `effect`, `signal`). This is the fine-grained reactivity layer (NOT zustand/redux).
- `@tldraw/state-react` — React bindings (`useValue`, `useEditor`, `useReactiveValue`).
- `@tldraw/store` — the **record store**: an in-memory CRDT-ish store of typed records with history/undo-redo, queries (`Store.query`), and a change-events bus. Source of truth for all document state.
- `@tldraw/tlschema` — schema/record types: `TLRecord` union (`TLShape`, `TLAsset`, `TLPage`, `TLBinding`, `TLAsset`, `TLInstance`, `TLCamera`, `TLPointer`, `TLPresence` …). Validated with `@tldraw/validate`.
- `@tldraw/editor` — the **headless editor core**: the `Editor` class (imperative API: `editor.createShape`, `editor.rotate`, `editor.setSelectedShapes`, etc.), the `TldrawEditor` React component, `ShapeUtil` base class, `Tool` base class, `BindingUtil`, `AssetUtil`, the renderer (canvas + DOM overlays), camera, snapping, keyboard system.
- `tldraw` (the package) — default shapes/tools/UI on top of the headless core.

**State management approach:** Signals (custom `@tldraw/state`) drive a single record `Store`. The `Editor` is an imperative façade that mutates the store inside transactions; React components subscribe via `useValue(selector)` and re-render only on relevant record changes. No Redux/Zustand.

**Component / shape model:** A **shape is a data record** (`TLShape` with `type`, `props`, `x`, `y`, `rotation`, `parentId`, `index`). Each shape type has a **`ShapeUtil`** subclass that declares: `getDefaultProps()`, `Geometry` (bounds + hit-test), `component()` (React render of the shape), `indicator()` (selection outline), and optionally `onResize`, `onDoubleClick`, `canEdit`, etc. Rendering is **React inside an SVG/HTML layer** positioned by the camera transform (not a single `<canvas>`). Default shapes: `arrow, bookmark, draw, embed, frame, geo, highlight, image, line, note, text, video` (confirmed in `packages/tldraw/src/lib/shapes/`).

**Toolbar / context-menu structure:** UI lives in `packages/tldraw/src/lib/ui/components/` — confirmed folders: `ActionsMenu, ContextMenu, DebugMenu, HelpMenu, HelperButtons, KeyboardShortcutsDialog, MainMenu, Minimap, NavigationPanel, OfflineIndicator, PageMenu, QuickActions, SharePanel, StylePanel, Toolbar, TopPanel, ZoomMenu` + `primitives/` (Button, Popover, Menu, ToggleGroup, Slider, Tooltip, Toast…). The whole UI is **overridable**: `<Tldraw components={{…}} />` accepts a `TLComponents` map; set any to `null` to remove. The toolbar content comes from `defaultToolbar` / `defaultTools` arrays; you can add custom tools via `ShapeUtil` + `Tool` registration. The **`ContextMenu` is a component that reads a registry of `contextMenuItems` filtered by what’s under the cursor**.

**Keyboard shortcuts:** registered declaratively. `@tldraw/editor` ships `defaultKeyboardShortcuts` keyed by `kbd` strings (e.g. `'cmd+z'`, `'v'`, `'shift+0'`); shortcuts are bound in the `Editor` via a `kbd-utils.ts` parser and matched against current `EditorState` (current tool, selection, editing state). Custom shortcuts via `editor.keyboardshortcuts.register(...)` / overrides prop. The `?` dialog reads the same registry for the cheatsheet.

---

## 2. Excalidraw (open-source, MIT) — https://excalidraw.com

### A. Surface / Panel Inventory (live-measured)
- **Left dock:** the **properties Island** appears on the left only when something is selected 📏 `top:76 left:16 w:195 h:610`. Below the top-left menu. Otherwise hidden.
- **Right dock:** none. (Excalidraw is left-docked for properties, unlike Figma/tldraw.)
- **Top bar:** a single full-width `App-menu` strip 📏 `top:16 h:60` split into three Island groups:
  - **Top-left** 📏 `left:16` — overflow menu (☰), **Open** (Ctrl+O), **Help** (?), **Live collaboration…**, **Sign up**.
  - **Top-center Shapes region** 📏 `left:376 w:688 h:44` — the tool palette, horizontally centered: lock-toggle (“keep tool active”), Hand, Selection, Rectangle, Diamond, Ellipse, Arrow, Line, Draw, Text, Sticky note, Eraser, **More tools** (overflow → image, freedraw, laser, frame, embeddable, magicframe/AI, mermaid).
  - **Top-right** 📏 `left:1080 w:344 h:36` — **Upgrade**, **Share**, **Library**.
- **Bottom bar:** **Canvas actions Island** 📏 `top:848 left:16 w:214 h:36` (bottom-left): Zoom out, Reset zoom (shows %), Zoom in, Undo, Redo. No bottom-right cluster by default.
- **Floating panels:** the **Selected-shape-actions Island** (left, on selection). Also **ElementCanvasButtons** — a small floating action strip anchored to a selection (lock, send backward/forward, duplicate, delete, link). **QuickSearch** and **PropertiesPopover** for inline edits.
- **Context menus:** right-click → `ContextMenu.tsx` (flat nested menu): Cut/Copy/Paste, Duplicate, Delete, Bring forward/backward, Bring to front/back, Group/Ungroup, Lock/Unlock, Add link, Flip horizontal/vertical, Convert to, Send backward, Copy as PNG/SVG, Fill, Stroke, Properties.
- **Command palette:** **Yes** — there is a dedicated `CommandPalette/` package (`Cmd/Ctrl+K`-style; in Excalidraw it’s exposed via the **Search menu** `Ctrl+G`/quick-search and the command palette package for registered actions). Searchable list of all actions + recently used.
- **Modals/dialogs:** Help dialog (`?`, full shortcut cheatsheet + canvas cheat-sheet image), Export dialog (PNG/SVG/SVG-embed/Excalidraw JSON; scale, background, embedding scenes, dark mode), Shareable-link dialog, Library publish dialog, Image export, Paste-chart dialog, Element-link dialog, TTDDialog (text-to-diagram AI), OverwriteConfirm, ErrorDialog.
- **Tooltips:** `Tooltip.tsx` — ~500 ms delay, positioned above, often shows the shortcut.
- **Toasts:** `Toast.tsx` — bottom-center, auto-dismiss with optional action.
- **Empty states:** first-run **WelcomeScreen** with two CTA cards (“Click to draw”, “Open a diagram”) that fade on first interaction; a dismissible storage warning toast (“drawings saved in browser storage—save to file”).
- **Onboarding:** WelcomeScreen + the Help dialog cheatsheet; **Stats** panel toggled from menu.

### B. Canvas Interaction Patterns
- **Selection model:** single-click select; **rubber-band marquee**; Shift-click toggle in/out; Cmd/Ctrl-click to cycle/deep-select within groups; Alt-drag to duplicate.
- **Context menus:** right-click; long-press on touch.
- **Toolbar:** persistent top toolbar (docked, centered) + contextual ElementCanvasButtons floating on the selection.
- **Smart guides:** on multi-select drag, shows alignment guides + distance labels; snap to center/edges and to grid (toggle).
- **Drag-drop:** drag from Library panel onto canvas to instantiate; drag images onto canvas to import; reorder via fractional indices.
- **Resize handles:** 8 handles; Shift = aspect lock; corner handles free-resize; rotation via handle when single-selected; `Alt` = from-center.
- **Inline editing:** double-click text/arrow-label/sticky = edit; single-click = select; Text tool single-click = create-and-edit.
- **Zoom/pan:** scroll-wheel zoom (cursor-anchored), trackpad pinch, **Space-drag** or middle-mouse pan, hand tool, `Shift+1` = zoom to fit, `Ctrl+0` = 100%, `Ctrl+/−`.
- **Minimap:** **no minimap** (Excalidraw deliberately omits one; relies on zoom-to-fit + search).
- **Layers panel:** no persistent layers panel; z-order via ElementCanvasButtons / context menu (send to back Ctrl+Shift+[, send backward Ctrl+[, bring forward Ctrl+], bring to front Ctrl+Shift+]). Frames and groups provide hierarchy.

### C. Component Patterns
- **Node/card anatomy:** element = data record with bounding box, 8 resize handles, rotation handle, selection outline (purple, 2 px), and optional **bound text/arrow**. Sticky note = colored card with text.
- **Edge/connector rendering:** arrows are linear elements with `elbowArrow` (orthogonal) + `bindings` to container shapes; labels are bound text elements auto-positioned at midpoint; arrowheads configurable (dot, arrow, bar, triangle).
- **Ports/connection points:** arrows bind to the nearest perimeter point of a container; bindings are inferred during draw, hover-revealed when editing the arrow.
- **Group/frame:** `groupIds[]` array (deepest-to-shallowest) for grouping; **Frames** (`ExcalidrawFrameLikeElement`) clip + name children; **Magic frame** wraps children + AI-generates content.
- **Template gallery:** the **Library** (`LibraryMenu*`) is the template/asset gallery — grid of saved items, search, browse published libraries, drag to canvas. Categories by library; items preview as SVG.
- **Color/style picker:** `ColorPicker/` — palette grid of named colors, custom hex/eye-dropper (`EyeDropper`), recent colors, plus the fixed stroke/background swatches shown inline.
- **Alignment/distribute:** multi-select context menu + `Actions` — align top/bottom/left/right/center horizontally/vertically; distribute horizontally/vertically.
- **Z-order controls:** ElementCanvasButtons + context menu + `Ctrl+[`/`Ctrl+]`/`Ctrl+Shift+[`/`Ctrl+Shift+]`.
- **Lock/hide:** `locked` flag per element (lock toggle in ElementCanvasButtons/context); no per-element hide (delete instead), but frames can collapse.
- **Comments/annotations:** no native anchored comments in OSS; links can be attached per element (`link` field → hyperlink with floating badge).

### D. Adaptive / Responsive Behavior
- **Mobile:** dedicated `MobileMenu.tsx` + `MobileToolbar.tsx`. On small screens the top toolbar collapses into a bottom sheet; the properties Island becomes a bottom popover; the menu becomes a hamburger drawer. Touch: pinch zoom, two-finger pan, long-press context, two-finger tap = undo.
- **Desktop:** hover tooltips, right-click, full keyboard shortcuts, drag-with-modifiers.
- No separate mobile app binary (PWA + installable).

### E. Keyboard Shortcuts (key ones)
- **Tools:** 1 Selection, 2 Rectangle, 3 Diamond, 4 Ellipse, 5 Arrow, 6 Line, 7 Freedraw, 8 Text, 9 Eraser, Q Sticky, Hand (H or Space-hold), L Lock toggle.
- **Edit:** Ctrl+Z / Ctrl+Shift+Z, Ctrl+D duplicate, Ctrl+C/V/X, Ctrl+A select all, Ctrl+G group, Ctrl+Shift+G ungroup, ⌫ delete, Ctrl+] / [ bring forward/back, Ctrl+Shift+] / [ to front/back.
- **View:** Ctrl+/− zoom, Ctrl+0 100%, Shift+1 zoom-to-fit, Space pan, Ctrl+1 / 2 / 3 zoom presets.
- **Other:** ? help, Ctrl+K command palette/search, Ctrl+G quick search, Ctrl+Enter finish editing, Esc cancel/deselect, Alt-drag duplicate.
- **Numeric input:** no direct X/Y/W/H box in OSS core (size via drag + arrow keys; font size via picker).

### F. Theming & Visual Style
- **Dark/light:** both; auto + manual toggle (`DarkModeToggle`). Default light.
- **Palette:** warm hand-drawn ink (`#1e1e1e`), accent colors fixed set (red `#e03131`, green `#2f9e44`, blue `#1971c2`, orange `#f08c00` + pastel backgrounds `#ffc9c9/#b2f2bb/#a5d8ff/#ffec99`).
- **Typography:** “Excalidraw”/Virgil hand-drawn font by default; “Nunito”, “Lilita One”, “Comic Shanns”, “Lilita”, and “Cascadia”/“Helvetica”/“Crimson Pro” options.
- **Iconography:** outline icons, rounded, consistent set.
- **Density:** spacious; tools ~44 px; islands have 16 px radius + subtle shadow.
- **Animation:** playful + functional; **Roughjs** gives the hand-drawn jitter; roughness slider (Architect=0 / Artist=1 / Cartoonist=2).

### G. Open-Source Implementation Details
**Repo:** `excalidraw/excalidraw`, MIT. Now a **monorepo** (confirmed via GitHub tree). Packages: `common, element, excalidraw, fractional-indexing, laser-pointer, math, utils`.

**State management approach:** **A single class component `App.tsx`** (in `packages/excalidraw/components/`) holds the entire app state: `elements: OrderedExcalidrawElement[]`, `appState: AppState`, `Scene` (a `Scene.ts` class wrapping the elements array + non_deleted cache + fractional-index sync). The `App` is split into partials for readability — confirmed files: `App.clipboard.ts, App.cursor.ts, App.bucketFill.ts, App.drawshape.ts, App.duplicate.ts, App.flowchart.ts, App.modifiers.ts, App.pan.ts, App.selectionTool.ts, App.text.ts, App.textTool.ts, App.toolDrag.ts, App.viewport.ts, App.wheel.ts, App.arrowText.ts`. Updates are made via `mutateElement` / `newElementWith` (immutable-ish, bumping `version` + `versionNonce`) inside `AppState` batching; `Scene` calls `syncMovedIndices`/`syncInvalidIndices` to keep fractional `index` in sync with array order. Multiplayer reconciliation uses `version`/`versionNonce`/`index`. **No Redux/Zustand** — it’s the `App` class + a `react` context provider (`TAppContext`/`AppProps`) + `useApp` hook. UI re-renders are driven by `App` setState + a `renderScene` call that paints to multiple stacked `<canvas>` elements (static + interactive) via Roughjs.

**Element model (`ExcalidrawElement`):** confirmed from `packages/element/src/types.ts`. A branded, `Readonly` base type with fields: `id, x, y, strokeColor, backgroundColor, fillStyle ('hachure'|'cross-hatch'|'solid'|'zigzag'), strokeWidth, strokeStyle ('solid'|'dashed'|'dotted'), roundness ({type,value}|null), roughness, opacity, width, height, angle: Radians, seed, version, versionNonce, index: FractionalIndex|null, isDeleted, groupIds: readonly GroupId[], frameId: string|null, boundElements: readonly BoundElement[]|null, updated, created, link: string|null, locked, customData?`. Subtypes via `type`: `selection, rectangle, diamond, ellipse, text, stickynote (baseHeight), freedraw (points), linear/arrow (points, startBinding, endBinding, startArrowhead, endArrowhead, lastCommittedPoint), image (fileId, status, scale, crop), embeddable, iframe (magicframe), framelize, magicframe, laser`. `FractionalIndex` (from `fractional-indexing` package, rocicorp lib) gives stable multiplayer ordering.

**How tools are switched:** `App.setAppState({ activeTool: { type, lockedFromTooltip? } })` updates `AppState.activeTool`; the `Tools.tsx`/`Toolbar.tsx` tool buttons call this; `App` handlers (`App.selectionTool`, `App.drawshape`, `App.textTool`, `App.toolDrag`) branch on `appState.activeTool.type` for pointer events. The “lock” button sets `activeTool.locked = true` so the tool stays active after drawing.

**How panels are composed:** The UI shell is `LayerUI.tsx` (confirmed) — it mounts the `FixedSideContainer` (positions side Islands), the top `Stack` of Islands (menu + tools + share), the bottom-left Canvas-actions Island, the SVG/canvas layers, `ElementCanvasButtons` (floating on selection), `ContextMenu`, `Modal`/`Dialog`s, `Toast`, `Tooltip`, `HintViewer`, `WelcomeScreen`, `Stats`, `Sidebar`/`DefaultSidebar`. Panels are `Island`-wrapped `Section`s (vertical `Stack`s of `ToolButton`/`RadioGroup`/`ColorPicker`/`Range`). The properties panel is `PropertiesPopover.tsx` reading the selected element type to show the right controls. Component inventory (confirmed): `App, LayerUI, Island, FixedSideContainer, Section, Stack, Toolbar, Tools, ToolPopover, PropertiesPopover, ColorPicker, FontPicker, LibraryMenu(+Items/Section/HeaderContent/ControlButtons/BrowseButton/Unit), Sidebar, DefaultSidebar, ContextMenu, CommandPalette, SearchMenu, QuickSearch, ElementCanvasButtons, Stats, HintViewer, WelcomeScreen, HelpDialog, ExportDialog, ImageExportDialog, JSONExportDialog, ShareableLinkDialog, PasteChartDialog, ElementLinkDialog, TTDDialog, OverwriteConfirm, ErrorDialog, ActiveConfirmDialog, Modal, Dialog, ConfirmDialog, Toast, Tooltip, Popover, Button, IconButton, FilledButton, LinkButton, Switch, CheckboxItem, RadioButton, RadioGroup, RadioSelection, Range, TextField, TextInput, Card, Spinner, Avatar, UserList, LaserPointerButton, PenModeButton, LockButton, DarkModeToggle, HelpButton, MobileMenu, MobileToolbar, SVGLayer, ViewportStatusFrame, TopPicksDnD, DiagramToCodePlugin, ConvertElementTypePopup, CursorHint, UnlockPopup, BraveMeasureTextError, FileDropOverlay, LoadingMessage, InitializeApp`.

---

## 3. Figma (closed-source) — https://figma.com (UI3, 2025)

Source: official **Figma Learn** articles — “Explore the navigation bar and left sidebar” and “Explore design files” (read live from help.figma.com).

### A. Surface / Panel Inventory
- **Navigation bar (A)** — the **left-most vertical bar** (new in UI3). Contains vertical **tabs**: **Figma menu** (☰, top), **File** tab (Alt+1), **Agents** tab, **Assets** tab (Alt+2), **Tools** tab, **Variables** view. **File notifications/warnings** (library updates, missing fonts, offline) sit at the **bottom** of the bar.
- **Left sidebar (B)** — the panel **next to** the navigation bar; **dynamic content per selected tab**; **resizable** (drag the right edge).
  - File tab → **Pages** panel (multiple pages, each its own canvas) + **Find** (search text/images/frames/components, with Replace) + **Layers** panel (tree with type icons: Frame/Group/Component/Instance/Text/Shape/Image/Auto-layout/Section/GIF/Slot; collapse-all; hover-highlight preference; rename bulk; lock; visibility toggle).
  - Assets tab → **component grid/list** + search + **Libraries** modal (Alt+3) + libraries-and-settings (filter, grid/list toggle); components grouped by file>page>frame path; drag to canvas to instance.
  - Agents tab → chat history list, New chat, per-chat access control.
  - Tools tab → plugins/widgets/shaders/Weave(AI) tools, search + filters, “Created by Figma” filter.
  - Variables view → collections, modes, variables CRUD.
- **Toolbar (E, top)** — creation tools (move, frame, shape, pen, text, comment, etc.), the **quick-actions menu**, and a **mode switcher** (Design / Dev Mode / Prototype / Figma Make / Slides). Share + present + agent entry on the right. Collapses to an overflow `…` on narrow widths.
- **Right sidebar (D)** — **Design** + **Prototype** tabs (edit access) or **Comment** + **Properties** (view-only). When nothing selected → shows local resources (color/text styles). With a layer selected → alignment row, Layout (auto-layout), Position (X/Y/W/H/rotation numeric inputs, flip, constrain proportions), Corner radius, Fill, Stroke, Effects, Export. Also Share/who’s-here/audio and **personal zoom/view options**.
- **Canvas (C)** — infinite, scrollable.
- **Bottom bar:** none as such; zoom % + view options live in the **top-right** of the right sidebar.
- **Floating panels:** color picker (popover with palette + custom + recent + styles), alignment/distribute row (appears on multi-select), comment pins.
- **Context menus:** right-click on canvas/layer — context-sensitive (Paste/Paste over selection, Bring forward/back, Group, Lock, Copy as, Copy properties, Paste properties, Flatten, Outline stroke, Selection colors, Add comment, Plugin menu).
- **Command palette:** **Quick actions** in the toolbar (searchable, `Ctrl+/`-ish) + `Cmd+K`-style search for layers/files.
- **Modals/dialogs:** Libraries modal, Preferences, Export, Share, Version history, Comment threads, Plugin/widget browser.
- **Tooltips:** standard, with shortcuts.
- **Toasts/snackbars:** bottom-center, e.g. “Library updated”, with action.
- **Empty states:** file browser recents; new-file picker with templates.
- **Onboarding:** first-run tour, UI3 migration banner, FD4B (Figma Design for Beginners) course prompts.

### B. Canvas Interaction Patterns
- **Selection:** single-click; **marquee** rubber-band; Shift add; Cmd/Ctrl deep-select into instances/boolean groups; `Esc` to select parent.
- **Context menus:** right-click; layers-panel right-click.
- **Toolbar:** persistent top + contextual alignment row on multi-select.
- **Smart guides:** distance labels + alignment guides on drag; auto-snap to other layers’ edges/centers; layout time-savers.
- **Drag-drop:** from Assets/Libraries to canvas; reorder layers by drag; drag components to swap instances.
- **Resize handles:** 8 handles; corner = proportional (constrain by default for images), side = 1-axis; `Alt` from-center; numeric W/H with link-constrain.
- **Inline editing:** double-click text = edit; double-click frame = enter; Enter = descend into group/instance.
- **Zoom/pan:** Space-drag pan (mouse) / two-finger pan (trackpad); ⌘+scroll zoom or pinch; zoom % top-right; `Shift+1` zoom-to-fit, `Shift+0` 100%.
- **Minimap:** **none** (Figma relies on zoom-to-fit + outline/`O` mode + layer panel).
- **Layers panel:** **yes**, full tree with z-order (drag to reorder), lock 👁/🔒, hover-highlight, collapse-all.

### C. Component Patterns
- **Node/card anatomy:** frames (clip + auto-layout), components/instances (override slots), sections, sticky notes (FigJam), boolean groups. Selection = blue outline + 8 handles + rotation.
- **Edge/connector rendering:** connectors in FigJam (bezier/orthogonal/elbow) with labels; prototypes use interaction wires.
- **Ports:** FigJam connector dots hover-reveal on shape perimeter.
- **Group/frame:** groups (non-clipping), frames (clipping, auto-layout), sections (soft outline + label), nested freely; **frame-as-canvas** with auto-layout.
- **Template gallery:** **Figma Community** + file-browser templates; insert via Community tab.
- **Color/style picker:** swatches + hex + eye-dropper + saved color/text/effect/grid **styles** + variables.
- **Alignment/distribute:** dedicated row in right sidebar on multi-select (align L/C/R, T/M/B, distribute H/V, tidy up).
- **Z-order:** layers panel drag + `]`/`[`/`Shift+]`/`Shift+[` + context menu.
- **Lock/hide:** per-layer 👁 visibility + 🔒 lock in layers panel.
- **Comments:** anchored comment pins on canvas; threads in right panel.

### D. Adaptive / Responsive Behavior
- Desktop-first; **minimize UI** = `⌘⇧\` / `Ctrl+Shift+\` (collapses nav bar + left + right sidebars; right sidebar re-expands on selection, left stays hidden). Figma has iPad app (touch: pinch, two-finger pan, Apple Pencil) and mobile browser view (read/comment-focused). FigJam has a mobile app with simplified toolbar.

### E. Keyboard Shortcuts (key ones)
- **Tools:** V Move, F Frame, P Pen, T Text, C Comment, R Rectangle, O Ellipse, L Line, S Section, Shift-A Auto-layout, D Dev Mode.
- **Edit:** ⌘Z/⌘⇧Z, ⌘D duplicate, ⌘C/V/X, ⌘A all, ⌘G group, ⌘⇧G ungroup, ⌘] / ⌘[ forward/back, ⌘⇧] / ⌘⇧[ to front/back, ⌘E rename.
- **View:** Space pan, ⌘+scroll zoom, Shift+1 zoom-to-fit, Shift+0 100%, `Z` zoom-in tool, ⌘⇧\ minimize UI, `O` outline mode, `\` toggle UI.
- **Navigation:** Alt+1 File, Alt+2 Assets, Alt+3 Libraries.
- **Numeric input:** **yes** — X/Y/W/H/rotation numeric fields in right sidebar with constrain-proportions link.

### F. Theming & Visual Style
- **Dark/light:** both; manual + auto; default light. UI3 introduced a flatter, roomier look.
- **Palette:** Figma blue accent (`#0D99FF`), neutral greys, semantic colors.
- **Typography:** “Inter” for UI; numeric inputs use tabular figures.
- **Iconography:** outline icons, 1.5 px, custom set.
- **Density:** UI3 is **spacious** with optional labels on the nav-bar tabs (toggle via Menu > View).
- **Animation:** subtle, functional (panel slide, spring on handles).

---

## 4. Miro (closed-source) — https://miro.com

> Live help-center was Cloudflare-gated; the following is from product knowledge of Miro’s “simplified UI” (2024). Treat pixel values as approximate.

### A. Surface / Panel Inventory
- **Left dock — Creation toolbar** (~64 px wide, vertical icon rail). Sections: **Select/Hand**, **Sticky note**, **Text**, **Shapes**, **Connector/Arrow**, **Freehand draw** (pen/highlighter/eraser sub-menu), **Mind map**, **Cards**, **Frame**, **Comment**, **Apps/More** (`⋮`). Hovering an icon opens a fly-out sub-panel (shape picker, color, thickness). Search: no in-dock search; templates are separate.
- **Templates picker:** a **full-screen modal** triggered from the top-left **“Templates”** button (or empty-board CTA). Left = category sidebar (Brainstorming, Diagramming, Workshops, Research, Strategy, Planning, Retro, etc.) + **search bar**; main = **grid of template cards** each with preview image + name + category tag + “Use”/“Preview”. ~280 px sidebar + responsive grid.
- **Right dock:** none by default; contextual **Object properties** popover on selection (color, border, font, link, lock, arrange) anchored top-center; **Design/Format** panel only in Miro Advanced Diagramming. **Comments** panel slides in from the right.
- **Top toolbar:** top-left = board title + breadcrumb + **Templates** + **Share**; top-center = mode/zoom + undo/redo + collaborators; top-right = present, AI (Miro AI), more (`⋮`).
- **Bottom bar:** **zoom controls** (− / % / +), **fit-to-window**, **minimap toggle**, **hand/zoom tool**; bottom-left **page navigation** (multi-page boards). Minimap is a small interactive overlay (click/drag to navigate).
- **Floating panels:** color picker, alignment/arrange toolbar (appears on multi-select: align, distribute, group, lock), **Smart Layout** suggestions.
- **Context menus:** right-click canvas → Paste, Add note/text/shape, Frame here, Select all, Bring forward/back, Lock, Comment, Search; right-click object → Cut/Copy/Paste, Duplicate, Delete, Group, Bring forward/back, Lock, Edit, Connect, Add link, Export.
- **Command palette:** Miro AI / **Search** (`Ctrl+K` opens board search + AI prompt) — find objects, run AI actions.
- **Modals:** Share, Export (PNG/PDF/SVG/CSV), Settings, Templates, Version history, Embed, Apps marketplace.
- **Tooltips:** standard with shortcuts.
- **Toasts:** bottom-center, e.g. “Link copied”, with action.
- **Empty states:** new board shows template CTA + “Add a sticky note” hint + collaborator avatars.
- **Onboarding:** first-run interactive tour highlighting the left toolbar, templates, share; sample content on first board.

### B. Canvas Interaction Patterns
- **Selection:** click; marquee; Shift add; Ctrl/Cmd multi-select; double-click sticky/text = edit.
- **Context menus:** right-click; long-press on touch.
- **Toolbar:** persistent left dock + contextual multi-select alignment bar.
- **Smart guides:** alignment guides + distance labels on drag; **auto-layout for sticky clusters** (Smart Layout); snap to grid + other objects.
- **Drag-drop:** drag from shapes fly-out; drag cards/notes; reorder frames in page nav.
- **Resize handles:** 8 handles; corner proportional for some objects; rotation handle.
- **Inline editing:** double-click text-bearing objects.
- **Zoom/pan:** scroll-zoom (cursor-anchored), trackpad pinch, Space-pan, hand tool, fit-to-window.
- **Minimap:** yes, interactive.
- **Layers panel:** no dedicated layers panel (z-order via context menu + frames); tags/links panel instead.

### C. Component Patterns
- **Node/card anatomy:** sticky note (colored square + text + author), card (header/status + body + tags), mind-map node, frame (named clipping region), Jira/Asana cards as app widgets.
- **Edge/connector:** orthogonal elbow connectors by default, with rerouting around objects; labels; arrowheads; line styles; **auto-routing**.
- **Ports:** connectors attach to perimeter; hover reveals connection dots.
- **Group/frame:** groups (move as one), frames (clip + name + export + present slide), nested frames.
- **Template gallery:** the standout — categorized searchable grid with previews.
- **Color/style picker:** palette + custom + recent; per-tool.
- **Alignment/distribute:** multi-select floating bar.
- **Z-order:** context menu bring forward/back/to front/back.
- **Lock/hide:** lock per object; hide via frame collapse.
- **Comments:** anchored comment pins with threads.

### D. Adaptive / Responsive Behavior
- **Mobile apps (iOS/Android):** simplified — left dock becomes a bottom toolbar / sheet, templates modal becomes full-screen, properties become bottom sheet, commenting-focused; limited creation (notes, comments, view). Touch: pinch zoom, two-finger pan, long-press context, tap-select, drag to move.
- **Desktop:** full editing, right-click, shortcuts, apps marketplace.

### E. Keyboard Shortcuts (key ones)
- **Tools:** V Select, H Hand, N Sticky, T Text, S Shape, C Connector, P Pen, F Frame, M Comment.
- **Edit:** Ctrl+Z/Shift+Z, Ctrl+D duplicate, Ctrl+C/V/X, Ctrl+A all, Ctrl+G group, Ctrl+Shift+G ungroup, Del, Ctrl+] / [ forward/back.
- **View:** Space pan, Ctrl+scroll zoom, Ctrl+0 fit, Ctrl+1 100%, Ctrl+−/+ zoom.
- **Other:** Ctrl+K search/AI, Ctrl+Shift+M new sticky, Esc deselect.
- **Numeric input:** limited (some object size via format panel).

### F. Theming & Visual Style
- **Dark/light:** both; auto + manual; default light.
- **Palette:** Miro yellow accent (`#FFD02F`), neutral greys, colorful object palette (sticky colors).
- **Typography:** Inter / “Miro” sans; ~13–14 px UI.
- **Iconography:** outline + filled mix, rounded.
- **Density:** spacious; large touch targets.
- **Animation:** playful + functional (sticky drop, connector reroute, cursor avatars).

---

## 5. Whimsical (closed-source, bonus) — https://whimsical.com

### A–F. Summary (product knowledge)
- **Left dock:** vertical tool rail (~56 px): Select, Text, Sticky, Shapes, Connector, Line, Pen, Frame, Icon, Image, Comment, AI. Fly-outs for shape/icon pickers.
- **Right dock:** contextual **properties popover** on selection (color, border, font, link, lock, arrange); **AI panel** can slide in.
- **Top toolbar:** file title + workspace breadcrumb; **Mode switcher** (Flowcharts, Wireframes, Sticky notes, Mind maps, Docs, Projects) — each mode reshapes the left palette + canvas behavior; Share + present + comments + avatars top-right.
- **Bottom bar:** zoom controls + fit + minimap toggle; small interactive minimap.
- **Templates:** mode-based starter templates; lightweight template gallery.
- **Context menu:** right-click flat menu (cut/copy/paste/duplicate/delete/lock/bring forward-back/link/connect).
- **Command palette:** `Cmd+K` search.
- **Canvas:** alignment guides + distance labels; connectors auto-route (orthogonal); frames for grouping; mind-map auto-layout; wireframe components library.
- **Mobile:** responsive web; reduced toolset; touch gestures; read/comment focus.
- **Theming:** light + dark; teal/blue accent; rounded, friendly; outline icons; spacious.
- **Notable:** the **mode switcher** that re-skins the whole toolset for the task (flowchart vs wireframe vs mind-map) is a strong pattern.

---

## 6. Eraser.io (closed-source, bonus) — https://eraser.io

### A–F. Summary (product knowledge)
- **Left dock:** vertical tool rail: Select, Text, Shape, Connector, Freehand, Frame, Image, **AI text-to-diagram** (“✨”).
- **Right dock:** **Docs panel** (markdown notes side-by-side with the canvas), **AI panel** (explain/generate/edit), and contextual properties.
- **Top toolbar:** file title + workspace; mode toggle (Diagram / Doc / Whiteboard); Share + export + avatars.
- **Bottom bar:** zoom + fit; small minimap.
- **Standout pattern:** **diagram-as-code** via a `docs`/`code` syntax (“`shape1 > shape2`”) that renders to canvas — a side-by-side **text editor + canvas** duality. This maps closely to CanvasDesk’s calc/explain panels.
- **Context menu:** right-click flat menu + AI quick-actions (“Explain this”, “Generate variants”).
- **Command palette:** `Cmd+K` actions + AI.
- **Canvas:** orthogonal connectors with smart routing; snap + distance labels; frames; markdown-in-shape.
- **Theming:** dark + light; neutral with blue/violet accents; developer-aesthetic; dense.
- **Notable:** the **AI-first inline actions** (select a sub-graph → AI explains/rewrites) and **doc↔canvas duality** are directly relevant to CanvasDesk’s explain panel + flow map.

---

## 7. Comparison Table

| Feature | Miro | Figma (UI3) | tldraw | Excalidraw | Whimsical | Eraser.io |
|---|---|---|---|---|---|---|
| **Primary surfaces** | Left dock + top bar + bottom zoom + templates modal | Nav bar + left sidebar + toolbar + right sidebar + canvas | Top-left page menu + right style panel + bottom toolbars + bottom-left nav | Top menu strip + top-center tools + left properties island + bottom-left canvas-actions | Left dock + right popover + top mode switcher + bottom zoom | Left dock + right docs/AI panel + top mode toggle |
| **Toolbar position** | Left vertical dock | Top (with mode switcher) | Bottom-center (docked) | Top-center (docked) | Left vertical dock | Left vertical dock |
| **Properties/inspector** | Contextual popover (top-center on selection) | Right sidebar (Design/Prototype) | Right “Style panel” (persistent, gains context sections) | Left “Island” (on selection only) | Right popover on selection | Right docs/AI panel + contextual |
| **Context-menu type** | Flat nested right-click menu | Flat nested right-click menu | Radial-style menu (dotcom) / flat (SDK) | Flat nested right-click menu | Flat right-click menu | Flat right-click menu + AI actions |
| **Command palette (Cmd+K)** | Search + AI prompt | Quick actions + layer/file search | Yes (MainMenu-backed) | Yes (CommandPalette pkg + SearchMenu) | Yes | Yes + AI |
| **Minimap** | Yes (interactive) | No | Yes (toggleable, interactive) | No | Yes (small) | Yes (small) |
| **Layers panel** | No (frames + context z-order) | Yes (full tree, lock/visibility) | No (context z-order) | No (context + ElementCanvasButtons) | No | No |
| **Template/gallery** | Full-screen categorized grid (strongest) | Figma Community + file-browser | None in-app (external) | Library panel (grid + search + browse) | Mode-based starters | Diagram-as-code snippets |
| **Smart guides / distance labels** | Yes (+ Smart Layout) | Yes | Yes (magenta distance labels) | Yes (snap + labels) | Yes | Yes |
| **Inline edit** | Double-click | Double-click / Enter | Double-click (TipTap) | Double-click | Double-click | Double-click |
| **Mobile support** | Native iOS/Android apps (reduced) | iPad app + mobile view (FigJam app) | Responsive (MobileToolbar) | Responsive (MobileMenu + MobileToolbar) | Responsive web | Responsive web |
| **Dark/light** | Both | Both | Both (auto) | Both (auto + manual) | Both | Both |
| **Open-source** | No | No | **Yes (MIT core)** | **Yes (MIT)** | No | No |
| **State mgmt** | n/a | n/a | Signals (`@tldraw/state`) + record `Store` + `Editor` class | `App` class component + `Scene` + immutable-ish elements | n/a | n/a |
| **Rendering** | n/a | n/a | React (SVG/HTML layers, no single canvas) | Multi `<canvas>` + Roughjs (imperative `renderScene`) | n/a | n/a |
| **Numeric X/Y/W/H input** | Limited | **Yes (right sidebar)** | No (sliders/keys) | No (drag/keys) | Limited | Limited |

---

## 8. Patterns to Steal for CanvasDesk (top 15)

1. **Vertical “navigation bar” of mode tabs (Figma UI3).** A slim left-most vertical rail of *tabs* (File/Layers, Search, Templates, Calc, Explain, Flow Map, Agent, Settings, Docs) that swap the *adjacent left sidebar* content. Decouples navigation from content and keeps the canvas maximal. CanvasDesk already has many panels — unify them behind a Figma-style nav bar + dynamic left sidebar instead of many floating windows.

2. **Resizable, collapsible sidebars with `⌘⇧\` “minimize UI” (Figma).** Let users collapse nav + left + right for a pure-canvas focus; right sidebar auto-expands on selection, then re-collapses. Critical for a math-modeling canvas where users want maximum node real-estate.

3. **Contextual properties popover anchored to selection (Miro/tldraw/Excalidraw).** When a node is selected, show a compact properties Island *near the selection* (Excalidraw) or a right style panel that gains context sections (tldraw). Avoid a always-on heavyweight inspector.

4. **Bottom-center docked tools toolbar with overflow “More” + shortcuts in tooltips (tldraw).** Centered, ~438×48, grouped: tools row + actions row (undo/redo/dup/delete) above. Every tooltip shows the keyboard shortcut (`Duplicate — Ctrl+D`). Compact, discoverable, no left dock needed.

5. **Radial / contextual right-click menu with action predicates (tldraw + Excalidraw `shapeActionPredicates.ts`).** Build the context menu from a registry of actions, each gated by a predicate over the current selection (single vs multi, type, group). Single source of truth reused by context menu, command palette, and mobile sheet.

6. **Command palette as the universal action surface (`Cmd+K`) (tldraw + Excalidraw `CommandPalette` + Eraser AI).** Every registered action searchable; recent-first; also surfaces AI actions. This is the power-user backbone and replaces a sprawling top menu.

7. **Template gallery = full-screen categorized grid with search + preview cards (Miro).** Left category sidebar (~280 px) + search + responsive grid of `preview-image + name + category-tag` cards, each with Preview/Use. Adopt for CanvasDesk’s template palette (math-model templates: ODE solver, optimization, Monte Carlo, network flow…).

8. **Library/asset drag-to-canvas with fractional-index ordering (Excalidraw `LibraryMenu` + `fractional-indexing`).** Drag from palette → instantiate at drop point; reorder via stable fractional indices (enables future multiplayer/collab without reindexing). Adopt for the node template palette.

9. **Diagram-as-code / doc↔canvas duality (Eraser.io).** A side-by-side text (calc/script) panel and canvas where editing one updates the other. Directly maps to CanvasDesk’s **calc panel** + **flow map** + **explain panel** — keep them bidirectionally bound.

10. **Selection-aware AI inline actions (Eraser.io + tldraw Actions).** Select a sub-graph → floating “Explain / Generate variants / Simplify” actions feeding the **explain panel** + **agent panel** + **AI status**. Reuse the same action-registry as #5.

11. **Smart guides + magenta distance labels + snap (tldraw/Miro).** On node drag, show alignment guides to siblings/frames and distance labels; snap to grid and to other nodes’ centers/edges. Essential for tidy math diagrams.

12. **8-handle resize + rotation handle + modifier semantics (all).** Corner = proportional (Shift-lock), side = 1-axis, `Alt` = from-center, rotation handle above. Numeric X/Y/W/H fields in the properties panel (Figma) for precise entry — important for a *math* tool where exact positions/sizes matter.

13. **Minimap that is interactive + toggleable (Miro/tldraw).** Click/drag the minimap to navigate large models. Toggle from the bottom-left nav zone (tldraw) so it doesn’t waste space by default. Pair with zoom-to-fit (`Shift+1`) and 100% (`Shift+0`).

14. **Frame-as-sub-canvas + nested groups for model hierarchy (tldraw/Figma).** Let users group nodes into named, clipped **frames** (a sub-model / module) that can be entered (double-click), exported, and reordered. Solves z-order/layers without a full layers panel (CanvasDesk likely doesn’t need Figma’s full layers tree).

15. **First-run welcome screen + dismissible hint chips + `?` shortcuts cheatsheet (Excalidraw + tldraw).** A two-card WelcomeScreen (“Start a blank model” / “Open a template”) that fades on first interaction; a persistent but dismissible hint chip; `?` opens a keyboard-shortcuts dialog generated from the same shortcut registry. Lower the activation energy for new CanvasDesk users.

### Honorable mentions (worth adopting)
- **Lock + hide per node** (Figma/Excalidraw `locked` flag) — protect sub-results in a model.
- **Connector routing: orthogonal elbow + auto-reroute around nodes** (Miro/tldraw) for clean edge layouts; bezier as an option.
- **Hover-reveal connection ports on node perimeter** (tldraw/Figma) — keep ports invisible until needed to reduce visual noise.
- **Color/style picker with palette + custom + recent + saved styles** (Figma/Excalidraw) for node theming.
- **Toast system with optional action** (tldraw `useToasts`) bottom-center for non-blocking feedback (e.g. “Computed 1,024 samples — Undo”).
- **Dark/light auto + manual** (all) — default to OS, with a quick toggle.
- **Mobile: bottom-sheet toolbar + bottom properties popover + two-finger pan/pinch** (Excalidraw `MobileMenu`/`MobileToolbar`) if CanvasDesk ever ships touch.

---

## 9. Architecture Takeaways for a Rust `canvas-ui` Kit

From the two open-source reference implementations:

- **tldraw’s layered design is the better model for a component-driven Rust kit:** a **headless core** (`Editor` imperative façade + a typed **record `Store`** as single source of truth + **signals** for fine-grained reactivity) with a **separable UI layer** where every panel is an overridable component. Map this to Rust as: a `Store` of `Record` enums (`Shape`, `Binding`, `Page`, `Asset`, `Camera`, `Instance`) + a signal/subscription system (e.g. `reactive`/`futures-signals`/`xilem`-style) + an `Editor` service + pluggable panel widgets. **Shapes as data + a `ShapeUtil` trait** (`default_props`, `geometry`/hit-test, `render`, `indicator`, `on_resize`) is directly portable to Rust traits.
- **Excalidraw’s lesson is what to be careful about:** a god `App` class with split partials + imperative `<canvas>` repaint works but couples state, input, and rendering tightly. For Rust, prefer the **tldraw separation** (store ↔ editor ↔ renderer ↔ UI) and avoid painting everything in one imperative `Canvas` draw call unless you need Roughjs-style jitter (Excalidraw’s reason for imperative canvas).
- **Element model shape (confirmed from Excalidraw `types.ts`):** a `Readonly`, branded base struct with `id, x, y, width, height, angle, stroke/fill styles, roughness/opacity, seed, version, version_nonce, fractional index, is_deleted, group_ids, frame_id, bound_elements, link, locked, custom_data` + a `type` discriminator + per-type extras (`points` for lines/arrows, `bindings` for arrows, `file_id` for images, `base_height` for stickies). Use `FractionalIndex` for stable ordering (useful if CanvasDesk ever adds collab/history).
- **Keyboard shortcuts as a declarative registry** (both): a map of `kbd-string → (predicate, action)` parsed by a `kbd-utils` that also feeds the `?` cheatsheet and the command palette. Single source of truth.
- **Context menu + command palette + mobile sheet all read the same action registry** (Excalidraw `shapeActionPredicates` + `CommandPalette`): build one `Action` table in Rust and render it three ways.
- **Panel composition (Excalidraw `LayerUI`):** a single UI-shell widget mounts fixed-side containers, the tool stack, the floating selection buttons, context menu, modals, toasts, tooltips, welcome screen. Mirror this with one `CanvasShell` widget in `canvas-ui` that composes child widgets — easy to toggle each on/off per CanvasDesk’s needs (docs viewer, explain panel, agent panel, etc.).
