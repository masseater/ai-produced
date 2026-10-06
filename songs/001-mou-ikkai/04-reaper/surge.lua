local surge = {}

local alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"

local function base64(data)
  local out = {}
  for i = 1, #data, 3 do
    local a, b, c = data:byte(i, i + 2)
    local n = a * 65536 + (b or 0) * 256 + (c or 0)
    local chars = {}
    for k = 3, 0, -1 do
      local index = (n >> (6 * k)) & 63
      chars[#chars + 1] = alphabet:sub(index + 1, index + 1)
    end
    if not b then chars[3], chars[4] = "=", "=" elseif not c then chars[4] = "=" end
    out[#out + 1] = table.concat(chars)
  end
  return table.concat(out)
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
