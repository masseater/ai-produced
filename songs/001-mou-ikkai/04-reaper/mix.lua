local here = ({reaper.get_action_context()})[2]:match("^(.*)/[^/]*$")

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

local function volume(track, db, pan)
  reaper.SetMediaTrackInfo_Value(track, "D_VOL", 10 ^ (db / 20))
  reaper.SetMediaTrackInfo_Value(track, "D_PAN", pan or 0)
end

local function synth(name, db, pan, params)
  local track = track_named(name)
  add(track, "ReaSynth (Cockos)", params)
  volume(track, db, pan)
  return track
end

local function mix()
  reaper.Main_openProject("noprompt:" .. here .. "/mou-ikkai.rpp")
  local vocal = track_named("Vocal")
  add(vocal, "ReaEQ (Cockos)", {{"Freq-High Pass 5", 120}, {"Freq-Band 3", 3000}, {"Gain-Band 3", 2}})
  add(vocal, "ReaComp (Cockos)", {{"Threshold", -20}, {"Ratio", 4}, {"Attack", 5}, {"Release", 80}})
  add(vocal, "ReaVerbate (Cockos)", {{"Wet", -18}, {"Room size", 40}})
  volume(vocal, 6)

  local drums = track_named("Drums")
  local kit = {
    {36, "kick", 0}, {37, "stick", -10}, {38, "snare", -3}, {39, "clap", -6}, {42, "hat-closed", -8},
    {45, "tom-low", -6}, {46, "hat-open", -10}, {47, "tom-mid", -6}, {49, "crash", -12}, {50, "tom-high", -6},
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
  volume(drums, 0)

  local bass = synth("Bass", -4, 0, {{"Saw mix", 0.6}, {"Square mix", 0.4}, {"Attack", 2}, {"Decay", 300}, {"Sustain", -6}, {"Release", 30}})
  add(bass, "ReaEQ (Cockos)", {{"Freq-High Shelf 4", 2000}, {"Gain-High Shelf 4", -9}, {"Freq-High Pass 5", 35}})
  add(bass, "ReaComp (Cockos)", {{"Threshold", -15}, {"Ratio", 4}, {"Attack", 5}, {"Release", 100}})
  local pad = synth("Pad", -16, 0, {{"Saw mix", 0.5}, {"Triangle mix", 0.5}, {"Attack", 250}, {"Release", 400}, {"Global detune", 8}})
  local saw = synth("Saw", -14, -0.7, {{"Saw mix", 1}, {"Attack", 3}, {"Decay", 400}, {"Sustain", -8}, {"Release", 80}, {"Global detune", 10}})
  synth("Arp", -18, 0.7, {{"Square mix", 0.8}, {"Pulse Width", 0.3}, {"Attack", 1}, {"Decay", 120}, {"Sustain", -30}, {"Release", 40}})
  synth("Piano", -14, -0.45, {{"Triangle mix", 0.6}, {"Extra sine mix", 0.3}, {"Attack", 1}, {"Decay", 700}, {"Sustain", -40}, {"Release", 200}})
  local lead = synth("Lead", -14, 0.2, {{"Saw mix", 0.7}, {"Square mix", 0.3}, {"Attack", 5}, {"Release", 100}, {"Global detune", 6}})
  add(lead, "ReaVerbate (Cockos)", {{"Wet", -14}, {"Room size", 60}})

  add(saw, "ReaVerbate (Cockos)", {{"Wet", -10}, {"Room size", 70}, {"Width", 1}})
  add(pad, "ReaVerbate (Cockos)", {{"Wet", -6}, {"Room size", 80}, {"Width", 1}})

  local master = reaper.GetMasterTrack(0)
  add(master, "ReaEQ (Cockos)", {{"Freq-Band 3", 3500}, {"Gain-Band 3", 3}, {"Freq-High Shelf 4", 4000}, {"Gain-High Shelf 4", 5}, {"Freq-High Pass 5", 25}})
  add(master, "ReaComp (Cockos)", {{"Threshold", -14}, {"Ratio", 2}, {"Attack", 20}, {"Release", 150}})
  add(master, "ReaLimit (Cockos)", {{"Threshold", -7}, {"Ceiling", -1.5}})

  reaper.GetSetProjectInfo_String(0, "RENDER_FILE", here .. "/render", true)
  reaper.GetSetProjectInfo_String(0, "RENDER_PATTERN", "mou-ikkai-mix", true)
  reaper.GetSetProjectInfo_String(0, "RENDER_FORMAT", "ZXZhdxgAAA==", true)
  reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", 1, true)
  reaper.GetSetProjectInfo(0, "RENDER_SRATE", 48000, true)
  reaper.GetSetProjectInfo(0, "RENDER_CHANNELS", 2, true)
  reaper.Main_SaveProjectEx(0, here .. "/mou-ikkai-mix.rpp", 0)
  reaper.Main_OnCommand(42230, 0)
end

local ok, err = pcall(mix)
local log = io.open(here .. "/render/mix-done.txt", "w")
log:write(ok and "ok" or tostring(err), "\n")
log:close()
