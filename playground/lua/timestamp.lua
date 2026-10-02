local function normalize_timestamp(timestamp)
  return math.floor(timestamp)
end

---@class TimestampFormatting
local TimestampFormatting = {}

---@param timestamp number
---@return string
function TimestampFormatting:format(timestamp)
  error("abstract formatter")
end

---@class DefaultTimestampFormatting : TimestampFormatting
local DefaultTimestampFormatting = setmetatable({}, { __index = TimestampFormatting })

---@param timestamp number
---@return string
function DefaultTimestampFormatting:format(timestamp)
  return tostring(normalize_timestamp(timestamp))
end

---@type TimestampFormatting
local formatter = setmetatable({}, { __index = DefaultTimestampFormatting })

local function render_timestamp(timestamp)
  return formatter:format(timestamp)
end

local function format_timestamp(timestamp)
  return render_timestamp(timestamp)
end

return { format_timestamp = format_timestamp }
