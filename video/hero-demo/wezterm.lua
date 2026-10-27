-- WezTerm look for recording scene 2 by hand: the same picture as the VHS scenes
-- (tapes/settings.tape): a 1920x1080 inner window, Geist Mono at 26 px, line height 1.35,
-- 64 px padding, the dark ember theme. Start it with wezterm.sh, which also copies the fonts.
local wezterm = require 'wezterm'
local mux = wezterm.mux
local config = wezterm.config_builder()

-- Pixel-exact sizes regardless of Windows display scaling: 96 dpi makes 19.5 pt = 26 px.
config.dpi = 96
config.font_dirs = { wezterm.config_dir .. '/fonts' }
config.font = wezterm.font_with_fallback { 'Geist Mono', 'Noto Sans Symbols 2' }
config.font_size = 19.5
config.line_height = 1.35
config.window_padding = { left = 64, right = 64, top = 64, bottom = 64 }

config.colors = {
  background = '#140e0b',
  foreground = '#f4ede7',
  cursor_bg = '#e9590c',
  cursor_fg = '#140e0b',
  cursor_border = '#e9590c',
  selection_bg = '#3a2a20',
  selection_fg = '#f4ede7',
  ansi = { '#1a120e', '#ff6b5a', '#9bc78f', '#f3a072', '#9bb4d9', '#d9a0c8', '#8fc9c2', '#d8cec6' },
  brights = { '#5f524a', '#ff8a7a', '#b3d6a9', '#f7c4a6', '#b8cbe6', '#e6bcd9', '#aedbd5', '#f4ede7' },
}
config.default_cursor_style = 'SteadyBlock'

-- Nothing but the terminal in the window: no tabs, no title bar, no scroll bar, no bell flash.
config.enable_tab_bar = false
config.window_decorations = 'RESIZE'
config.enable_scroll_bar = false
config.audible_bell = 'Disabled'
config.visual_bell = { fade_in_duration_ms = 0, fade_out_duration_ms = 0 }
config.window_close_confirmation = 'NeverPrompt'
-- Keep the window open if the shell exits, so an error stays readable.
config.exit_behavior = 'Hold'

-- Open straight into WSL; wezterm.sh passes the command (the demo shell).
config.default_domain = 'WSL:Ubuntu'

wezterm.on('gui-startup', function(cmd)
  local _, _, window = mux.spawn_window(cmd or {})
  window:gui_window():set_inner_size(1920, 1080)
end)

return config
