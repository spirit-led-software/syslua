local prio = require('syslua.priority')
local interpolate = require('syslua.interpolation')

---@class syslua.user
local M = {}

-- ============================================================================
-- Type Definitions
-- ============================================================================

---@class syslua.user.Options
---@field description? syslua.Option<string> User description/comment
---@field homeDir syslua.Option<string> Home directory path (required)
---@field config syslua.Option<string> Path to user's syslua config (required)
---@field shell? syslua.Option<BuildRef> Login shell package
---@field initialPassword? syslua.Option<string> Initial password (plaintext, set on creation only)
---@field groups? syslua.MergeableOption<string[]> Groups to add user to (must exist)
---@field preserveHomeOnRemove? syslua.Option<boolean> Keep home directory when user is removed (default: false)

---@alias syslua.user.UserMap table<string, syslua.user.Options>

-- ============================================================================
-- Constants
-- ============================================================================

local BIND_ID_PREFIX = '__syslua_user_'

-- ============================================================================
-- Default Options
-- ============================================================================

---@diagnostic disable-next-line: missing-fields
M.defaults = {
  description = '',
  homeDir = nil,
  config = nil,
  shell = nil,
  initialPassword = nil,
  groups = prio.mergeable({ default = {} }),
  preserveHomeOnRemove = false,
}

-- ============================================================================
-- Platform-Specific Commands
-- ============================================================================

---Get the default shell for the current platform
---@return string
local function get_default_shell()
  if sys.os == 'windows' then
    return 'cmd.exe'
  elseif sys.os == 'darwin' then
    return '/bin/zsh'
  else
    return '/bin/bash'
  end
end

---Get shell path from BuildRef or use default
---@param shell? BuildRef
---@return string
local function get_shell_path(shell)
  if shell and shell.outputs and shell.outputs.bin then
    return shell.outputs.bin
  end
  return get_default_shell()
end

---Build Linux user creation command
---@param name string
---@param opts syslua.user.Options
---@return string bin, string[] args
local function linux_create_user_cmd(name, opts)
  local args = { '-m', '-d', opts.homeDir }

  if opts.description and opts.description ~= '' then
    table.insert(args, '-c')
    table.insert(args, opts.description)
  end

  local shell = get_shell_path(opts.shell)
  table.insert(args, '-s')
  table.insert(args, shell)

  if opts.groups and #opts.groups > 0 then
    table.insert(args, '-G')
    table.insert(args, table.concat(opts.groups, ','))
  end

  table.insert(args, name)

  return '/usr/sbin/useradd', args
end

---Build macOS user creation command
---@param name string
---@param opts syslua.user.Options
---@return string bin, string[] args
local function darwin_create_user_cmd(name, opts)
  local args = { '-addUser', name }

  if opts.description and opts.description ~= '' then
    table.insert(args, '-fullName')
    table.insert(args, opts.description)
  end

  table.insert(args, '-home')
  table.insert(args, opts.homeDir)

  local shell = get_shell_path(opts.shell)
  table.insert(args, '-shell')
  table.insert(args, shell)

  if opts.initialPassword then
    table.insert(args, '-password')
    table.insert(args, opts.initialPassword)
  end

  return '/usr/sbin/sysadminctl', args
end

---Add user to group on macOS
---@param username string
---@param group string
---@return string bin, string[] args
local function darwin_add_to_group_cmd(username, group)
  return '/usr/sbin/dseditgroup', { '-o', 'edit', '-a', username, '-t', 'user', group }
end

---Build Linux user update command (for existing users)
---@param name string
---@param opts syslua.user.Options
---@return string bin, string[] args
local function linux_update_user_cmd(name, opts)
  local args = {}

  if opts.description and opts.description ~= '' then
    table.insert(args, '-c')
    table.insert(args, opts.description)
  end

  local shell = get_shell_path(opts.shell)
  table.insert(args, '-s')
  table.insert(args, shell)

  if opts.groups and #opts.groups > 0 then
    table.insert(args, '-G')
    table.insert(args, table.concat(opts.groups, ','))
  end

  table.insert(args, name)

  return '/usr/sbin/usermod', args
end

---Build macOS user update commands (returns shell script)
---@param name string
---@param opts syslua.user.Options
---@return string
local function darwin_update_user_script(name, opts)
  local cmds = {}

  if opts.description and opts.description ~= '' then
    table.insert(
      cmds,
      interpolate(
        'dscl . -create /Users/{{name}} RealName "{{description}}"',
        { name = name, description = opts.description }
      )
    )
  end

  local shell = get_shell_path(opts.shell)
  table.insert(
    cmds,
    interpolate('dscl . -create /Users/{{name}} UserShell "{{shell}}"', { name = name, shell = shell })
  )

  return table.concat(cmds, ' && ')
end

---Build Windows user update PowerShell script (for existing users)
---@param name string
---@param opts syslua.user.Options
---@return string
local function windows_update_user_script(name, opts)
  local description = opts.description or ''
  return interpolate(
    'Set-LocalUser -Name "{{name}}" -Description "{{description}}"',
    { name = name, description = description }
  )
end

---Build Windows user creation PowerShell script
---@param name string
---@param opts syslua.user.Options
---@return string
local function windows_create_user_script(name, opts)
  local lines = {}
  local description = opts.description or ''

  -- Create user
  if opts.initialPassword then
    table.insert(
      lines,
      interpolate(
        '$securePass = ConvertTo-SecureString "{{password}}" -AsPlainText -Force',
        { password = opts.initialPassword }
      )
    )
    table.insert(
      lines,
      interpolate(
        'New-LocalUser -Name "{{name}}" -Description "{{description}}" -Password $securePass',
        { name = name, description = description }
      )
    )
  else
    table.insert(
      lines,
      interpolate(
        'New-LocalUser -Name "{{name}}" -Description "{{description}}" -NoPassword',
        { name = name, description = description }
      )
    )
  end

  -- Create home directory
  table.insert(
    lines,
    interpolate('New-Item -ItemType Directory -Path "{{homeDir}}" -Force | Out-Null', { homeDir = opts.homeDir })
  )

  -- Add to groups
  if opts.groups then
    for _, group in ipairs(opts.groups) do
      table.insert(
        lines,
        interpolate(
          'Add-LocalGroupMember -Group "{{group}}" -Member "{{name}}" -ErrorAction Stop',
          { group = group, name = name }
        )
      )
    end
  end

  return table.concat(lines, '; ')
end

---Build Linux user deletion command
---@param name string
---@param preserve_home boolean
---@return string bin, string[] args
local function linux_delete_user_cmd(name, preserve_home)
  local args = {}
  if not preserve_home then
    table.insert(args, '-r')
  end
  table.insert(args, name)
  return '/usr/sbin/userdel', args
end

---Build macOS user deletion command
---@param name string
---@param preserve_home boolean
---@return string bin, string[] args
local function darwin_delete_user_cmd(name, preserve_home)
  local args = { '-deleteUser', name }
  if preserve_home then
    table.insert(args, '-keepHome')
  else
    table.insert(args, '-secure')
  end
  return '/usr/sbin/sysadminctl', args
end

---Build Windows user deletion PowerShell script
---@param name string
---@param home_dir string
---@param preserve_home boolean
---@return string
local function windows_delete_user_script(name, home_dir, preserve_home)
  local lines = {
    interpolate('Remove-LocalUser -Name "{{name}}"', { name = name }),
  }
  if not preserve_home then
    table.insert(
      lines,
      interpolate('Remove-Item -Recurse -Force "{{home_dir}}" -ErrorAction SilentlyContinue', { home_dir = home_dir })
    )
  end
  return table.concat(lines, '; ')
end

-- ============================================================================
-- User Config Execution
-- ============================================================================

---Get the store path for a user
---@param home_dir string
---@return string
local function get_user_store(home_dir)
  return home_dir .. '/.syslua/store'
end

---Get the parent store path (system store)
---@return string
local function get_parent_store()
  -- Use the current store as parent for user subprocesses
  local store = sys.getenv('SYSLUA_STORE')
  if store and store ~= '' then
    return store
  end
  -- Fallback to default system store
  if sys.os == 'windows' then
    local drive = sys.getenv('SYSTEMDRIVE') or 'C:'
    return drive .. '\\syslua\\store'
  else
    return '/syslua/store'
  end
end

---Resolve config path (file or directory with init.lua)
---@param config_path string
---@return string
local function resolve_config_path(config_path)
  -- If it's a directory, append init.lua
  -- The actual check happens at runtime in the bind
  if config_path:match('%.lua$') then
    return config_path
  else
    return config_path .. '/init.lua'
  end
end

---Build Unix command to run sys apply as user
---@param username string
---@param home_dir string
---@param config_path string
---@return string bin, string[] args
local function unix_run_as_user_cmd(username, home_dir, config_path)
  local user_store = get_user_store(home_dir)
  local parent_store = get_parent_store()
  local resolved_config = resolve_config_path(config_path)

  local cmd = interpolate(
    'SYSLUA_STORE={{user_store}} SYSLUA_PARENT_STORE={{parent_store}} sys apply {{config}}',
    { user_store = user_store, parent_store = parent_store, config = resolved_config }
  )

  return '/bin/su', { '-', username, '-c', cmd }
end

---Build Unix command to run sys destroy as user
---@param username string
---@param home_dir string
---@return string bin, string[] args
local function unix_destroy_as_user_cmd(username, home_dir)
  local user_store = get_user_store(home_dir)
  local parent_store = get_parent_store()

  local cmd = interpolate(
    'SYSLUA_STORE={{user_store}} SYSLUA_PARENT_STORE={{parent_store}} sys destroy',
    { user_store = user_store, parent_store = parent_store }
  )

  return '/bin/su', { '-', username, '-c', cmd }
end

---Build Windows command to run sys apply as user (via scheduled task)
---@param username string
---@param home_dir string
---@param config_path string
---@return string
local function windows_run_as_user_script(username, home_dir, config_path)
  local user_store = get_user_store(home_dir):gsub('/', '\\')
  local parent_store = get_parent_store():gsub('/', '\\')
  local resolved_config = resolve_config_path(config_path):gsub('/', '\\')

  return interpolate(
    [[
$env:SYSLUA_STORE = "{{user_store}}"
$env:SYSLUA_PARENT_STORE = "{{parent_store}}"
$taskName = "SysluaApply_{{username}}"
$action = New-ScheduledTaskAction -Execute "sys" -Argument "apply {{config}}"
$principal = New-ScheduledTaskPrincipal -UserId "{{username}}" -LogonType Interactive
try {
  Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Force | Out-Null
  Start-ScheduledTask -TaskName $taskName
  $timeout = 300
  $elapsed = 0
  while (($task = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) -and $task.State -eq 'Running' -and $elapsed -lt $timeout) {
    Start-Sleep -Seconds 1
    $elapsed++
  }
  $info = Get-ScheduledTaskInfo -TaskName $taskName -ErrorAction SilentlyContinue
  if ($info -and $info.LastTaskResult -ne 0) {
    throw "sys apply failed with exit code $($info.LastTaskResult)"
  }
} finally {
  Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
}
]],
    { user_store = user_store, parent_store = parent_store, username = username, config = resolved_config }
  )
end

---Build Windows command to run sys destroy as user
---@param username string
---@param home_dir string
---@return string
local function windows_destroy_as_user_script(username, home_dir)
  local user_store = get_user_store(home_dir):gsub('/', '\\')
  local parent_store = get_parent_store():gsub('/', '\\')

  return interpolate(
    [[
$env:SYSLUA_STORE = "{{user_store}}"
$env:SYSLUA_PARENT_STORE = "{{parent_store}}"
$taskName = "SysluaDestroy_{{username}}"
$action = New-ScheduledTaskAction -Execute "sys" -Argument "destroy"
$principal = New-ScheduledTaskPrincipal -UserId "{{username}}" -LogonType Interactive
try {
  Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Force | Out-Null
  Start-ScheduledTask -TaskName $taskName
  $timeout = 300
  $elapsed = 0
  while (($task = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) -and $task.State -eq 'Running' -and $elapsed -lt $timeout) {
    Start-Sleep -Seconds 1
    $elapsed++
  }
  $info = Get-ScheduledTaskInfo -TaskName $taskName -ErrorAction SilentlyContinue
  if ($info -and $info.LastTaskResult -ne 0) {
    throw "sys destroy failed with exit code $($info.LastTaskResult)"
  }
} finally {
  Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
}
]],
    { user_store = user_store, parent_store = parent_store, username = username }
  )
end

-- ============================================================================
-- User Existence Checks
-- ============================================================================

---Check if user exists on Linux
---@param username string
---@return string
local function linux_user_exists_check(username)
  return interpolate('id "{{username}}" >/dev/null 2>&1', { username = username })
end

---Check if user exists on macOS
---@param username string
---@return string
local function darwin_user_exists_check(username)
  return interpolate('dscl . -read /Users/{{username}} >/dev/null 2>&1', { username = username })
end

---Check if user exists on Windows (PowerShell condition expression)
---@param username string
---@return string
local function windows_user_exists_check(username)
  return interpolate('(Get-LocalUser -Name "{{username}}" -ErrorAction SilentlyContinue)', { username = username })
end

-- ============================================================================
-- Validation Helpers
-- ============================================================================

---Check if a group exists on Linux
---@param group string
---@return boolean
local function linux_group_exists(group)
  local handle =
    io.popen(interpolate('getent group "{{group}}" >/dev/null 2>&1 && echo yes || echo no', { group = group }))
  if not handle then
    return false
  end
  local result = handle:read('*a'):gsub('%s+', '')
  handle:close()
  return result == 'yes'
end

---Check if a group exists on macOS
---@param group string
---@return boolean
local function darwin_group_exists(group)
  local handle =
    io.popen(interpolate('dscl . -read /Groups/{{group}} >/dev/null 2>&1 && echo yes || echo no', { group = group }))
  if not handle then
    return false
  end
  local result = handle:read('*a'):gsub('%s+', '')
  handle:close()
  return result == 'yes'
end

---Check if a group exists on Windows
---@param group string
---@return boolean
local function windows_group_exists(group)
  local handle = io.popen(
    interpolate(
      'powershell -NoProfile -Command "if (Get-LocalGroup -Name \'{{group}}\' -ErrorAction SilentlyContinue) { echo yes } else { echo no }"',
      { group = group }
    )
  )
  if not handle then
    return false
  end
  local result = handle:read('*a'):gsub('%s+', '')
  handle:close()
  return result == 'yes'
end

---Check if a group exists (cross-platform)
---@param group string
---@return boolean
local function group_exists(group)
  if sys.os == 'linux' then
    return linux_group_exists(group)
  elseif sys.os == 'darwin' then
    return darwin_group_exists(group)
  elseif sys.os == 'windows' then
    return windows_group_exists(group)
  end
  return false
end

---Check if a config path exists (file or directory with init.lua)
---@param config_path string
---@return boolean, string? -- exists, resolved_path
local function validate_config_path(config_path)
  -- Check if it's a file ending in .lua
  if config_path:match('%.lua$') then
    local f = io.open(config_path, 'r')
    if f then
      f:close()
      return true, config_path
    end
    return false, nil
  end

  -- Check if it's a directory with init.lua
  local init_path = config_path .. '/init.lua'
  local f = io.open(init_path, 'r')
  if f then
    f:close()
    return true, init_path
  end

  -- Check if the path itself is a file (without .lua extension)
  f = io.open(config_path, 'r')
  if f then
    f:close()
    return true, config_path
  end

  return false, nil
end

-- ============================================================================
-- Validation
-- ============================================================================

---@param name string
---@param opts syslua.user.Options
---@param groups string[]
local function validate_user_options(name, opts, groups)
  local home_dir = prio.unwrap(opts.homeDir)
  local config = prio.unwrap(opts.config)

  if not home_dir then
    error(interpolate("user '{{name}}': homeDir is required", { name = name }), 0)
  end
  if not config then
    error(interpolate("user '{{name}}': config is required", { name = name }), 0)
  end
  if not sys.is_elevated then
    error('syslua.user requires elevated privileges (root/Administrator)', 0)
  end

  -- Validate config path exists
  local config_exists = validate_config_path(config)
  if not config_exists then
    error(interpolate("user '{{name}}': config path does not exist: {{config}}", { name = name, config = config }), 0)
  end

  -- Validate all groups exist
  local missing_groups = {}
  for _, group in ipairs(groups) do
    if not group_exists(group) then
      table.insert(missing_groups, group)
    end
  end
  if #missing_groups > 0 then
    error(
      interpolate(
        "user '{{name}}': groups do not exist: {{groups}}",
        { name = name, groups = table.concat(missing_groups, ', ') }
      ),
      0
    )
  end
end

-- ============================================================================
-- Public API
-- ============================================================================

---Resolve groups from merged options (handles Mergeable type)
---@param groups_opt syslua.MergeableOption<string[]>|nil
---@return string[]
local function resolve_groups(groups_opt)
  if not groups_opt then
    return {}
  end
  -- If it's a mergeable, access will resolve it
  if prio.is_mergeable(groups_opt) then
    -- Access the merged value through the MergedTable mechanism
    -- For mergeables without separator, result is an array
    local result = {}
    for _, entry in ipairs(groups_opt.__entries or {}) do
      local val = entry.value
      if type(val) == 'table' then
        for _, v in ipairs(val) do
          table.insert(result, v)
        end
      else
        table.insert(result, val)
      end
    end
    return result
  end
  -- Otherwise unwrap and return
  local unwrapped = prio.unwrap(groups_opt)
  if type(unwrapped) == 'table' then
    return unwrapped
  end
  return {}
end

---Create a bind for a single user
---@param name string
---@param opts syslua.user.Options
---@param groups string[]
local function create_user_bind(name, opts, groups)
  local bind_id = BIND_ID_PREFIX .. name

  -- Unwrap all priority values for bind inputs
  local description = prio.unwrap(opts.description) or ''
  local home_dir = prio.unwrap(opts.homeDir)
  local config_path = prio.unwrap(opts.config)
  local shell = prio.unwrap(opts.shell)
  local initial_password = prio.unwrap(opts.initialPassword)
  local preserve_home = prio.unwrap(opts.preserveHomeOnRemove) or false

  sys.bind({
    id = bind_id,
    replace = true,
    inputs = {
      username = name,
      description = description,
      home_dir = home_dir,
      config_path = config_path,
      shell = shell,
      initial_password = initial_password,
      groups = groups,
      preserve_home = preserve_home,
      os = sys.os,
    },
    create = function(inputs, ctx)
      -- Step 1: Create or update the user account
      if inputs.os == 'linux' then
        local exists_check = linux_user_exists_check(inputs.username)
        ---@diagnostic disable-next-line: missing-fields
        local _, create_args = linux_create_user_cmd(inputs.username, {
          description = inputs.description,
          homeDir = inputs.home_dir,
          shell = inputs.shell,
          groups = inputs.groups,
        })
        local create_cmd = '/usr/sbin/useradd ' .. table.concat(create_args, ' ')

        ---@diagnostic disable-next-line: missing-fields
        local _, update_args = linux_update_user_cmd(inputs.username, {
          description = inputs.description,
          shell = inputs.shell,
          groups = inputs.groups,
        })
        local update_cmd = '/usr/sbin/usermod ' .. table.concat(update_args, ' ')

        -- Create if doesn't exist, update if exists
        ctx:exec({
          bin = '/bin/sh',
          args = {
            '-c',
            interpolate(
              'if ! {{exists_check}}; then {{create_cmd}}; else {{update_cmd}}; fi',
              { exists_check = exists_check, create_cmd = create_cmd, update_cmd = update_cmd }
            ),
          },
        })

        -- Set password separately on Linux
        if inputs.initial_password and inputs.initial_password ~= '' then
          ctx:exec({
            bin = '/bin/sh',
            args = {
              '-c',
              interpolate(
                'echo "{{username}}:{{password}}" | chpasswd',
                { username = inputs.username, password = inputs.initial_password }
              ),
            },
          })
        end
      elseif inputs.os == 'darwin' then
        local exists_check = darwin_user_exists_check(inputs.username)
        ---@diagnostic disable-next-line: missing-fields
        local _, create_args = darwin_create_user_cmd(inputs.username, {
          description = inputs.description,
          homeDir = inputs.home_dir,
          shell = inputs.shell,
          initialPassword = inputs.initial_password,
        })
        local create_cmd = '/usr/sbin/sysadminctl ' .. table.concat(create_args, ' ')

        ---@diagnostic disable-next-line: missing-fields
        local update_script = darwin_update_user_script(inputs.username, {
          description = inputs.description,
          shell = inputs.shell,
        })

        -- Create if doesn't exist, update if exists
        ctx:exec({
          bin = '/bin/sh',
          args = {
            '-c',
            interpolate(
              'if ! {{exists_check}}; then {{create_cmd}}; else {{update_script}}; fi',
              { exists_check = exists_check, create_cmd = create_cmd, update_script = update_script }
            ),
          },
        })

        -- Add to groups separately on macOS (idempotent - dseditgroup handles existing membership)
        for _, group in ipairs(inputs.groups) do
          local grp_bin, grp_args = darwin_add_to_group_cmd(inputs.username, group)
          ctx:exec({ bin = grp_bin, args = grp_args })
        end
      elseif inputs.os == 'windows' then
        local exists_check = windows_user_exists_check(inputs.username)
        ---@diagnostic disable-next-line: missing-fields
        local create_script = windows_create_user_script(inputs.username, {
          description = inputs.description,
          homeDir = inputs.home_dir,
          initialPassword = inputs.initial_password,
          groups = inputs.groups,
        })

        ---@diagnostic disable-next-line: missing-fields
        local update_script = windows_update_user_script(inputs.username, {
          description = inputs.description,
        })

        -- Create if doesn't exist, update if exists
        ctx:exec({
          bin = 'powershell.exe',
          args = {
            '-NoProfile',
            '-Command',
            interpolate(
              'if (-not {{exists_check}}) { {{create_script}} } else { {{update_script}} }',
              { exists_check = exists_check, create_script = create_script, update_script = update_script }
            ),
          },
        })

        -- Update group membership (idempotent - Add-LocalGroupMember handles existing membership with -ErrorAction SilentlyContinue)
        for _, group in ipairs(inputs.groups) do
          ctx:exec({
            bin = 'powershell.exe',
            args = {
              '-NoProfile',
              '-Command',
              interpolate(
                'Add-LocalGroupMember -Group "{{group}}" -Member "{{username}}" -ErrorAction SilentlyContinue',
                { group = group, username = inputs.username }
              ),
            },
          })
        end
      end

      -- Step 2: Apply user's syslua config
      if inputs.os == 'windows' then
        local script = windows_run_as_user_script(inputs.username, inputs.home_dir, inputs.config_path)
        ctx:exec({
          bin = 'powershell.exe',
          args = { '-NoProfile', '-Command', script },
        })
      else
        local bin, args = unix_run_as_user_cmd(inputs.username, inputs.home_dir, inputs.config_path)
        ctx:exec({ bin = bin, args = args })
      end

      return {
        username = inputs.username,
        home_dir = inputs.home_dir,
        preserve_home = inputs.preserve_home,
      }
    end,
    destroy = function(outputs, ctx)
      -- Only proceed if user exists (idempotency)
      if sys.os == 'linux' then
        local exists_check = linux_user_exists_check(outputs.username)

        -- Step 1: Destroy user's syslua config (only if user exists)
        local _, destroy_args = unix_destroy_as_user_cmd(outputs.username, outputs.home_dir)
        local destroy_cmd = '/bin/su ' .. table.concat(destroy_args, ' ')
        ctx:exec({
          bin = '/bin/sh',
          args = {
            '-c',
            interpolate(
              'if {{exists_check}}; then {{destroy_cmd}}; fi',
              { exists_check = exists_check, destroy_cmd = destroy_cmd }
            ),
          },
        })

        -- Step 2: Remove user account (only if user exists)
        local _, delete_args = linux_delete_user_cmd(outputs.username, outputs.preserve_home)
        local delete_cmd = '/usr/sbin/userdel ' .. table.concat(delete_args, ' ')
        ctx:exec({
          bin = '/bin/sh',
          args = {
            '-c',
            interpolate(
              'if {{exists_check}}; then {{delete_cmd}}; fi',
              { exists_check = exists_check, delete_cmd = delete_cmd }
            ),
          },
        })
      elseif sys.os == 'darwin' then
        local exists_check = darwin_user_exists_check(outputs.username)

        -- Step 1: Destroy user's syslua config (only if user exists)
        local _, destroy_args = unix_destroy_as_user_cmd(outputs.username, outputs.home_dir)
        local destroy_cmd = '/bin/su ' .. table.concat(destroy_args, ' ')
        ctx:exec({
          bin = '/bin/sh',
          args = {
            '-c',
            interpolate(
              'if {{exists_check}}; then {{destroy_cmd}}; fi',
              { exists_check = exists_check, destroy_cmd = destroy_cmd }
            ),
          },
        })

        -- Step 2: Remove user account (only if user exists)
        local _, delete_args = darwin_delete_user_cmd(outputs.username, outputs.preserve_home)
        local delete_cmd = '/usr/sbin/sysadminctl ' .. table.concat(delete_args, ' ')
        ctx:exec({
          bin = '/bin/sh',
          args = {
            '-c',
            interpolate(
              'if {{exists_check}}; then {{delete_cmd}}; fi',
              { exists_check = exists_check, delete_cmd = delete_cmd }
            ),
          },
        })
      elseif sys.os == 'windows' then
        local exists_check = windows_user_exists_check(outputs.username)

        -- Step 1: Destroy user's syslua config (only if user exists)
        local destroy_script = windows_destroy_as_user_script(outputs.username, outputs.home_dir)
        ctx:exec({
          bin = 'powershell.exe',
          args = {
            '-NoProfile',
            '-Command',
            interpolate(
              'if ({{exists_check}}) { {{destroy_script}} }',
              { exists_check = exists_check, destroy_script = destroy_script }
            ),
          },
        })

        -- Step 2: Remove user account (only if user exists)
        local delete_script = windows_delete_user_script(outputs.username, outputs.home_dir, outputs.preserve_home)
        ctx:exec({
          bin = 'powershell.exe',
          args = {
            '-NoProfile',
            '-Command',
            interpolate(
              'if ({{exists_check}}) { {{delete_script}} }',
              { exists_check = exists_check, delete_script = delete_script }
            ),
          },
        })
      end
    end,
  })
end

---Set up users according to the provided definitions
---@param users syslua.user.UserMap
function M.setup(users)
  if not users or next(users) == nil then
    error('syslua.user.setup: at least one user definition is required', 2)
  end

  for name, opts in pairs(users) do
    -- Merge user options with defaults
    local merged = prio.merge(M.defaults, opts)
    if not merged then
      error(interpolate("user '{{name}}': failed to merge options", { name = name }), 2)
    end

    -- Resolve groups early for validation and bind creation
    local groups = resolve_groups(merged.groups)

    validate_user_options(name, merged, groups)
    create_user_bind(name, merged, groups)
  end
end

return M
