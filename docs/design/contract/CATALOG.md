# CATALOG — capability index (`fg-*`)

> **Machine-scannable role index.** One row per consumable role: canonical class,
> semantic concept, and cross-ecosystem **aliases** so an external system can map
> its own component names onto this contract. Companion machine file: `catalog.json`.
>
> **This file is a VIEW, not a source.** It is generated from `components.css` +
> `patterns/*.css`. Every `.fg-*` role must appear here (or in the `ignore` set of
> `catalog.json`) — the `Catalog coverage` group in `demo/contract-tests.html`
> turns **red** if a role ships without an entry. Update this file in the same
> change that adds a role (see SPEC → Change Protocol).
>
> Prefix `fg-` · 125 roles · groups: Core · Signature · Editorial · Task · CRM · Financial · Media.
> `⬡` marks roles with a `<fg-*>` Web Component in `contract/elements.js`.

**How to map an unknown element:** search this file (or `catalog.json`) for the
concept or an alias of the element you have → the row's `Class` is what to author →
follow `See` for disambiguation between look-alike roles → read the `@dsCard`
specimen in `cards/` and the SPEC Tier-4 entry for full markup.

## Core (50)

| Class | Concept | Aliases | Variants | See |
|---|---|---|---|---|
| `.fg-btn` ⬡ | Clickable action / call-to-action button | Button, Btn, CTA, ActionButton | `primary` `secondary` `ghost` `danger` `disabled` | — |
| `.fg-icon-btn` | Square icon-only button | IconButton, GhostIconButton, ToolButton | `on` | — |
| `.fg-seg` | Segmented control — pick one of a short set | SegmentedControl, ButtonGroup, Segmented, ToggleGroup | — | — |
| `.fg-card` ⬡ | Content card / panel surface | Card, Panel, Surface, Tile | `rule` `sunken` | — |
| `.fg-field` ⬡ | Form field wrapper (label + control + hint/error) | FormField, FieldWrapper, FormControl, FormGroup | — | — |
| `.fg-input` | Text input / textarea | TextField, TextInput, Textarea, Input | `area` `mono` | — |
| `.fg-select` | Native single-select dropdown | Select, Dropdown, Combobox, NativeSelect, Picker | — | — |
| `.fg-radio` | Radio pill — single choice from a set | Radio, RadioButton, RadioGroup, Choice | `on` | group container is .fg-radioset |
| `.fg-check` | Checkbox — independent on/off | Checkbox, Check | — | — |
| `.fg-switch` ⬡ | Binary toggle switch (settings, consent, tweaks) | Switch, Toggle, ToggleSwitch | `on` | — |
| `.fg-search` | Search box (leading glyph + clear affordance) | Search, SearchBox, SearchField, Omnibox | — | — |
| `.fg-chip` ⬡ | Compact chip / tag / filter facet | Chip, Tag, Pill, FilterChip, Token | `neutral` `accent` `success` `warning` `danger` `info` `toggle` `on` | — |
| `.fg-badge` ⬡ | Small status/count badge | Badge, Counter, Label, StatusBadge | `accent` | — |
| `.fg-status` ⬡ | Workflow-readiness status pill with glowing dot | StatusPill, StatusIndicator, StatusDot, Presence | `ready` `warn` `blocked` | connection state → .fg-sync · configurable lanes → .fg-status-lane |
| `.fg-delta` | Directional trend delta (up/down/flat) | Delta, Trend, Change, TrendArrow, Variance | `up` `down` `flat` | — |
| `.fg-money` | Direction-colored currency figure (CRM + finance) | Money, Currency, Amount, CurrencyText, MoneyValue | `pos` `neg` | — |
| `.fg-kpi` ⬡ | KPI / big-number stat card | KPI, StatCard, MetricCard, BigNumber, Metric | — | — |
| `.fg-table` | Data table | DataTable, Table, Grid, DataGrid | `clickable` | sortable header → .fg-th-sort · selection → .fg-bulkbar · row .fg-row--selected |
| `.fg-th-sort` | Sortable table-header control | SortableHeader, SortHeader, ColumnSort | `asc` `desc` | — |
| `.fg-bulkbar` | Bulk-action bar shown when table rows are selected | BulkActionBar, BulkBar, SelectionToolbar, SelectionBar | — | — |
| `.fg-tabs` ⬡ | Tab bar | Tabs, TabBar, TabList, SegmentedTabs | — | individual tab = .fg-tab |
| `.fg-tab` ⬡ | Single tab within a tab bar | Tab, TabItem | `on` | — |
| `.fg-nav` | Vertical navigation rail / sidebar menu | Sidebar, NavRail, NavMenu, SideNav, VerticalNav | `on` | — |
| `.fg-tree` | Nested collapsible navigation tree (depth-indented) | TreeView, FileTree, NavTree, Tree, Explorer | `on` `closed` `leaf` | task subtask rows → .fg-subtask-tree (data, not nav) |
| `.fg-group-head` | Collapsible grouped-list section header | GroupHeader, SectionHeader, CollapsibleHeader, GroupRow | `closed` | — |
| `.fg-page` | Centered page frame (max-width document column) | PageFrame, PageContainer, Container, ContentWrap | — | header → .fg-pagehead |
| `.fg-shell` | Workspace app shell — header/[left·main·right]/footer | AppShell, WorkspaceLayout, AppFrame, Layout, AppChrome | `no-left` `no-right` | — |
| `.fg-toolbar` | Horizontal toolbar / action & filter bar | Toolbar, ActionBar, FilterBar | — | filter selector part = .fg-fsel |
| `.fg-breadcrumb` | Breadcrumb path navigation | Breadcrumbs, Breadcrumb, PathNav | — | — |
| `.fg-pager` | Pagination control | Pagination, Pager, PageNav | `on` | — |
| `.fg-stepper` | Multi-step / wizard progress indicator | Steps, Stepper, Wizard, MultiStepIndicator, ProgressSteps | `on` `done` | single step = .fg-step |
| `.fg-modal` | Centered modal dialog (place inside .fg-scrim) | Modal, Dialog, Popup, Overlay | — | backdrop = .fg-scrim |
| `.fg-scrim` | Full-viewport modal backdrop / overlay layer | Scrim, Backdrop, Overlay, Dimmer | — | — |
| `.fg-drawer` | Edge-anchored slide-in panel | Drawer, SidePanel, Sheet, SlideOver, OffCanvas | — | — |
| `.fg-menu` | Dropdown menu / popover action list | Menu, Popover, DropdownMenu, ContextMenu, ActionList, List | `danger` | — |
| `.fg-tooltip` | Hover/focus tooltip | Tooltip, Hint, PopoverTip | — | — |
| `.fg-toast` | Transient toast notification (stack in .fg-toast-stack) | Toast, Snackbar, Notification, FlashMessage | `success` `danger` `info` | — |
| `.fg-banner` ⬡ | Persistent page-level inline notice | AlertBanner, InlineAlert, Notice, MessageBar, Banner | `info` `warning` `danger` `success` | block aside → .fg-caveat · transient → .fg-toast |
| `.fg-caveat` ⬡ | Block-level callout / attention aside | Callout, AttentionBox, InfoBox, Aside, Admonition | `info` `warn` `rule` | page notice → .fg-banner · in-prose → .fg-callout |
| `.fg-divider` | Divider / separator rule | Divider, Separator, Rule, Hr | `dashed` | — |
| `.fg-avatar` ⬡ | User/entity avatar | Avatar, UserPic, ProfilePic, Gravatar | `more` | overlap group → .fg-avatar-cluster |
| `.fg-avatar-cluster` ⬡ | Overlapping avatar group (multi-assignee) | AvatarGroup, AvatarStack, AvatarCluster, FacePile | — | — |
| `.fg-acc` | Accordion / disclosure (details+summary) | Accordion, Disclosure, ExpandCollapse, Collapsible, Details | — | — |
| `.fg-cmdk` | Command palette / quick-actions launcher | CommandPalette, Spotlight, QuickActions, CommandMenu, CmdK, Launcher | `active` | — |
| `.fg-upload` | File drop zone | FileUpload, Dropzone, Uploader, FilePicker | `drag` | uploaded-file row = .fg-file |
| `.fg-file` | Uploaded-file row | FileRow, AttachmentRow, FileItem | — | — |
| `.fg-skeleton` | Shimmer loading placeholder | Skeleton, Placeholder, ShimmerLoader, ContentLoader | — | — |
| `.fg-empty` | Empty / zero state | EmptyState, ZeroState, NoData, BlankSlate | — | — |
| `.fg-spinner` | Indeterminate loading spinner | Spinner, Loader, ProgressCircle, ActivityIndicator | — | — |
| `.fg-sync` | Connection / sync-state indicator | SyncStatus, ConnectionStatus, ConnectivityIndicator, SyncIndicator | `synced` `syncing` `offline` `error` | workflow readiness → .fg-status |

## Signature (6)

| Class | Concept | Aliases | Variants | See |
|---|---|---|---|---|
| `.fg-slab` | Edge-to-edge value slab / hero figure | ValueSlab, HeroStat, FeatureValue, Slab | — | — |
| `.fg-signature-border` | Expressive animated conic/gradient border wrap | GradientBorder, AnimatedBorder, GlowBorder, ConicBorder | — | — |
| `.fg-gradient-text` | Gradient-filled display text / wordmark | GradientText, Wordmark, GradientHeading | — | — |
| `.fg-band` | Sunken section band / rhythm break | SectionBand, Band, SectionBreak, TintBand | `tint` | — |
| `.fg-dark-panel` | Dark feature panel on a light page | DarkPanel, FeaturePanel, InversePanel, DarkSection | — | — |
| `.fg-accent-swatches` | Renders the alternate accent palette | AccentSwatches, PaletteSwatches, ColorSwatches | — | — |

## Editorial (12)

| Class | Concept | Aliases | Variants | See |
|---|---|---|---|---|
| `.fg-reading-shell` | 3-column reading layout (chapters · column · TOC) | ReadingLayout, ArticleShell, DocLayout, ThreeColReading | — | — |
| `.fg-reading` | Centered reading column | ReadingColumn, ArticleBody, ContentColumn | — | — |
| `.fg-article-title` | Article / document title | ArticleTitle, PostTitle, DocTitle, Headline | — | — |
| `.fg-lede` | Lede / standfirst intro paragraph | Lede, Standfirst, Dek, IntroParagraph | — | — |
| `.fg-prose` | Rich-text / markdown body (full H1–H4, lists, code, tables, quotes) | Prose, RichText, MarkdownBody, Typography, ArticleContent | — | GFM task list, footnotes, heading anchors are prose sub-features |
| `.fg-callout` | In-prose callout / admonition | Callout, Admonition, Note, Aside | `success` `warn` `danger` `note` | app-chrome notice → .fg-banner/.fg-caveat |
| `.fg-codeblock` | Code block with filename bar + syntax tokens | CodeBlock, CodeFence, SyntaxBlock, Snippet | — | — |
| `.fg-kbd` | Keyboard key cap | Kbd, KeyCap, ShortcutKey, Keyboard | — | — |
| `.fg-mark` | Inline highlight / marker | Mark, Highlight, Highlighter | — | — |
| `.fg-chapters` | Chapter navigation list | Chapters, ChapterNav, SectionList | `on` | — |
| `.fg-toc` | Table of contents / on-this-page | TableOfContents, TOC, OnThisPage, PageOutline | `on` `sub` | — |
| `.fg-reading-progress` | Scroll-linked reading progress hairline | ReadingProgress, ScrollProgress, ScrollIndicator | — | — |

## Task (23)

| Class | Concept | Aliases | Variants | See |
|---|---|---|---|---|
| `.fg-kanban` | Kanban board with columns | Kanban, Board, KanbanBoard, ColumnBoard | — | — |
| `.fg-task` | Task / issue card | TaskCard, IssueCard, TicketCard | — | — |
| `.fg-priority` | Priority flag / badge | Priority, PriorityFlag, PriorityBadge | `urgent` `high` `normal` `low` | — |
| `.fg-progress` | Determinate progress / completion bar | ProgressBar, Progress, CompletionBar, Meter | — | — |
| `.fg-tag` | Colored label tag (from accent-alt set) | Tag, Label, ColorTag | `1` `2` `3` `4` | status text → .fg-chip |
| `.fg-checklist` | Checklist / subtask items | Checklist, TodoList, SubtaskList, TaskItems | `done` | — |
| `.fg-assignee` | Assignee / owner (composes avatar) | Assignee, Owner, AssignedUser | — | composes `.fg-avatar` |
| `.fg-due` | Due-date badge | DueDate, Deadline, DueBadge | `overdue` | — |
| `.fg-tasklist` | Flat task list view | TaskList, ListView, IssueList, RowList | — | row = .fg-trow · nested = .fg-subtask-tree |
| `.fg-subtasks` | Subtask count / roll-up | Subtasks, SubtaskCount, ChildTasks | — | — |
| `.fg-subtask-tree` | Nested subtask rows within a task list (data) | SubtaskTree, NestedTasks, TaskHierarchy | — | navigation tree → .fg-tree |
| `.fg-effort` | Effort / story-point estimate | Effort, StoryPoints, Estimate, Points | — | — |
| `.fg-blocked-by` | Blocked-by dependency indicator | BlockedBy, Blocker, Dependency, Blocked | — | — |
| `.fg-comment-count` | Comment / reply count | CommentCount, Comments, ReplyCount | — | — |
| `.fg-cal` | Month calendar grid (doubles as date picker) | Calendar, DatePicker, MonthGrid, MiniCalendar, DateGrid | `today` `sel` `out` `evt` | — |
| `.fg-gantt` | Gantt timeline | Gantt, Timeline, GanttChart, ScheduleBar | `done` `overdue` | — |
| `.fg-status-lane` | Configurable workflow status lane / state label | StatusLane, WorkflowStatus, StateLabel, StatusColumn, Lane | — | lane set container = .fg-status-set |
| `.fg-task-id` | Mono task/issue identifier (CU-1284) | TaskId, IssueKey, TicketId, RefId | — | — |
| `.fg-task-table` | Grouped list-view table (composes .fg-table + .fg-group-head) | TaskTable, GroupedTable, ListTable | — | composes `.fg-table`, `.fg-group-head` |
| `.fg-td-lane` | Colored value table cell (status/category column) | LaneCell, ColoredCell, StatusCell, CategoryCell | — | color utilities .fg-lane--1..5 |
| `.fg-field-chip` | Custom-field key/value cell | FieldChip, CustomField, PropertyCell, FieldCell | `dropdown` `num` `date` `progress` | — |
| `.fg-view-bar` | View switcher bar (composes .fg-tabs) | ViewBar, ViewTabs, ViewSwitcher | — | composes `.fg-tabs` |
| `.fg-time-track` | Logged-vs-estimate time bar (composes .fg-progress) | TimeTracking, TimeLog, LoggedVsEstimate | `over` | composes `.fg-progress` |

## CRM (15)

| Class | Concept | Aliases | Variants | See |
|---|---|---|---|---|
| `.fg-profile` | 360 contact/profile header | Profile, ContactHeader, PersonCard, ProfileHeader | — | — |
| `.fg-facts` | Key/value facts grid | FactList, DetailsList, PropertyList, KeyValue, DefinitionList | — | single item = .fg-fact |
| `.fg-scard` | Compact summary card | SummaryCard, MiniCard, StatCard | — | — |
| `.fg-activity` | Activity timeline / audit feed | ActivityFeed, Timeline, AuditLog, EventFeed, History | `success` `warning` `danger` `neutral` `hollow` | — |
| `.fg-pipeline` | Deal / stage pipeline tracker | Pipeline, StageTracker, DealStages, Funnel, StageBar | `on` `done` | — |
| `.fg-aq` | Action-queue / next-best-action card | ActionQueue, NextBestAction, RecommendedAction, TaskQueue | — | — |
| `.fg-note` | Note / annotation block | Note, StickyNote, Annotation, Memo | — | — |
| `.fg-rule` | Segment / automation rule builder (composes select/input/btn) | RuleBuilder, SegmentBuilder, ConditionBuilder, FilterBuilder, QueryBuilder | — | composes `.fg-select`, `.fg-input`, `.fg-btn` |
| `.fg-cmethod` | Contact-method channel row with consent state | ContactMethod, ContactChannel, ContactRow | `yes` `no` | — |
| `.fg-thread` | Conversation / message thread (chat bubbles) | ConversationThread, MessageThread, ChatThread | — | single bubble = .fg-msg |
| `.fg-msg` | Chat message bubble | Message, ChatBubble, MessageBubble | `out` | — |
| `.fg-convo` | Conversation-list row (inbox item) | ConversationRow, InboxItem, ThreadRow | `on` | — |
| `.fg-segment` | Customer-segment classification chip | Segment, AudienceTag, SegmentChip, CustomerTier | `vip` `risk` `new` | — |
| `.fg-related` | Linked / related-record row | RelatedRecords, LinkedRecords, Associations | — | — |
| `.fg-quick-actions` | Icon-button cluster under a profile header | QuickActions, ActionRow, ShortcutBar | — | — |

## Financial (15)

| Class | Concept | Aliases | Variants | See |
|---|---|---|---|---|
| `.fg-report-head` | Report header with reporting period | ReportHeader, StatementHeader, PeriodHeader | — | — |
| `.fg-stat` | Report stat (label / value / sub) | Stat, StatBlock, FigureBlock | — | — |
| `.fg-metric-grid` | Grid layout for report stats | MetricGrid, StatGrid, KPIGrid | — | — |
| `.fg-ledger` | Financial ledger table (subtotals, totals) | Ledger, AccountsTable, LedgerTable | — | — |
| `.fg-chart` | Bar/line chart with legend (CSS) | Chart, BarChart, LineChart, Graph | `2` `3` `4` `5` `muted` | — |
| `.fg-sparkline` | Inline sparkline | Sparkline, MicroChart, TrendLine | — | — |
| `.fg-gauge` | Conic-ring gauge / progress dial | Gauge, Dial, RadialProgress, ProgressRing | — | — |
| `.fg-variance` | Variance indicator (vs target/prior) | Variance, BudgetVariance, Delta | — | — |
| `.fg-budget` | Budget vs actual bar | Budget, BudgetBar, BudgetVsActual | — | — |
| `.fg-period-compare` | Period-over-period comparison | PeriodCompare, PoP, ComparePeriods | — | — |
| `.fg-currency` | Currency amount formatting helper | CurrencyFormat, MoneyFormat, AmountFormat | — | direction-colored figure → .fg-money |
| `.fg-table-wrap` | Scroll wrapper with sticky header/column | StickyTable, ScrollTable, TableWrap | `sticky` | — |
| `.fg-export-btn` | Export / download action button | ExportButton, DownloadButton, ExportAction | — | — |
| `.fg-diverge` | Two-sided divergence bar around a zero axis | DivergenceBar, DivergingBar, PosNegBar | `pos` `neg` | — |
| `.fg-cohort` | Cohort retention heat grid | CohortGrid, RetentionHeatmap, HeatGrid, Heatmap | `head` `lab` `size` `v` `null` | — |

## Media (4)

| Class | Concept | Aliases | Variants | See |
|---|---|---|---|---|
| `.fg-waveform` | Audio waveform bar display | Waveform, AudioWave, WaveformBar | `played` | — |
| `.fg-scrubber` | Interactive playback scrubber (draggable handle) | Scrubber, Seekbar, PlaybackSlider, ProgressSlider, Slider | — | — |
| `.fg-transport` | Media transport / playback control bar | Transport, PlaybackControls, MediaControls, PlayerBar | — | composes `.fg-icon-btn`, `.fg-scrubber` |
| `.fg-transcript` | Timestamped transcript / caption cues | Transcript, Captions, CueList, Subtitles | `current` | — |

## Folded sub-parts (not standalone roles)

These `.fg-*` class roots exist in the CSS but belong to a parent role; they are listed in `catalog.json → ignore` and excluded from coverage.

| Class root | Belongs to |
|---|---|
| `.fg-radioset` | radio group container (.fg-radio) |
| `.fg-fact` | item of .fg-facts |
| `.fg-step` | step of .fg-stepper |
| `.fg-pagehead` | header of .fg-page |
| `.fg-toast-stack` | stack container of .fg-toast |
| `.fg-status-set` | lane-set container of .fg-status-lane |
| `.fg-trow` | row of .fg-tasklist |
| `.fg-row` | table row selection modifier (.fg-table) |
| `.fg-fsel` | filter-select part of .fg-toolbar |
| `.fg-lane` | color utility for .fg-td-lane / .fg-status-lane |
| `.fg-tasklist-md` | GFM checkbox list inside .fg-prose |
| `.fg-footnotes` | footnotes block inside .fg-prose |
| `.fg-anchor` | heading anchor inside .fg-prose |
| `.fg-tok-k` | syntax token inside .fg-codeblock |
| `.fg-tok-s` | syntax token inside .fg-codeblock |
| `.fg-tok-f` | syntax token inside .fg-codeblock |
| `.fg-tok-n` | syntax token inside .fg-codeblock |
| `.fg-tok-c` | syntax token inside .fg-codeblock |
