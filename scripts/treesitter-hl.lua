-- highlight-treesitter の門番が Neovim のハイライトを測るための探針。
-- 環境変数 HADO_SO / HADO_RTP / HADO_FILE / HADO_OUT を受け取る。
--
-- 本物の TUI（門番は pty の下で nvim を起こす）で `redraw!` を回し、tree-sitter
-- highlighter が `nvim_buf_set_extmark` に渡した highlight group をその場で記録する。
-- highlighter の extmark は `ephemeral = true` で、redraw の外から get_extmarks
-- では読めないため、設定の呼び出しを包んで観測する。
--
-- 探針の本体は pcall で包む。query が壊れていると `edit` や `redraw!` が投げ、
-- そのままだと Neovim が入力待ちで止まって門番が赤ではなくタイムアウトになる。
-- 失敗しても記録済みの行を書き出して必ず `qa!` する。
local so = assert(vim.env.HADO_SO, 'HADO_SO')
local rtp = assert(vim.env.HADO_RTP, 'HADO_RTP')
local file = assert(vim.env.HADO_FILE, 'HADO_FILE')
local out = assert(vim.env.HADO_OUT, 'HADO_OUT')

local lines = {}
local function emit(s)
    lines[#lines + 1] = s
end

local marks = {}
local real_set = vim.api.nvim_buf_set_extmark
vim.api.nvim_buf_set_extmark = function(buf, ns, row, col, opts)
    marks[#marks + 1] = {
        ns = ns,
        row = row,
        col = col,
        end_row = opts.end_row,
        end_col = opts.end_col,
        hl = opts.hl_group,
    }
    return real_set(buf, ns, row, col, opts)
end

local function probe()
    vim.cmd('filetype on')
    vim.opt.runtimepath:prepend(rtp)
    vim.filetype.add({ extension = { hd = 'hado', hdi = 'hado' } })
    vim.treesitter.language.add('hado', { path = so })
    vim.cmd('edit ' .. vim.fn.fnameescape(file))

    emit('filetype ' .. vim.bo.filetype)
    local parser_ok = pcall(vim.treesitter.get_parser, 0)
    emit('parser ' .. tostring(parser_ok))
    local query_ok, query = pcall(vim.treesitter.query.get, 'hado', 'highlights')
    emit('query ' .. tostring(query_ok) .. ' ' .. tostring(query and #query.captures or -1))
    if query_ok and query then
        for _, name in ipairs(query.captures) do
            -- highlighter は `_` で始まる capture を highlighter_query.lua で捨てる。
            if not vim.startswith(name, '_') then
                local id = vim.api.nvim_get_hl_id_by_name('@' .. name .. '.hado')
                -- query.captures の名前は `@` を含まない。門番は `@` 付きで読む。
                emit('capture ' .. id .. ' @' .. name)
            end
        end
    end

    local start_ok, start_err = pcall(vim.treesitter.start)
    emit('start ' .. tostring(start_ok) .. ' ' .. (tostring(start_err):gsub('%s+', ' ')))
    local buf = vim.api.nvim_get_current_buf()
    emit('active ' .. tostring(vim.treesitter.highlighter.active[buf] ~= nil))
    local redraw_ok, redraw_err = pcall(vim.cmd, 'redraw!')
    emit('redraw ' .. tostring(redraw_ok) .. ' ' .. (tostring(redraw_err):gsub('%s+', ' ')))

    local tsns = vim.api.nvim_get_namespaces()['nvim.treesitter.highlighter']
    for _, m in ipairs(marks) do
        if m.ns == tsns and m.end_row ~= nil then
            emit(string.format('mark %d %d %d %d %d', m.row, m.col, m.end_row, m.end_col, m.hl))
        end
    end
end

local ok, err = pcall(probe)
if not ok then
    emit('probe false ' .. (tostring(err):gsub('%s+', ' ')))
end

local f = io.open(out, 'w')
f:write(table.concat(lines, '\n') .. '\n')
f:close()
vim.cmd('qa!')
