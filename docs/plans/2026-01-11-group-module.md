# Group Module Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a cross-platform group management module (`syslua.group`) for declarative group CRUD operations.

**Architecture:** Bind-based lifecycle management following the user module pattern. Each group gets one bind with ID `__syslua_group_<name>`. Uses create/update/destroy lifecycle with platform-specific command builders for Linux (`groupadd`/`groupmod`/`groupdel`), macOS (`dscl`), and Windows (`*-LocalGroup` PowerShell cmdlets).

**Tech Stack:** Lua, syslua bind system, priority module for option merging

**Reference Files:**
- `lua/syslua/user.lua` - Pattern to follow (user module)
- `lua/syslua/priority.lua` - Option merging
- `lua/syslua/interpolation.lua` - String interpolation
- `crates/cli/tests/fixtures/bind_update.lua` - Update lifecycle example

---

## Task 1: Create Module Skeleton with Type Definitions

**Files:**
- Create: `lua/syslua/group.lua`

**Step 1: Create the module file with type definitions and constants**

```lua
local prio = require('syslua.priority')
local interpolate = require('syslua.interpolation')

---@class syslua.group
local M = {}

-- ============================================================================
-- Type Definitions
-- ============================================================================

---@class syslua.group.Options
---@field description? syslua.Option<string> Group description/comment
---@field gid? syslua.Option<number> Specific GID (optional, auto-assigned if nil)
---@field system? syslua.Option<boolean> Create as system group (low GID range)

---@alias syslua.group.GroupMap table<string, syslua.group.Options>

---@class syslua.group.Defaults
---@field description string
---@field gid nil
---@field system boolean

-- ============================================================================
-- Constants
-- ============================================================================

local BIND_ID_PREFIX = '__syslua_group_'

-- ============================================================================
-- Default Options
-- ============================================================================

---@type syslua.group.Defaults
M.defaults = {
  description = '',
  gid = nil,
  system = false,
}

return M
```

**Step 2: Verify module loads**

Run: `cd /Users/ianpascoe/code/syslua && cargo build -p syslua-cli`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add lua/syslua/group.lua
git commit -m "feat(group): add module skeleton with type definitions"
```

---

## Task 2: Add Platform-Specific Group Creation Commands

**Files:**
- Modify: `lua/syslua/group.lua`

**Step 1: Add Linux group creation command builder**

Add after the defaults section:

```lua
-- ============================================================================
-- Platform-Specific Commands: Creation
-- ============================================================================

---Build Linux group creation command
---@param name string
---@param opts {description?: string, gid?: number, system?: boolean}
---@return string bin, string[] args
local function linux_create_group_cmd(name, opts)
  local args = {}

  if opts.gid then
    table.insert(args, '-g')
    table.insert(args, tostring(opts.gid))
  end

  if opts.system then
    table.insert(args, '-r')
  end

  table.insert(args, name)
  return '/usr/sbin/groupadd', args
end
```

**Step 2: Add macOS group creation script builder**

```lua
---Build macOS group creation script (multiple dscl commands)
---@param name string
---@param opts {description?: string, gid?: number, system?: boolean}
---@return string
local function darwin_create_group_script(name, opts)
  local cmds = {
    interpolate('dscl . -create /Groups/{{name}}', { name = name }),
  }

  if opts.gid then
    table.insert(cmds, interpolate(
      'dscl . -create /Groups/{{name}} PrimaryGroupID {{gid}}',
      { name = name, gid = opts.gid }
    ))
  else
    -- Auto-assign GID: find max existing + 1
    local start_gid = opts.system and 100 or 1000
    table.insert(cmds, interpolate(
      'gid=$(dscl . -list /Groups PrimaryGroupID | awk "\\$2 >= {{start}} {print \\$2}" | sort -n | tail -1); dscl . -create /Groups/{{name}} PrimaryGroupID $((gid + 1))',
      { name = name, start = start_gid }
    ))
  end

  if opts.description and opts.description ~= '' then
    table.insert(cmds, interpolate(
      'dscl . -create /Groups/{{name}} RealName "{{desc}}"',
      { name = name, desc = opts.description }
    ))
  end

  return table.concat(cmds, ' && ')
end
```

**Step 3: Add Windows group creation script builder**

```lua
---Build Windows group creation PowerShell script
---@param name string
---@param opts {description?: string}
---@return string
local function windows_create_group_script(name, opts)
  local desc = opts.description or ''
  return interpolate(
    'New-LocalGroup -Name "{{name}}" -Description "{{desc}}"',
    { name = name, desc = desc }
  )
end
```

**Step 4: Verify build**

Run: `cargo build -p syslua-cli`
Expected: Build succeeds

**Step 5: Commit**

```bash
git add lua/syslua/group.lua
git commit -m "feat(group): add platform-specific creation commands"
```

---

## Task 3: Add Existence Checks and Deletion Commands

**Files:**
- Modify: `lua/syslua/group.lua`

**Step 1: Add existence check functions**

```lua
-- ============================================================================
-- Platform-Specific Commands: Existence Checks
-- ============================================================================

---Check if group exists on Linux
---@param name string
---@return string
local function linux_group_exists_check(name)
  return interpolate('getent group "{{name}}" >/dev/null 2>&1', { name = name })
end

---Check if group exists on macOS
---@param name string
---@return string
local function darwin_group_exists_check(name)
  return interpolate('dscl . -read /Groups/{{name}} >/dev/null 2>&1', { name = name })
end

---Check if group exists on Windows (PowerShell condition)
---@param name string
---@return string
local function windows_group_exists_check(name)
  return interpolate(
    '(Get-LocalGroup -Name "{{name}}" -ErrorAction SilentlyContinue)',
    { name = name }
  )
end
```

**Step 2: Add deletion command functions**

```lua
-- ============================================================================
-- Platform-Specific Commands: Deletion
-- ============================================================================

---Build Linux group deletion command
---@param name string
---@return string bin, string[] args
local function linux_delete_group_cmd(name)
  return '/usr/sbin/groupdel', { name }
end

---Build macOS group deletion command
---@param name string
---@return string bin, string[] args
local function darwin_delete_group_cmd(name)
  return '/usr/bin/dscl', { '.', '-delete', '/Groups/' .. name }
end

---Build Windows group deletion PowerShell script
---@param name string
---@return string
local function windows_delete_group_script(name)
  return interpolate('Remove-LocalGroup -Name "{{name}}"', { name = name })
end
```

**Step 3: Verify build**

Run: `cargo build -p syslua-cli`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add lua/syslua/group.lua
git commit -m "feat(group): add existence checks and deletion commands"
```

---

## Task 4: Add Update Commands

**Files:**
- Modify: `lua/syslua/group.lua`

**Step 1: Add update command functions**

```lua
-- ============================================================================
-- Platform-Specific Commands: Update
-- ============================================================================

---Build macOS group update script (description only)
---@param name string
---@param opts {description?: string}
---@return string
local function darwin_update_group_script(name, opts)
  if opts.description and opts.description ~= '' then
    return interpolate(
      'dscl . -create /Groups/{{name}} RealName "{{desc}}"',
      { name = name, desc = opts.description }
    )
  end
  return 'true' -- no-op
end

---Build Windows group update PowerShell script
---@param name string
---@param opts {description?: string}
---@return string
local function windows_update_group_script(name, opts)
  return interpolate(
    'Set-LocalGroup -Name "{{name}}" -Description "{{desc}}"',
    { name = name, desc = opts.description or '' }
  )
end
```

**Step 2: Verify build**

Run: `cargo build -p syslua-cli`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add lua/syslua/group.lua
git commit -m "feat(group): add platform-specific update commands"
```

---

## Task 5: Add Validation Helpers

**Files:**
- Modify: `lua/syslua/group.lua`

**Step 1: Add member listing functions**

```lua
-- ============================================================================
-- Validation Helpers
-- ============================================================================

---Get group members on Linux
---@param name string
---@return string[]
local function linux_get_group_members(name)
  local handle = io.popen(interpolate(
    'getent group "{{name}}" 2>/dev/null | cut -d: -f4',
    { name = name }
  ))
  if not handle then return {} end
  local members_str = handle:read('*a'):gsub('%s+$', '')
  handle:close()
  if members_str == '' then return {} end
  local members = {}
  for member in members_str:gmatch('[^,]+') do
    table.insert(members, member)
  end
  return members
end

---Get group members on macOS
---@param name string
---@return string[]
local function darwin_get_group_members(name)
  local handle = io.popen(interpolate(
    'dscl . -read /Groups/{{name}} GroupMembership 2>/dev/null | sed "s/GroupMembership://" | tr " " "\\n" | grep -v "^$"',
    { name = name }
  ))
  if not handle then return {} end
  local members = {}
  for line in handle:lines() do
    local member = line:gsub('%s+', '')
    if member ~= '' then
      table.insert(members, member)
    end
  end
  handle:close()
  return members
end

---Get group members on Windows
---@param name string
---@return string[]
local function windows_get_group_members(name)
  local handle = io.popen(interpolate(
    'powershell -NoProfile -Command "Get-LocalGroupMember -Group \\"{{name}}\\" -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }"',
    { name = name }
  ))
  if not handle then return {} end
  local members = {}
  for line in handle:lines() do
    local member = line:gsub('%s+$', '')
    if member ~= '' then
      table.insert(members, member)
    end
  end
  handle:close()
  return members
end

---Get group members (cross-platform)
---@param name string
---@return string[]
local function get_group_members(name)
  if sys.os == 'linux' then
    return linux_get_group_members(name)
  elseif sys.os == 'darwin' then
    return darwin_get_group_members(name)
  elseif sys.os == 'windows' then
    return windows_get_group_members(name)
  end
  return {}
end
```

**Step 2: Add GID and options validation**

```lua
---Validate GID range and warn if in system range
---@param name string
---@param gid number?
---@param is_system boolean
local function validate_gid(name, gid, is_system)
  if not gid then return end

  local system_max = 999
  if gid <= system_max and not is_system then
    io.stderr:write(string.format(
      "Warning: group '%s' has GID %d which is in system range (<%d). Consider using system=true or a higher GID.\n",
      name, gid, system_max + 1
    ))
  end
end

---Validate group options
---@param name string
---@param opts syslua.group.Options
local function validate_group_options(name, opts)
  if not sys.is_elevated then
    error('syslua.group requires elevated privileges (root/Administrator)', 0)
  end

  local gid = prio.unwrap(opts.gid)
  local is_system = prio.unwrap(opts.system) or false

  if gid then
    validate_gid(name, gid, is_system)
  end
end
```

**Step 3: Verify build**

Run: `cargo build -p syslua-cli`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add lua/syslua/group.lua
git commit -m "feat(group): add validation helpers for members and GID"
```

---

## Task 6: Add Bind Creation with Full Lifecycle

**Files:**
- Modify: `lua/syslua/group.lua`

**Step 1: Add the bind creation function**

```lua
-- ============================================================================
-- Bind Creation
-- ============================================================================

---Create a bind for a single group
---@param name string
---@param opts syslua.group.Options
local function create_group_bind(name, opts)
  local bind_id = BIND_ID_PREFIX .. name

  local description = prio.unwrap(opts.description) or ''
  local gid = prio.unwrap(opts.gid)
  local is_system = prio.unwrap(opts.system) or false

  sys.bind({
    id = bind_id,
    replace = true,
    inputs = {
      groupname = name,
      description = description,
      gid = gid,
      system = is_system,
      os = sys.os,
    },

    create = function(inputs, ctx)
      if inputs.os == 'linux' then
        local exists_check = linux_group_exists_check(inputs.groupname)
        local _, create_args = linux_create_group_cmd(inputs.groupname, {
          description = inputs.description,
          gid = inputs.gid,
          system = inputs.system,
        })
        local create_cmd = '/usr/sbin/groupadd ' .. table.concat(create_args, ' ')

        ctx:exec({
          bin = '/bin/sh',
          args = { '-c', interpolate(
            'if ! {{exists_check}}; then {{create_cmd}}; fi',
            { exists_check = exists_check, create_cmd = create_cmd }
          )},
        })

      elseif inputs.os == 'darwin' then
        local exists_check = darwin_group_exists_check(inputs.groupname)
        local create_script = darwin_create_group_script(inputs.groupname, {
          description = inputs.description,
          gid = inputs.gid,
          system = inputs.system,
        })

        ctx:exec({
          bin = '/bin/sh',
          args = { '-c', interpolate(
            'if ! {{exists_check}}; then {{create_script}}; fi',
            { exists_check = exists_check, create_script = create_script }
          )},
        })

      elseif inputs.os == 'windows' then
        local exists_check = windows_group_exists_check(inputs.groupname)
        local create_script = windows_create_group_script(inputs.groupname, {
          description = inputs.description,
        })

        ctx:exec({
          bin = 'powershell.exe',
          args = { '-NoProfile', '-Command', interpolate(
            'if (-not {{exists_check}}) { {{create_script}} }',
            { exists_check = exists_check, create_script = create_script }
          )},
        })
      end

      return { groupname = inputs.groupname }
    end,

    update = function(outputs, inputs, ctx)
      if inputs.os == 'linux' then
        io.stderr:write(string.format(
          "Warning: group '%s' description cannot be updated on Linux (groupmod limitation). Recreate group to change.\n",
          inputs.groupname
        ))
      elseif inputs.os == 'darwin' then
        local update_script = darwin_update_group_script(inputs.groupname, {
          description = inputs.description,
        })
        ctx:exec({
          bin = '/bin/sh',
          args = { '-c', update_script },
        })
      elseif inputs.os == 'windows' then
        local update_script = windows_update_group_script(inputs.groupname, {
          description = inputs.description,
        })
        ctx:exec({
          bin = 'powershell.exe',
          args = { '-NoProfile', '-Command', update_script },
        })
      end

      return { groupname = inputs.groupname }
    end,

    destroy = function(outputs, ctx)
      local members = get_group_members(outputs.groupname)
      if #members > 0 then
        io.stderr:write(string.format(
          "Warning: deleting group '%s' which has %d member(s): %s\n",
          outputs.groupname,
          #members,
          table.concat(members, ', ')
        ))
      end

      if sys.os == 'linux' then
        local exists_check = linux_group_exists_check(outputs.groupname)
        local bin, args = linux_delete_group_cmd(outputs.groupname)
        local delete_cmd = bin .. ' ' .. table.concat(args, ' ')

        ctx:exec({
          bin = '/bin/sh',
          args = { '-c', interpolate(
            'if {{exists_check}}; then {{delete_cmd}}; fi',
            { exists_check = exists_check, delete_cmd = delete_cmd }
          )},
        })

      elseif sys.os == 'darwin' then
        local exists_check = darwin_group_exists_check(outputs.groupname)
        local bin, args = darwin_delete_group_cmd(outputs.groupname)
        local delete_cmd = bin .. ' ' .. table.concat(args, ' ')

        ctx:exec({
          bin = '/bin/sh',
          args = { '-c', interpolate(
            'if {{exists_check}}; then {{delete_cmd}}; fi',
            { exists_check = exists_check, delete_cmd = delete_cmd }
          )},
        })

      elseif sys.os == 'windows' then
        local exists_check = windows_group_exists_check(outputs.groupname)
        local delete_script = windows_delete_group_script(outputs.groupname)

        ctx:exec({
          bin = 'powershell.exe',
          args = { '-NoProfile', '-Command', interpolate(
            'if ({{exists_check}}) { {{delete_script}} }',
            { exists_check = exists_check, delete_script = delete_script }
          )},
        })
      end
    end,
  })
end
```

**Step 2: Verify build**

Run: `cargo build -p syslua-cli`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add lua/syslua/group.lua
git commit -m "feat(group): add bind creation with create/update/destroy lifecycle"
```

---

## Task 7: Add Public API

**Files:**
- Modify: `lua/syslua/group.lua`

**Step 1: Add the setup function before `return M`**

```lua
-- ============================================================================
-- Public API
-- ============================================================================

---Set up groups according to the provided definitions
---@param groups syslua.group.GroupMap
function M.setup(groups)
  if not groups or next(groups) == nil then
    error('syslua.group.setup: at least one group definition is required', 2)
  end

  for name, opts in pairs(groups) do
    local merged = prio.merge(M.defaults, opts)
    if not merged then
      error(interpolate("group '{{name}}': failed to merge options", { name = name }), 2)
    end

    validate_group_options(name, merged)
    create_group_bind(name, merged)
  end
end

return M
```

**Step 2: Verify build**

Run: `cargo build -p syslua-cli`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add lua/syslua/group.lua
git commit -m "feat(group): add public setup() API"
```

---

## Task 8: Add Test Fixture

**Files:**
- Create: `crates/lib/tests/fixtures/group/group_basic.lua`

**Step 1: Create test fixture directory and file**

```lua
-- Test fixture for syslua.group module
-- Note: This requires elevated privileges and creates real groups
-- Only run in isolated test environments

local group = require('syslua.group')

group.setup({
  testgroup = {
    description = 'Test Group',
    gid = 2001,
  },
  sysgroup = {
    description = 'System Test Group',
    system = true,
  },
})
```

**Step 2: Verify fixture parses**

Run: `cargo build -p syslua-cli`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add crates/lib/tests/fixtures/group/group_basic.lua
git commit -m "test(group): add basic test fixture"
```

---

## Task 9: Update Module Exports

**Files:**
- Modify: `lua/syslua/init.lua`

**Step 1: Add group to the class definition**

In the `@class syslua` comment block, add:

```lua
---@field group syslua.group
```

So it becomes:

```lua
---@class syslua
---@field pkgs syslua.pkgs
---@field environment syslua.environment
---@field programs syslua.programs
---@field user syslua.user
---@field group syslua.group
---@field lib syslua.lib
---@field f fun(str: string, values?: table): string String interpolation (f-string style)
---@field interpolate fun(str: string, values?: table): string String interpolation
local M = {}
```

**Step 2: Verify build**

Run: `cargo build -p syslua-cli`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add lua/syslua/init.lua
git commit -m "feat(group): export group module from syslua"
```

---

## Task 10: Update Related Issue Documentation

**Files:**
- Modify: Issue syslua-e02 (user module group documentation)

**Step 1: Check issue status**

Run: `bd show syslua-e02`

**Step 2: Close the main group issue**

Run: `bd close syslua-8i8 --reason "Group module implemented with full CRUD lifecycle"`

**Step 3: Sync beads**

Run: `bd sync`

---

## Task 11: Final Verification

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass

**Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features`
Expected: No warnings

**Step 3: Format check**

Run: `cargo fmt --check`
Expected: No formatting issues

**Step 4: Final commit if any changes**

```bash
git status
# If changes exist:
git add -A
git commit -m "chore: final cleanup for group module"
```

**Step 5: Push**

```bash
git push
```

---

## Summary

| Task | Description | Est. Time |
|------|-------------|-----------|
| 1 | Module skeleton with types | 3 min |
| 2 | Creation commands | 5 min |
| 3 | Existence/deletion commands | 3 min |
| 4 | Update commands | 3 min |
| 5 | Validation helpers | 5 min |
| 6 | Bind with full lifecycle | 5 min |
| 7 | Public API | 2 min |
| 8 | Test fixture | 2 min |
| 9 | Module exports | 2 min |
| 10 | Issue updates | 2 min |
| 11 | Final verification | 5 min |
| **Total** | | **~37 min** |
