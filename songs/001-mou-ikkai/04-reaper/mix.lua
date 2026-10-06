local here = ({reaper.get_action_context()})[2]:match("^(.*)/[^/]*$")
package.path = here .. "/?.lua;" .. package.path
local surge = require("surge")

local function number_of(text)
  if text:find("inf") then
    return text:find("^-") and -math.huge or math.huge
  end
  return tonumber(text:match("[-+]?%d+%.?%d*"))
end

local function set(track, fx, name, target)
  for p = 0, reaper.TrackFX_GetNumParams(track, fx) - 1 do
    local _, param = reaper.TrackFX_GetParamName(track, fx, p, "")
    if param == name then
      local function at(v)
        local _, text = reaper.TrackFX_FormatParamValueNormalized(track, fx, p, v, "")
        return number_of(text)
      end
      local lo, hi = 0.0, 1.0
      local rising = at(hi) >= at(lo)
      for _ = 1, 40 do
        local mid = (lo + hi) / 2
        if (at(mid) < target) == rising then lo = mid else hi = mid end
      end
      reaper.TrackFX_SetParamNormalized(track, fx, p, (lo + hi) / 2)
      return
    end
  end
  error("no parameter " .. name)
end

local function add(track, plugin, params)
  local fx = reaper.TrackFX_AddByName(track, plugin, false, -1)
  if fx < 0 then error("no plugin " .. plugin) end
  for _, kv in ipairs(params) do set(track, fx, kv[1], kv[2]) end
  return fx
end

local function track_named(name)
  for i = 0, reaper.CountTracks(0) - 1 do
    local track = reaper.GetTrack(0, i)
    local _, n = reaper.GetTrackName(track)
    if n == name then return track end
  end
  error("no track " .. name)
end

local function read_table(path)
  local values = {}
  local file = io.open(path)
  if not file then return values end
  for line in file:lines() do
    local key, value = line:match("^([^\t]+)\t([^\t]+)$")
    if key then values[key] = value end
  end
  file:close()
  return values
end

local patches = {
  Bass = "patches_factory/Basses/Bass 1",
  Pad = "patches_factory/Pads/Super",
  Saw = "patches_factory/Polysynths/Hugeness",
  Arp = "patches_factory/Plucks/Sync Pluck",
  Piano = "patches_factory/Keys/DX EP",
  Lead = "patches_factory/Leads/Classic Lead 2",
}

local high_pass = {Pad = 120, Saw = 180, Arp = 250, Piano = 250, Lead = 200}

local widen = {Pad = 4, Saw = 4, Arp = 4}

local function build(gains)
  local vocal = track_named("Vocal")
  add(vocal, "ReaEQ (Cockos)", {
    {"Freq-Low Shelf", 250}, {"Gain-Low Shelf", -2},
    {"Freq-Band 3", 3200}, {"Gain-Band 3", 2.5},
    {"Freq-High Shelf 4", 9000}, {"Gain-High Shelf 4", 4},
    {"Freq-High Pass 5", 110},
  })
  add(vocal, "ReaComp (Cockos)", {{"Threshold", -22}, {"Ratio", 4}, {"Attack", 3}, {"Release", 60}})
  add(vocal, "ReaVerbate (Cockos)", {{"Wet", -12}, {"Room size", 55}, {"Width", 1}})

  local drums = track_named("Drums")
  local kit = {
    {36, "kick", 0}, {37, "stick", -10}, {38, "snare", -3}, {39, "clap", -6}, {42, "hat-closed", 0},
    {45, "tom-low", -6}, {46, "hat-open", -2}, {47, "tom-mid", -6}, {49, "crash", -7}, {50, "tom-high", -6},
  }
  for _, piece in ipairs(kit) do
    local fx = reaper.TrackFX_AddByName(drums, "ReaSamplOmatic5000 (Cockos)", false, -1)
    reaper.TrackFX_SetNamedConfigParm(drums, fx, "FILE0", here .. "/samples/" .. piece[2] .. ".wav")
    reaper.TrackFX_SetNamedConfigParm(drums, fx, "DONE", "")
    reaper.TrackFX_SetNamedConfigParm(drums, fx, "MODE", "1")
    set(drums, fx, "Note range start", piece[1])
    set(drums, fx, "Note range end", piece[1])
    set(drums, fx, "Volume", piece[3])
  end
  add(drums, "ReaComp (Cockos)", {{"Threshold", -12}, {"Ratio", 3}, {"Attack", 10}, {"Release", 60}})
  add(drums, "JS:sstillwell/eventhorizon2", {{"Threshold", -6}, {"Ceiling", -6}})

  for name, patch in pairs(patches) do
    local track = track_named(name)
    surge.load(track, patch)
    if high_pass[name] then add(track, "ReaEQ (Cockos)", {{"Freq-High Pass 5", high_pass[name]}}) end
    if widen[name] then add(track, "JS:sstillwell/stereowidth", {{"Width Boost (dB)", widen[name]}}) end
  end
  add(track_named("Bass"), "ReaEQ (Cockos)", {{"Freq-Band 2", 130}, {"Gain-Band 2", 3}, {"Freq-High Pass 5", 35}})
  add(track_named("Bass"), "ReaComp (Cockos)", {{"Threshold", -15}, {"Ratio", 4}, {"Attack", 5}, {"Release", 100}})

  for i = 0, reaper.CountTracks(0) - 1 do
    local track = reaper.GetTrack(0, i)
    local _, name = reaper.GetTrackName(track)
    reaper.SetMediaTrackInfo_Value(track, "D_VOL", 10 ^ ((tonumber(gains[name]) or 0) / 20))
  end
end

local function master_chain(settings)
  local master = reaper.GetMasterTrack(0)
  add(master, "ReaEQ (Cockos)", {
    {"Freq-Low Shelf", 60}, {"Gain-Low Shelf", tonumber(settings.low_shelf or 0)},
    {"Freq-Band 2", 700}, {"Gain-Band 2", -6}, {"BW-Band 2", 2.5},
    {"Freq-Band 3", 3500}, {"Gain-Band 3", 8}, {"BW-Band 3", 2.5},
    {"Freq-High Shelf 4", 8000}, {"Gain-High Shelf 4", tonumber(settings.high_shelf or 0)},
    {"Freq-High Pass 5", 25},
  })
  add(master, "ReaComp (Cockos)", {{"Threshold", -12}, {"Ratio", 2}, {"Attack", 20}, {"Release", 150}})
  add(master, "JS:sstillwell/eventhorizon2", {{"Threshold", tonumber(settings.threshold)}, {"Ceiling", -1.5}})
  local limiter = add(master, "ReaLimit (Cockos)", {{"Threshold", -1.6}, {"Ceiling", -1.6}, {"Release", 20}})
  reaper.TrackFX_SetNamedConfigParm(master, limiter, "instance_oversample_shift", "3")
end

local function render(pattern, settings_flag)
  reaper.GetSetProjectInfo_String(0, "RENDER_FILE", here .. "/render", true)
  reaper.GetSetProjectInfo_String(0, "RENDER_PATTERN", pattern, true)
  reaper.GetSetProjectInfo_String(0, "RENDER_FORMAT", "ZXZhdxgAAA==", true)
  reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", 1, true)
  reaper.GetSetProjectInfo(0, "RENDER_SRATE", 48000, true)
  reaper.GetSetProjectInfo(0, "RENDER_CHANNELS", 2, true)
  reaper.GetSetProjectInfo(0, "RENDER_SETTINGS", settings_flag, true)
  reaper.Main_OnCommand(42230, 0)
end

local function mix()
  local settings = read_table(here .. "/render/pass.tsv")
  reaper.Main_openProject("noprompt:" .. here .. "/mou-ikkai.rpp")
  if settings.pass == "stems" then
    build({})
    reaper.Main_OnCommand(40296, 0)
    render("stems/$track", 3)
    return
  end
  build(read_table(here .. "/render/gains.tsv"))
  master_chain(settings)
  reaper.Main_SaveProjectEx(0, here .. "/mou-ikkai-mix.rpp", 0)
  render("mou-ikkai-mix", 0)
end

local ok, err = pcall(mix)
local log = io.open(here .. "/render/mix-done.txt", "w")
log:write(ok and "ok" or tostring(err), "\n")
log:close()
