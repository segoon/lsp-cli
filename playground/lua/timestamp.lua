local function normalize_timestamp(timestamp)
  return math.floor(timestamp)
end

local function format_timestamp(timestamp)
  return tostring(normalize_timestamp(timestamp))
end

return { format_timestamp = format_timestamp }
