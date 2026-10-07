-- Pandoc Lua filter for the KOP PDF (scripts/build-kop.sh).
--
-- The chapters are Docusaurus pages first, so they carry two things pandoc
-- has to translate for a standalone document:
--   * links to other handbook pages, written relative (../start/install.md,
--     ./kernel.md#section), become absolute links to the public site;
--   * Docusaurus admonitions (:::info[Title] … :::), which the build script
--     rewrites into fenced divs, become a quoted block with a bold title.
-- It also sizes the one tall screenshot so it fits a page.

local site = os.getenv("LOSOS_HANDBOOK_URL") or "https://losos.dasmat.us"

local function is_relative(target)
  return not target:match("^%a[%w+.-]*:") and not target:match("^#") and not target:match("^/")
end

function Link(el)
  local t = el.target
  if is_relative(t) then
    -- ./kernel.md#x -> /project/kop/kernel#x ; ../start/install.md -> /start/install
    local path, frag = t:match("^([^#]*)#?(.*)$")
    path = path:gsub("%.md$", "")
    -- resolve against this chapter's directory, /project/kop/
    local base = "/project/kop/"
    while path:sub(1, 3) == "../" do
      path = path:sub(4)
      base = base:gsub("[^/]+/$", "")
    end
    path = path:gsub("^%./", "")
    if path == "index" or path == "" then path = "" end
    local url = site .. base .. path
    if frag ~= "" then url = url .. "#" .. frag end
    el.target = url
  end
  return el
end

function Div(el)
  if el.classes:includes("admonition") then
    local title = el.attributes["title"] or "Poznámka"
    local blocks = pandoc.List({ pandoc.Para({ pandoc.Strong(title) }) })
    blocks:extend(el.content)
    return pandoc.BlockQuote(blocks)
  end
  return el
end

-- Pages are portrait A4 with ~16 cm of text width; a 1100x1388 screenshot at
-- full width would be taller than the page.
local tall = { ["img/sprievodca-heslo-kluc.png"] = "62%" }

function Image(el)
  local src = el.src:gsub("^%./", "")
  if tall[src] then el.attributes["width"] = tall[src] end
  return el
end
