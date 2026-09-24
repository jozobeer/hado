#!/usr/bin/env python3
"""lsp-minimal の門番。Neovim 0.12.5 の組み込み LSP client から `hado lsp` を
起こし、診断・hover・code action・lifecycle・位置の符号化・契約を測る
（docs/rounds/lsp-minimal.md の完了条件と判定）。

期待値はこの判定器が本文から独立に数える。実装の出力を読んで期待値にはしない。
コーパスのファイル名や行は埋め込まず、入力は作業ディレクトリに生成する。

使い方: lsp-check.py <nvim> <repo-root> <work-dir>
"""

import json
import os
import subprocess
import sys
from pathlib import Path

CHECKS = []


def check(name):
    def wrap(fn):
        CHECKS.append((name, fn))
        return fn
    return wrap


class Failure(AssertionError):
    pass


# --------------------------------------------------------------------------
# 位置の算術。符号化ごとの単位数を、対象の行を先頭から歩いて数える。
# --------------------------------------------------------------------------

def units(text, encoding):
    """`text`（行の先頭からの部分）が占める `encoding` の単位数。"""
    if encoding == 'utf-8':
        return len(text.encode('utf-8'))
    if encoding == 'utf-16':
        return len(text.encode('utf-16-le')) // 2
    if encoding == 'utf-32':
        return len(text)
    raise Failure('unknown encoding %r' % (encoding,))


def find_offset(text, needle, occurrence=0):
    """`needle` の `occurrence` 番目の出現のバイト位置。"""
    cursor = 0
    start = -1
    for _ in range(occurrence + 1):
        start = text.index(needle, cursor)
        cursor = start + len(needle)
    return start


def line_char(text, offset, encoding):
    """バイト位置 `offset` の 0-based LSP (line, character)。"""
    line = text.count('\n', 0, offset)
    line_start = text.rfind('\n', 0, offset) + 1
    return line, units(text[line_start:offset], encoding)


def range_of(text, needle, encoding, occurrence=0):
    """`needle` の 0-based range。"""
    offset = find_offset(text, needle, occurrence)
    line, character = line_char(text, offset, encoding)
    return {
        'start': {'line': line, 'character': character},
        'end': {'line': line, 'character': character + units(needle, encoding)},
    }


def point_range_of(text, needle, encoding, occurrence=0):
    """始点と同じ位置で終わる 0-based range（終端を持たない診断）。"""
    offset = find_offset(text, needle, occurrence)
    line, character = line_char(text, offset, encoding)
    point = {'line': line, 'character': character}
    return {'start': dict(point), 'end': dict(point)}


def offset_of_position(text, line, character, encoding):
    """0-based LSP (line, character) のバイト位置。スカラの境界でなければ失敗。"""
    offset = 0
    for _ in range(line):
        offset = text.index('\n', offset) + 1
    consumed = 0
    index = offset
    while index < len(text) and text[index] != '\n' and consumed < character:
        width = units(text[index], encoding)
        if consumed + width > character:
            raise Failure('position %d is not on a scalar boundary' % character)
        consumed += width
        index += 1
    if consumed != character:
        raise Failure('position %d is past the end of line %d' % (character, line))
    return index


def apply_edit(text, edit_range, new_text, encoding):
    start = offset_of_position(text, edit_range['start']['line'],
                               edit_range['start']['character'], encoding)
    end = offset_of_position(text, edit_range['end']['line'],
                             edit_range['end']['character'], encoding)
    return text[:start] + new_text + text[end:]


def diags_of(publishes, index=-1):
    if not publishes:
        return []
    return publishes[index]['diagnostics']


def ranges(diagnostics):
    return [diagnostic['range'] for diagnostic in diagnostics]


# --------------------------------------------------------------------------
# 道具
# --------------------------------------------------------------------------

PROBE_HEAD = r'''
local result = {}
local function write()
  local f = assert(io.open(vim.env.HADO_LSP_OUT, 'w'))
  f:write(vim.json.encode(result))
  f:close()
end
local ok, err = xpcall(function()
'''

PROBE_TAIL = r'''
end, function(e) return tostring(e) .. '\n' .. debug.traceback() end)
if not ok then result.error = err end
write()
vim.cmd('qa!')
'''

PRELUDE = r'''
local root = vim.env.HADO_LSP_ROOT
local work = vim.env.HADO_LSP_WORK

local function wait_for(fn, ms, what)
  if not vim.wait(ms or 30000, fn, 20) then
    error('timeout: ' .. (what or '?'), 0)
  end
end

-- README の設定を読んで cmd を取り出す。PATH の hado には依存しない。
local function readme_cmd()
  local file = io.open(root .. '/README.md', 'r')
  local body = file:read('*a')
  file:close()
  local line = body:match('cmd%s*=%s*{([^}]*)}')
  if not line then error('README has no cmd') end
  local path = line:match('"([^"]+)"')
  return vim.fn.resolve((path:gsub('%<checkout%>', root)))
end

vim.cmd('filetype on')
vim.filetype.add({ extension = { hd = 'hado', hdi = 'hado' } })
local cmd = readme_cmd()
result.cmd = cmd
-- HADO_LSP_FOLDERS に `:` 区切りで絶対パスを並べると、それを workspaceFolders
-- として渡す（§58.5 の複数 folder と接頭辞の対照）。
local folders = {}
if vim.env.HADO_LSP_FOLDERS and vim.env.HADO_LSP_FOLDERS ~= '' then
  for path in vim.gsplit(vim.env.HADO_LSP_FOLDERS, ':') do
    folders[#folders + 1] = { uri = vim.uri_from_fname(path) }
  end
end
result.folders = folders
local config = { cmd = { cmd }, filetypes = { 'hado' } }
-- 空の表を渡すと Neovim が落ちるので、指定があるときだけ足す。
if #folders > 0 then config.workspace_folders = folders end
vim.lsp.config('hado', config)
vim.lsp.enable('hado')

-- publishDiagnostics をそのまま記録する。既定の handler は vim.diagnostic に
-- 溜めるだけで「何がいつ届いたか」を残さないので、sequence を自分で持つ。
local publishes = {}
vim.lsp.handlers['textDocument/publishDiagnostics'] = function(err, res, ctx)
  publishes[#publishes + 1] = {
    uri = res.uri,
    version = res.version,
    diagnostics = res.diagnostics,
  }
end

local function publishes_for(uri)
  local list = {}
  for _, publish in ipairs(publishes) do
    if publish.uri == uri then
      list[#list + 1] = publish
    end
  end
  return list
end

-- README の cmd（scripts/hado-lsp.sh → cargo run）は exec で `hado lsp` に置き換わる。
-- それまでの時間は並走する門番と取り合う cargo の build lock 待ちなので、サーバの
-- 応答の予算（30 秒）に入れず、起動器の見張り（HADO_LSP_LAUNCH_MS）で待つ。
local function launched()
  for _, pid in ipairs(vim.api.nvim_get_proc_children(vim.fn.getpid())) do
    local proc = vim.api.nvim_get_proc(pid)
    if not proc or proc.name ~= 'hado' then return false end
  end
  return true
end

-- nvim_get_proc_children は pgrep を、nvim_get_proc は ps を起こす。vim.wait の条件に
-- 置くと、その子の終了が待ちを起こして条件を評価し直し、ps を回し続ける。
local function wait_launched()
  local deadline = vim.uv.hrtime() + tonumber(vim.env.HADO_LSP_LAUNCH_MS) * 1e6
  while not launched() do
    if vim.uv.hrtime() > deadline then error('timeout: lsp launcher', 0) end
    vim.wait(200)
  end
end

local function open(path)
  vim.cmd('edit ' .. vim.fn.fnameescape(path))
  wait_launched()
  wait_for(function()
    local clients = vim.lsp.get_clients({ bufnr = 0 })
    return #clients > 0 and clients[1].initialized == true
  end, 30000, 'lsp client')
  return vim.lsp.get_clients({ bufnr = 0 })[1]
end

local function request(client, method, params)
  local slot = {}
  client:request(method, params, function(err, res)
    slot.done, slot.err, slot.result = true, err, res
  end, 0)
  wait_for(function() return slot.done end, 30000, method)
  return slot
end

local function set_text(text)
  vim.api.nvim_buf_set_lines(0, 0, -1, false, vim.split(text, '\n', { plain = true }))
end

local function file_text(path)
  local file = assert(io.open(path, 'r'))
  local text = file:read('*a')
  file:close()
  return text
end

result.publishes = publishes
'''


# 起動器が `hado lsp` に exec するまでの見張り。cargo の build lock の待ちを含むので、
# run_hado と同じ 300 秒にする。
LAUNCH_SECONDS = 300


class Nvim:
    """Neovim を headless で起こし、Lua の probe を走らせて JSON を受け取る。"""

    def __init__(self, binary, root, work):
        self.binary = str(Path(binary).resolve())
        self.root = Path(root).resolve()
        # 作業場所は絶対パスにする。LSP の URI は `file:///abs/path` なので、
        # 相対の作業場所を nvim に渡すと file URI が壊れ、publish を取り落とす。
        self.work = Path(work).resolve()

    def run(self, body, env_extra=None, timeout=120 + LAUNCH_SECONDS):
        probe = self.work / 'probe.lua'
        probe.write_text(PROBE_HEAD + PRELUDE + '\n' + body + '\n' + PROBE_TAIL,
                         encoding='utf-8')
        out = self.work / 'probe.out'
        if out.exists():
            out.unlink()
        env = dict(os.environ)
        env.update({
            'HADO_LSP_ROOT': str(self.root),
            'HADO_LSP_WORK': str(self.work),
            'HADO_LSP_OUT': str(out),
            'HADO_LSP_LAUNCH_MS': str(LAUNCH_SECONDS * 1000),
        })
        for key, value in (env_extra or {}).items():
            env[key] = value
        for key in ('XDG_DATA_HOME', 'XDG_CONFIG_HOME', 'XDG_STATE_HOME',
                    'XDG_CACHE_HOME'):
            directory = self.work / 'xdg' / key
            directory.mkdir(parents=True, exist_ok=True)
            env[key] = str(directory)
        env['NVIM_LOG_FILE'] = str(self.work / 'nvim.log')
        proc = subprocess.run(
            [self.binary, '--headless', '-u', 'NONE', '-i', 'NONE',
             '-c', 'luafile ' + str(probe)],
            env=env, capture_output=True, text=True, timeout=timeout)
        if proc.returncode != 0:
            raise Failure('Neovim exited %d: %s'
                          % (proc.returncode, (proc.stderr or '')[-400:]))
        if not out.is_file():
            raise Failure('Neovim probe wrote no output: %s'
                          % (proc.stderr or '')[-400:])
        result = json.loads(out.read_text(encoding='utf-8'))
        if 'error' in result:
            raise Failure('probe error: %s' % str(result['error']).splitlines()[0])
        return result


def run_hado(root, args, timeout=300):
    return subprocess.run(['cargo', 'run', '--offline', '-q', '--'] + list(args),
                          cwd=str(root), capture_output=True, text=True,
                          timeout=timeout)


def make(work, *parts):
    directory = work.joinpath(*parts)
    directory.mkdir(parents=True, exist_ok=True)
    return directory


def write_text(path, text):
    path.write_text(text, encoding='utf-8')
    return text


def print_type(root, path, name):
    proc = run_hado(root, ['check', '--print-type=%s' % name, str(path)])
    out = (proc.stdout or '').strip()
    if not out:
        raise Failure('check --print-type=%s printed nothing (rc=%d): %s'
                      % (name, proc.returncode, (proc.stderr or '')[-300:]))
    return out.splitlines()[-1].strip()


MULTI = ('def decoy = "\u3042' + '\U0001F600' * 15
         + '" def target (b: Bool) => if b | true => 1 | false => 0'
         + '\ndef main = target true\n')
MULTI_IF = 'if b | true => 1 | false => 0'
ENC_CACHE = {}


# --------------------------------------------------------------------------
# 検査
# --------------------------------------------------------------------------

@check('lifecycle-shutdown')
def lifecycle_shutdown(nvim, work):
    directory = make(work, 'lifecycle')
    write_text(directory / 'a.hd', 'def main = 1\n')

    # shutdown に null が返り、続く exit で子プロセスが自分で終了コード 0 で終わる。
    after_shutdown = nvim.run(r'''
local path = work .. '/lifecycle/a.hd'
local exits = nil
vim.lsp.config('hado', { cmd = { cmd }, filetypes = { 'hado' },
  on_exit = function(code, signal) exits = { code = code, signal = signal } end })
local client = open(path)
local state = {}
client:request('shutdown', nil, function(err, res)
  state.done, state.err, state.result = true, err, res
end)
wait_for(function() return state.done end, 20000, 'shutdown response')
result.shutdown_null = (state.err == nil) and (state.result == nil or state.result == vim.NIL)
result.shutdown_error = state.err and tostring(state.err) or nil
client:notify('exit')
wait_for(function() return exits ~= nil end, 20000, 'exit after shutdown')
result.exit = exits
''')
    if not after_shutdown.get('shutdown_null'):
        raise Failure('shutdown did not answer null (error=%r)'
                      % after_shutdown.get('shutdown_error'))
    exit_code = after_shutdown['exit']['code']
    if after_shutdown['exit']['signal'] != 0 or exit_code != 0:
        raise Failure('exit after shutdown exited code=%r signal=%r, want 0/0'
                      % (exit_code, after_shutdown['exit']['signal']))

    # shutdown 無しの exit は非 0（1）で終わる。
    write_text(directory / 'b.hd', 'def main = 1\n')
    without_shutdown = nvim.run(r'''
local path = work .. '/lifecycle/b.hd'
local exits = nil
vim.lsp.config('hado', { cmd = { cmd }, filetypes = { 'hado' },
  on_exit = function(code, signal) exits = { code = code, signal = signal } end })
local client = open(path)
client:notify('exit')
wait_for(function() return exits ~= nil end, 20000, 'exit without shutdown')
result.exit = exits
''')
    exit_code = without_shutdown['exit']['code']
    if exit_code != 1:
        raise Failure('exit without shutdown exited %r, want 1' % (exit_code,))


@check('capabilities')
def capabilities(nvim, work):
    directory = make(work, 'capabilities')
    write_text(directory / 'a.hd', 'def main = 1\n')
    result = nvim.run(r'''
-- Neovim は initialize の result を resolve_capabilities で正規化してから
-- on_init へ渡す（textDocumentSync の 1 が表になる）。渡す前の写しを取る。
local raw = nil
local resolve = vim.lsp.protocol.resolve_capabilities
vim.lsp.protocol.resolve_capabilities = function(caps)
  raw = vim.deepcopy(caps)
  return resolve(caps)
end
local path = work .. '/capabilities/a.hd'
open(path)
wait_for(function() return raw ~= nil end, 20000, 'initialize result')
result.capabilities = raw
''')
    caps = result.get('capabilities') or {}
    sync = caps.get('textDocumentSync')
    if isinstance(sync, bool) or sync != 1:
        raise Failure('textDocumentSync is %r, want the kind 1'
                      % (sync,))
    for name in ('hoverProvider', 'codeActionProvider'):
        if caps.get(name) is not True:
            raise Failure('%s is %r, want true' % (name, caps.get(name)))
    encoding = caps.get('positionEncoding')
    if isinstance(encoding, list) or not isinstance(encoding, str):
        raise Failure('positionEncoding is %r, want a single string' % (encoding,))
    if encoding not in ('utf-8', 'utf-16', 'utf-32'):
        raise Failure('positionEncoding is %r, not a known encoding' % (encoding,))
    general = caps.get('general')
    if isinstance(general, dict) and 'positionEncoding' in general:
        raise Failure('positionEncoding is advertised under general, not top level')


def encoding_probe(nvim, work, mode):
    """client の申告を `mode` に固定した initialize を 1 回走らせる。"""
    if mode in ENC_CACHE:
        return ENC_CACHE[mode]
    directory = make(work, 'encoding', mode)
    path = directory / 'm.hd'
    write_text(path, MULTI)
    if mode == 'absent':
        setup = r'''
-- general.positionEncodings を capability から省く。client は既定の一覧を
-- 深く併合するので、既定を作る関数ごと差し替える。
local original = vim.lsp.protocol.make_client_capabilities
vim.lsp.protocol.make_client_capabilities = function()
  local caps = original()
  if caps.general then caps.general.positionEncodings = nil end
  return caps
end
'''
    else:
        setup = ("vim.lsp.config('hado', { cmd = { cmd }, filetypes = { 'hado' },"
                 " capabilities = { general = { positionEncodings = { '%s' } } } })\n"
                 % mode)
    body = setup + (r'''
local raw = nil
local resolve = vim.lsp.protocol.resolve_capabilities
vim.lsp.protocol.resolve_capabilities = function(caps)
  raw = vim.deepcopy(caps)
  return resolve(caps)
end
local path = work .. '/encoding/__MODE__/m.hd'
local uri = vim.uri_from_fname(path)
vim.lsp.config('hado', { cmd = { cmd }, filetypes = { 'hado' } })
open(path)
wait_for(function() return raw ~= nil end, 20000, 'initialize result')
wait_for(function()
  local ps = publishes_for(uri)
  return #ps > 0 and #ps[#ps].diagnostics > 0
end, 30000, 'diagnostics')
result.advertised = raw.positionEncoding
result.encoding = vim.lsp.get_clients({ bufnr = 0 })[1].offset_encoding
result.publishes = publishes_for(uri)
''').replace('__MODE__', mode)
    ENC_CACHE[mode] = nvim.run(body)
    return ENC_CACHE[mode]


def check_negotiated_range(nvim, work, mode, expected_encoding):
    result = encoding_probe(nvim, work, mode)
    advertised = result.get('advertised')
    if advertised != expected_encoding:
        raise Failure('%s: positionEncoding advertised %r, want %r'
                      % (mode, advertised, expected_encoding))
    expected = range_of(MULTI, MULTI_IF, expected_encoding)
    diagnostics = diags_of(result['publishes'])
    if expected not in ranges(diagnostics):
        raise Failure('%s: no diagnostic with range %r; got %r'
                      % (mode, expected, ranges(diagnostics)))


@check('encoding-negotiation')
def encoding_negotiation(nvim, work):
    for mode, expected in (('utf-8', 'utf-8'), ('utf-32', 'utf-32'),
                           ('absent', 'utf-16')):
        check_negotiated_range(nvim, work, mode, expected)


@check('diagnostics-range-encoding')
def diagnostics_range_encoding(nvim, work):
    for mode, expected in (('utf-8', 'utf-8'), ('utf-16', 'utf-16'),
                           ('utf-32', 'utf-32')):
        check_negotiated_range(nvim, work, mode, expected)


@check('diagnostics-unsaved-change')
def diagnostics_unsaved_change(nvim, work):
    directory = make(work, 'unsaved')
    old_text = 'def main = missing 1\n'
    new_text = 'def main =\n  let unused = 1 ->\n  0\n'
    fixed_text = 'def main = 1\n'
    path = directory / 'u.hd'
    write_text(path, old_text)
    write_text(directory / 'new.txt', new_text)

    result = nvim.run(r'''
local path = work .. '/unsaved/u.hd'
local uri = vim.uri_from_fname(path)
local new_text = file_text(work .. '/unsaved/new.txt')
open(path)
wait_for(function()
  local ps = publishes_for(uri)
  return #ps > 0 and #ps[#ps].diagnostics > 0
end, 30000, 'diagnostics of the disk text')
result.on_disk = publishes_for(uri)
set_text(new_text)
wait_for(function()
  local ps = publishes_for(uri)
  return #ps > 0 and ps[#ps].version ~= result.on_disk[#result.on_disk].version
    and #ps[#ps].diagnostics > 0
end, 30000, 'diagnostics of the unsaved text')
result.after_change = publishes_for(uri)
set_text('def main = 1\n')
wait_for(function()
  local ps = publishes_for(uri)
  return #ps > 0 and #ps[#ps].diagnostics == 0
    and ps[#ps].version ~= result.after_change[#result.after_change].version
end, 30000, 'empty diagnostics after the fix')
result.after_fix = publishes_for(uri)
result.disk = file_text(path)
''')
    if result['disk'] != old_text:
        raise Failure('the probe wrote the unsaved text to disk')
    on_disk = diags_of(result['on_disk'])
    old_point = point_range_of(old_text, 'missing', 'utf-8')
    if not any(diagnostic['range'] == old_point for diagnostic in on_disk):
        raise Failure('the disk text did not publish its own diagnostic at %r: %r'
                      % (old_point, ranges(on_disk)))
    after_change = diags_of(result['after_change'])
    new_point = point_range_of(new_text, 'unused', 'utf-8')
    if not any(diagnostic['range'] == new_point for diagnostic in after_change):
        raise Failure('the unsaved text did not publish its diagnostic at %r: %r'
                      % (new_point, ranges(after_change)))
    if any(diagnostic['code'] == 'E204' for diagnostic in after_change):
        raise Failure('the stale disk diagnostic E204 survived the unsaved change')
    after_fix = diags_of(result['after_fix'])
    if after_fix:
        raise Failure('removing the last diagnostic did not publish an empty array: %r'
                      % (after_fix,))


@check('hover-top-level-def')
def hover_top_level_def(nvim, work):
    directory = make(work, 'hover')
    path = directory / 'm.hd'
    write_text(path, MULTI)
    expected = print_type(nvim.root, path, 'target')
    decoy = print_type(nvim.root, path, 'decoy')
    position = range_of(MULTI, 'target', 'utf-8')['start']
    result = nvim.run(r'''
local path = work .. '/hover/m.hd'
vim.lsp.config('hado', { cmd = { cmd }, filetypes = { 'hado' },
  capabilities = { general = { positionEncodings = { 'utf-8' } } } })
local client = open(path)
wait_for(function()
  local ps = publishes_for(vim.uri_from_fname(path))
  return #ps > 0
end, 30000, 'diagnostics')
result.encoding = client.offset_encoding
result.hover = request(client, 'textDocument/hover', {
  textDocument = { uri = vim.uri_from_fname(path) },
  position = { line = __LINE__, character = __CHAR__ },
}).result
'''.replace('__LINE__', str(position['line'])).replace('__CHAR__', str(position['character'])))
    hover = result.get('hover')
    if not isinstance(hover, dict):
        raise Failure('hover at target returned %r, want a Hover' % (hover,))
    value = (hover.get('contents') or {}).get('value')
    if value != expected:
        raise Failure('hover at target returned %r, check --print-type says %r'
                      % (value, expected))
    if value == decoy:
        raise Failure('hover at target returned the type of decoy (%r)' % (value,))


@check('code-action-byte-identical')
def code_action_byte_identical(nvim, work):
    directory = make(work, 'codeaction')
    text = ('def target (b: Bool) => if b | true => 1 | false => 0\n'
            'def main = target true\n')
    needle = 'if b | true => 1 | false => 0'
    path = directory / 'fix.hd'
    write_text(path, text)
    expected_range = range_of(text, needle, 'utf-8')
    result = nvim.run(r'''
local path = work .. '/codeaction/fix.hd'
vim.lsp.config('hado', { cmd = { cmd }, filetypes = { 'hado' },
  capabilities = { general = { positionEncodings = { 'utf-8' } } } })
local client = open(path)
wait_for(function()
  local ps = publishes_for(vim.uri_from_fname(path))
  return #ps > 0 and #ps[#ps].diagnostics > 0
end, 30000, 'diagnostics')
result.encoding = client.offset_encoding
result.actions = request(client, 'textDocument/codeAction', {
  textDocument = { uri = vim.uri_from_fname(path) },
  range = { start = { line = __LINE__, character = __CHAR__ },
            ['end'] = { line = __LINE__, character = __CHAR__ } },
  context = { diagnostics = {} },
}).result
'''.replace('__LINE__', str(expected_range['start']['line']))
   .replace('__CHAR__', str(expected_range['start']['character'])))
    actions = result.get('actions')
    if not isinstance(actions, list) or not actions:
        raise Failure('the code action returned %r, want a non-empty array'
                      % (actions,))
    if len(actions) != 1:
        raise Failure('the code action returned %d actions, want 1'
                      % (len(actions),))
    changes = actions[0]['edit']['changes']
    edits = list(changes.values())[0]
    if len(edits) != 1:
        raise Failure('the code action returned %d edits, want 1' % (len(edits),))
    edit = edits[0]
    if edit['range'] != expected_range:
        raise Failure('the code action range is %r, want %r'
                      % (edit['range'], expected_range))
    patched = apply_edit(text, edit['range'], edit['newText'], result['encoding'])
    if patched == text:
        raise Failure('the code action rewrote nothing')

    # 同じ本文への `check --fix` の結果と、ファイルのバイト列で比べる。
    patched_path = directory / 'patched.hd'
    write_text(patched_path, patched)
    fixdir = make(directory, 'fixrun')
    fixture = fixdir / 'fix.hd'
    write_text(fixture, text)
    run_hado(nvim.root, ['check', '--fix', str(fixture)])
    patched_bytes = patched_path.read_bytes()
    fixed_bytes = fixture.read_bytes()
    if patched_bytes != fixed_bytes:
        raise Failure('the applied code action is not check --fix byte for byte:\n'
                      'action: %r\ncheck:  %r'
                      % (patched_bytes, fixed_bytes))


# `good` is valid and is the hover target. `broken`'s annotation does not
# resolve, which is what puts a diagnostic on the contract's own URI; it is not
# a member anyone hovers.
CONTRACT = ('type T = | A | B\n'
            'module M\n'
            'def good (x: Int): T\n'
            'def broken (x: Int): Missing\n')


def contract_probe(nvim, work, label, contract_rel, entry_rel, root_rel, open_entry,
                   explicit_root=True, library_marker=False):
    directory = make(work, 'contract', label)
    contract = directory / contract_rel
    contract.parent.mkdir(parents=True, exist_ok=True)
    write_text(contract, CONTRACT)
    entry = directory / entry_rel
    entry.parent.mkdir(parents=True, exist_ok=True)
    write_text(entry, 'def use (x: %s) => x\n' % PROBE_TYPE[label])
    if library_marker:
        # 根を渡さないとき、サーバは `library` を宣言する祖先まで辿る（§58.5）。
        write_text(directory / 'root.hd', 'library\n')
    member = range_of(CONTRACT, 'good', 'utf-8')['start']
    if explicit_root:
        config = ("vim.lsp.config('hado', { cmd = { cmd }, filetypes = { 'hado' },\n"
                  "  root_dir = directory .. '/__ROOT__' })").replace('__ROOT__', root_rel)
    else:
        # 根を渡さない。サーバが契約の祖先を辿って根を決める（§58.5）。
        config = ("vim.lsp.config('hado', { cmd = { cmd }, filetypes = { 'hado' } })")
    body = r'''
local directory = work .. '/contract/__LABEL__'
local contract = directory .. '/__CONTRACT__'
local entry = directory .. '/__ENTRY__'
__CONFIG__
local client = open(contract)
__OPEN_ENTRY__
local uri = vim.uri_from_fname(contract)
wait_for(function()
  local ps = publishes_for(uri)
  return #ps > 0 and #ps[#ps].diagnostics > 0
end, 30000, 'contract diagnostics')
result.encoding = client.offset_encoding
result.contract = contract
result.member = { line = __HLINE__, character = __HCHAR__ }
result.publishes = publishes_for(uri)
result.hover = request(client, 'textDocument/hover', {
  textDocument = { uri = uri },
  position = result.member,
}).result
result.actions = request(client, 'textDocument/codeAction', {
  textDocument = { uri = uri },
  range = { start = result.member, ['end'] = result.member },
  context = { diagnostics = {} },
}).result
'''.replace('__LABEL__', label).replace('__CONTRACT__', contract_rel) \
    .replace('__ENTRY__', entry_rel).replace('__CONFIG__', config) \
    .replace('__HLINE__', str(member['line'])).replace('__HCHAR__', str(member['character'])) \
   .replace('__OPEN_ENTRY__',
            ("open(entry)" if open_entry else ''))
    result = nvim.run(body)
    expected = point_range_of(CONTRACT, 'Missing', 'utf-8')
    diagnostics = diags_of(result['publishes'])
    if expected not in ranges(diagnostics):
        raise Failure('%s: no diagnostic on the contract at %r; got %r'
                      % (label, expected, ranges(diagnostics)))
    if result.get('actions') != []:
        raise Failure('%s: the contract code action returned %r, want []'
                      % (label, result.get('actions')))
    return result


PROBE_TYPE = {
    'flat': 'T',
    'nested': 'api::T',
    'together': 'T',
    'derived': 'api::T',
}


@check('show-all')
def show_all(nvim, work):
    # §58.4 / J2: `--show-all` は同じ段階の全件をアンカー順に返し、既定は先頭の
    # 1 件だけ。同じ欠陥を 2 行で出してはならない（名前解決と推論が同じ E204 を
    # 積むので、畳まないと同じ位置が 2 回出る）。
    directory = make(work, 'showall')
    # 独立な未束縛の名前を 2 つ置く。名前解決の段階で 2 件そろう。
    text = 'def main = a1 + a2\n'
    path = directory / 'names.hd'
    write_text(path, text)
    shown = run_hado(nvim.root, ['check', '--show-all', '--format', 'json', str(path)])
    if shown.returncode not in (0, 1):
        raise Failure('check --show-all exited %d: %s'
                      % (shown.returncode, (shown.stderr or '')[-200:]))
    # 診断は stderr に出る（stdout はプログラムの出力）。
    rows = [json.loads(line) for line in shown.stderr.splitlines() if line.startswith('{')]
    # 期待するアンカーは判定器が本文から数える。
    expected = sorted(json.dumps(point_range_of(text, name, 'utf-8'), sort_keys=True)
                      for name in ('a1', 'a2'))
    got = sorted(json.dumps({'start': {'line': r['line'] - 1, 'character': r['col'] - 1},
                             'end': {'line': r['end_line'] - 1, 'character': r['end_col'] - 1}},
                            sort_keys=True)
                 for r in rows)
    if len(rows) != len(set(got)):
        raise Failure('--show-all repeated an anchor: %r' % (got,))
    if got != expected:
        raise Failure('--show-all printed %r, want one row per unbound name %r'
                      % (got, expected))
    # 既定は先頭の 1 件だけ。
    plain = run_hado(nvim.root, ['check', '--format', 'json', str(path)])
    plain_rows = [json.loads(line) for line in plain.stderr.splitlines()
                  if line.startswith('{')]
    if len(plain_rows) != 1:
        raise Failure('the default check printed %d rows, want 1' % len(plain_rows))


@check('contract-hdi')
def contract_hdi(nvim, work):
    # 契約だけを開く（隣の .hd が入口）。
    flat = contract_probe(nvim, work, 'flat', 'store.hdi', 'main.hd', '.', False)
    # 入れ子の契約だけを開く。入口は祖先の main.hd で、契約の親だけを根にすると
    # 契約を読む候補が 0 になる。
    nested = contract_probe(nvim, work, 'nested', 'api/store.hdi', 'main.hd', '.', False)
    # 契約と、契約を読む .hd を一緒に開く。
    together = contract_probe(nvim, work, 'together', 'store.hdi', 'use.hd', '.', True)
    # 根を渡さない入れ子の契約。サーバが契約の祖先を辿って根を決めるので、
    # 契約の親だけを根にすると候補が 0 になり、診断も hover も出ない（§58.5）。
    derived = contract_probe(nvim, work, 'derived', 'api/store.hdi', 'main.hd', '.', False,
                             explicit_root=False, library_marker=True)

    for label, result in (('flat', flat), ('nested', nested), ('together', together),
                          ('derived', derived)):
        hover = result.get('hover')
        member = result['member']
        if not isinstance(hover, dict):
            raise Failure('%s: hover on the contract at %s %r returned %r, want a Hover'
                          % (label, result['contract'], member, hover))
        value = (hover.get('contents') or {}).get('value')
        if not isinstance(value, str) or not value:
            raise Failure('%s: hover value is %r' % (label, value))


@check('root-selection')
def root_selection(nvim, work):
    # §58.5: 契約だけを開いたとき、入口の候補は `workspaceFolders` のうち契約の
    # パス構成要素が接頭辞になる最も深い根から採る。文字列の接頭辞で選ぶと
    # `w/app` が `w/application` にも当たり、候補を採り違える。
    directory = make(work, 'roots')
    short = directory / 'w'
    long = short / 'app'
    application = short / 'application'
    for path in (long, application):
        path.mkdir(parents=True, exist_ok=True)
    # 入口は契約の型を使う。契約が入口の解析に載ることで、契約の URI に
    # 診断が付き、メンバの hover も引ける（§58.5）。
    write_text(short / 'lib.hd',
               'library\ndef use (x: application::T) => x\n')
    # 深い方の根にも候補を置く。接頭辞の判定を誤るとこちらが入口になる。
    write_text(long / 'only.hd', 'library\ndef lone (x: Int) => x\n')
    contract = application / 'c.hdi'
    write_text(contract, CONTRACT)

    member = range_of(CONTRACT, 'good', 'utf-8')['start']
    body = r'''
local contract = work .. '/roots/w/application/c.hdi'
local client = open(contract)
local uri = vim.uri_from_fname(contract)
wait_for(function()
  local ps = publishes_for(uri)
  return #ps > 0 and #ps[#ps].diagnostics > 0
end, 30000, 'contract diagnostics')
result.contract = contract
result.member = { line = __HLINE__, character = __HCHAR__ }
result.publishes = publishes_for(uri)
result.hover = request(client, 'textDocument/hover', {
  textDocument = { uri = uri },
  position = result.member,
}).result
result.actions = request(client, 'textDocument/codeAction', {
  textDocument = { uri = uri },
  range = { start = result.member, ['end'] = result.member },
  context = { diagnostics = {} },
}).result
'''.replace('__HLINE__', str(member['line'])).replace('__HCHAR__', str(member['character']))
    result = nvim.run(body, env_extra={
        'HADO_LSP_FOLDERS': '%s:%s' % (short.resolve(), long.resolve()),
    })
    # 契約の診断が出る（`w` の下の `lib.hd` が入口になった）。
    expected = point_range_of(CONTRACT, 'Missing', 'utf-8')
    if expected not in ranges(diags_of(result['publishes'])):
        raise Failure('no diagnostic on the contract at %r; got %r'
                      % (expected, ranges(diags_of(result['publishes']))))
    if result.get('actions') != []:
        raise Failure('the contract code action returned %r, want []'
                      % (result.get('actions'),))
    # 根が `w/app` になると、`w` の下の `lib.hd` は入口にならず候補が 0 に
    # なるので、診断が出ない。診断が出たこと自体が根の選択を弁別している。
    if len(result.get('folders') or []) != 2:
        raise Failure('the folder list did not reach the server: %r' % (result.get('folders'),))


@check('version-guard')
def version_guard(nvim, work):
    directory = make(work, 'version')
    many = 'def main = m1 + m2 + m3\n'
    few = 'def main = k1\n'
    write_text(directory / 'x.hd', few)
    write_text(directory / 'many.txt', many)
    write_text(directory / 'few.txt', few)
    stale = range_of(many, 'm1', 'utf-8')['start']
    result = nvim.run(r'''
local path = work .. '/version/x.hd'
local uri = vim.uri_from_fname(path)
local client = open(path)
wait_for(function() return #publishes_for(uri) > 0 end, 30000, 'open publish')
local opened = publishes_for(uri)
local open_version = opened[#opened].version
set_text(file_text(work .. '/version/many.txt'))
wait_for(function()
  local ps = publishes_for(uri)
  local last = ps[#ps]
  return last ~= nil and last.version ~= open_version and #last.diagnostics == 3
end, 30000, 'many diagnostics')
local many = publishes_for(uri)
local many_version = many[#many].version
set_text(file_text(work .. '/version/few.txt'))
wait_for(function()
  local ps = publishes_for(uri)
  local last = ps[#ps]
  return last ~= nil and last.version ~= many_version and #last.diagnostics == 1
end, 30000, 'few diagnostics')
local all = publishes_for(uri)
result.publishes = all
result.many_version = many_version
result.few_version = all[#all].version
local stale = { line = __VLINE__, character = __VCHAR__ }
result.stale_hover = request(client, 'textDocument/hover', {
  textDocument = { uri = uri, version = many_version },
  position = stale,
}).result
result.stale_actions = request(client, 'textDocument/codeAction', {
  textDocument = { uri = uri, version = many_version },
  range = { start = stale, ['end'] = stale },
  context = { diagnostics = {} },
}).result
'''.replace('__VLINE__', str(stale['line'])).replace('__VCHAR__', str(stale['character'])))
    publishes = result['publishes']
    if publishes[-1]['version'] != result['few_version']:
        raise Failure('the last publish carries version %r, want %r'
                      % (publishes[-1]['version'], result['few_version']))
    if result['few_version'] == result['many_version']:
        raise Failure('the two changes carried the same version %r'
                      % (result['many_version'],))
    expected = sorted(json.dumps(point_range_of(few, name, 'utf-8'), sort_keys=True)
                      for name in ('k1',))
    got = sorted(json.dumps(item, sort_keys=True) for item in ranges(diags_of(publishes)))
    if got != expected:
        raise Failure('the last publish does not hold the few defects: got %r want %r'
                      % (got, expected))
    # 公開は版の順に進む。ある版のあとに、それより古い版が公開されてはならない。
    # 最後の公開だけを見ると、追い越しが末尾の前で起きても見逃す。
    highest = None
    for publish in publishes:
        if highest is not None and publish['version'] < highest:
            raise Failure('version %r was published after version %r'
                          % (publish['version'], highest))
        highest = max(highest or publish['version'], publish['version'])
    # 追い越された版（N）の診断は、最後の N+1 の公開より前にしか現れてはならない。
    last_few = max(index for index, publish in enumerate(publishes)
                   if publish['version'] == result['few_version'])
    later = [publish for publish in publishes[last_few + 1:]
             if publish['version'] == result['many_version']]
    if later:
        raise Failure('a diagnostic of version %r was published after version %r'
                      % (result['many_version'], result['few_version']))
    if result.get('stale_hover') is not None:
        raise Failure('hover for the stale version returned %r, want null'
                      % (result.get('stale_hover'),))
    if result.get('stale_actions') != []:
        raise Failure('the stale code action returned %r, want []'
                      % (result.get('stale_actions'),))


def main():
    if len(sys.argv) != 4:
        print('usage: lsp-check.py <nvim> <repo-root> <work-dir>',
              file=sys.stderr)
        return 2
    nvim_path, root, work_dir = sys.argv[1], Path(sys.argv[2]).resolve(), Path(sys.argv[3])
    work_dir.mkdir(parents=True, exist_ok=True)
    nvim = Nvim(nvim_path, root, work_dir)

    ok = 0
    for name, fn in CHECKS:
        try:
            fn(nvim, work_dir)
        except Failure as error:
            print('NG: %s %s' % (name, error))
        except Exception as error:  # noqa: BLE001 - 判定器は理由を出して続ける
            print('NG: %s unexpected %s: %s' % (name, type(error).__name__, error))
        else:
            print('OK: %s' % name)
            ok += 1
    print('PASSED: %d/%d' % (ok, len(CHECKS)))
    return 0 if ok == len(CHECKS) else 1


if __name__ == '__main__':
    sys.exit(main())
