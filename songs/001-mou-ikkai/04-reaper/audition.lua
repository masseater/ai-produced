local here = ({reaper.get_action_context()})[2]:match("^(.*)/[^/]*$")
package.path = here .. "/?.lua;" .. package.path
local surge = require("surge")

local function track_named(name)
  for i = 0, reaper.CountTracks(0) - 1 do
    local track = reaper.GetTrack(0, i)
    local _, n = reaper.GetTrackName(track)
    if n == name then return track end
  end
  error("no track " .. name)
end

local function section(name)
  local i = 0
  while true do
    local found, region, pos, _, label = reaper.EnumProjectMarkers(i)
    if found == 0 then error("no marker " .. name) end
    if not region and label == name then
      local _, _, next_pos = reaper.EnumProjectMarkers(i + 1)
      return pos, next_pos
    end
    i = i + 1
  end
end

local function audition()
  local out = here .. "/render/audition"
  reaper.RecursiveCreateDirectory(out, 0)
  for line in io.lines(here .. "/audition.tsv") do
    local name, section_name, patch = line:match("^([^\t]+)\t([^\t]+)\t([^\t]+)$")
    if name and name ~= "track" then
      reaper.Main_openProject("noprompt:" .. here .. "/mou-ikkai.rpp")
      local track = track_named(name)
      surge.load(track, patch)
      reaper.SetMediaTrackInfo_Value(track, "I_SOLO", 1)
      local from, to = section(section_name)
      reaper.GetSetProjectInfo_String(0, "RENDER_FILE", out, true)
      reaper.GetSetProjectInfo_String(0, "RENDER_PATTERN", name .. "__" .. patch:gsub("^.*/", ""), true)
      reaper.GetSetProjectInfo_String(0, "RENDER_FORMAT", "ZXZhdxgAAA==", true)
      reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", 0, true)
      reaper.GetSetProjectInfo(0, "RENDER_STARTPOS", from, true)
      reaper.GetSetProjectInfo(0, "RENDER_ENDPOS", to, true)
      reaper.GetSetProjectInfo(0, "RENDER_SRATE", 48000, true)
      reaper.GetSetProjectInfo(0, "RENDER_CHANNELS", 2, true)
      reaper.Main_OnCommand(42230, 0)
    end
  end
end

local ok, err = pcall(audition)
local log = io.open(here .. "/render/audition-done.txt", "w")
log:write(ok and "ok" or tostring(err), "\n")
log:close()
