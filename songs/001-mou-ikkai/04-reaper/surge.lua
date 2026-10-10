local surge = {}

local function base64(data)
  local path = os.tmpname()
  local file = assert(io.open(path, "wb"))
  file:write(data)
  file:close()
  local pipe = assert(io.popen("base64 < '" .. path .. "'"))
  local encoded = pipe:read("a"):gsub("%s", "")
  pipe:close()
  os.remove(path)
  return encoded
end

surge.patches = os.getenv("HOME") .. "/Library/Application Support/Surge XT/"

function surge.load(track, patch)
  local file = assert(io.open(surge.patches .. patch .. ".fxp", "rb"))
  local fxp = file:read("a")
  file:close()
  local state = fxp:sub(61)
  local fx = reaper.TrackFX_AddByName(track, "VST3:Surge XT (Surge Synth Team)", false, -1)
  if fx < 0 then error("no Surge XT") end
  local chunk = base64(string.pack("<I4I4", #state, 1) .. state .. string.rep("\0", 8))
  if not reaper.TrackFX_SetNamedConfigParm(track, fx, "vst_chunk", chunk) then error("cannot load " .. patch) end
  return fx
end

return surge
