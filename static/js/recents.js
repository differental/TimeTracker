// Remember the last-applied filter so we can reload the table after an edit.
let lastCount = 0;
let lastDays = 1;
let editingIdx = null;
let editingState = null;
let editUseNow = false;

function recentsError(message) {
    const el = document.getElementById('recents-error');
    if (!el) return;
    if (!message) {
        el.textContent = '';
        el.classList.add('hidden');
        return;
    }
    el.textContent = message;
    el.classList.remove('hidden');
}

function renderRibbon(data) {
    const ribbon = document.getElementById('ribbon');
    const fromEl = document.getElementById('ribbon-from');
    const toEl = document.getElementById('ribbon-to');
    if (!ribbon) return;
    ribbon.innerHTML = '';

    const oldest = (Array.isArray(data) && data.length) ? Number(data[data.length - 1][1]) : null;
    const to = Date.now();
    const segs = (oldest !== null && isValidMs(oldest)) ? buildSegments(data, oldest, to) : [];

    if (segs.length === 0) {
        ribbon.className = 'ribbon-empty';
        ribbon.textContent = 'No entries in this range.';
        if (fromEl) fromEl.textContent = '';
        if (toEl) toEl.textContent = '';
        return;
    }

    ribbon.className = 'ribbon';
    const span = Math.max(1, to - segs[0].start);

    segs.forEach((seg, i) => {
        const el = document.createElement('div');
        el.className = 'ribbon-seg';
        el.style.background = stateColour(seg.state);
        el.style.width = `${((seg.end - seg.start) / span) * 100}%`;
        el.style.setProperty('--delay', `${Math.min(i * 18, 420)}ms`);
        el.title = `${stateName(seg.state)} · ${formatRounded(seg.start)} · ${msToReadable(seg.end - seg.start)}`;
        ribbon.appendChild(el);
    });

    if (fromEl) fromEl.textContent = formatRounded(segs[0].start);
    if (toEl) toEl.textContent = 'now';
}

function buildRow(stateIdx, startMs, endMs, entryIdx) {
    const tr = document.createElement('tr');

    const stateTd = document.createElement('td');
    stateTd.className = 'cell-activity';
    const stateDiv = document.createElement('span');
    stateDiv.className = 'cell-state';
    const dot = document.createElement('span');
    dot.className = 'cell-dot';
    dot.style.background = stateColour(stateIdx);
    stateDiv.appendChild(dot);
    const label = document.createElement('span');
    label.textContent = stateName(stateIdx);
    stateDiv.appendChild(label);
    stateTd.appendChild(stateDiv);

    const startTd = document.createElement('td');
    startTd.className = 'cell-time cell-start';
    startTd.textContent = formatRounded(startMs);
    startTd.dataset.hm = clockHM(startMs);

    const endTd = document.createElement('td');
    endTd.className = 'cell-time cell-end';
    endTd.textContent = formatRounded(endMs);
    endTd.dataset.hm = clockHM(endMs);

    const durTd = document.createElement('td');
    durTd.className = 'cell-dur';
    if (isValidMs(startMs) && isValidMs(endMs)) {
        const durationMs = Number(endMs) - Number(startMs);
        durTd.textContent = msToReadable(durationMs);
    } else {
        durTd.textContent = 'Error';
    }

    const editTd = document.createElement('td');
    editTd.className = 'cell-edit';
    // entryIdx is null when we couldn't determine the global index (e.g. the
    // length lookup failed) - omit the button rather than risk editing the wrong row.
    if (Number.isInteger(entryIdx) && entryIdx >= 0) {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'row-edit';
        btn.textContent = 'Edit';
        btn.setAttribute('aria-label', `Edit ${stateName(stateIdx)} entry`);
        btn.addEventListener('click', () => openEditDialog(entryIdx, stateIdx, startMs));
        editTd.appendChild(btn);
    }

    tr.append(stateTd, startTd, endTd, durTd, editTd);
    return tr;
}

function showEditError(message) {
    const el = document.getElementById('edit-error');
    el.textContent = message;
    el.classList.remove('hidden');
}

function clearEditError() {
    const el = document.getElementById('edit-error');
    el.textContent = '';
    el.classList.add('hidden');
}

function populateStateSelect() {
    const select = document.getElementById('edit-state-select');
    if (!select) return;
    select.innerHTML = '';
    for (let i = 0; i < APP.states.length; i++) {
        const option = document.createElement('option');
        option.value = String(i);
        option.textContent = stateLabel(i);
        select.appendChild(option);
    }
}

// Opens the edit dialog for a single entry's activity and start time and PUTs
// the change. The backend (PUT /api/entry/{idx}) validates that the new start
// stays between the neighbouring entries and returns the error text on violation.
function openEditDialog(entryIdx, stateIdx, startMs) {
    const sheet = document.getElementById('edit-sheet');
    const input = document.getElementById('edit-start-input');
    const select = document.getElementById('edit-state-select');
    const target = document.getElementById('edit-target');
    editingIdx = entryIdx;
    editingState = stateIdx;
    input.value = msToDatetimeLocal(startMs);
    select.value = String(stateIdx);
    target.textContent = stateName(stateIdx);
    clearEditError();
    setEditUseNow(false);
    setEditBusy(false, 'Save');
    sheet.showModal();
    document.getElementById('edit-title').focus();
}

function setEditUseNow(on) {
    editUseNow = on;
    const btn = document.getElementById('edit-now-btn');
    if (btn) btn.setAttribute('aria-pressed', String(on));
}

function setEditBusy(busy, label) {
    document.getElementById('edit-save').disabled = busy;
    document.getElementById('edit-cancel').disabled = busy;
    document.getElementById('edit-start-input').disabled = busy;
    document.getElementById('edit-state-select').disabled = busy;
    const nowBtn = document.getElementById('edit-now-btn');
    if (nowBtn) nowBtn.disabled = busy;
    document.getElementById('edit-save').textContent = label;
}

async function saveEdit() {
    const input = document.getElementById('edit-start-input');
    const select = document.getElementById('edit-state-select');
    clearEditError();

    let ms;
    if (editUseNow) {
        ms = Date.now();
    } else {
        const val = input && input.value;
        if (!val) {
            showEditError('Please choose a date and time.');
            return;
        }
        ms = new Date(val).getTime();
        if (Number.isNaN(ms)) {
            showEditError('Invalid date/time.');
            return;
        }
    }

    const stateIdx = parseInt(select && select.value, 10);
    if (!Number.isInteger(stateIdx) || stateIdx < 0 || stateIdx >= APP.states.length) {
        showEditError('Please choose an activity.');
        return;
    }

    // Only send new_state when it actually changed: the backend rejects a state
    // matching a neighbouring entry, and an untouched state shouldn't be able to
    // trip that check on what is otherwise a start-time-only edit.
    const payload = { start_timestamp: ms };
    if (stateIdx !== editingState) payload.new_state = stateIdx;

    setEditBusy(true, 'Saving…');
    try {
        const resp = await fetch(`/api/entry/${editingIdx}?key=${encodeURIComponent(window.ENTRY_KEY)}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
        });
        if (!resp.ok) {
            setEditBusy(false, 'Save');
            showEditError(await resp.text());
            return;
        }
        await resp.json();
        document.getElementById('edit-sheet').close();
        loadEvents(lastCount, lastDays);
    } catch (err) {
        setEditBusy(false, 'Save');
        showEditError((err && err.message) ? err.message : String(err));
    }
}

async function loadEvents(count = 0, days = 1) {
    lastCount = count;
    lastDays = days;
    const tbody = document.getElementById('events-tbody');
    tbody.innerHTML = '';
    recentsError('');
    try {
        // Recents entries come back newest-first without their global index, but
        // the feed always starts at the newest entry (len-1) and the days cutoff
        // only trims the old tail, so row i is entry index len-1-i. Fetch the total
        // length to map rows to indices for editing.
        let length = null;
        try {
            const lenResp = await fetch(`/api/length?key=${encodeURIComponent(window.ENTRY_KEY)}`);
            if (lenResp.ok) {
                const n = Number(await lenResp.json());
                if (Number.isFinite(n)) length = n;
            }
        } catch (_) {
            length = null;
        }

        const params = new URLSearchParams();
        // 0 means N/A - ignoring the limit in either count or days. Here we don't pass it in.
        // Note the backend logic actually replaces Nones with very large defaults (300 count, 30 days).
        if (typeof count !== 'undefined' && count !== null && count !== 0) params.set('count', String(count));
        if (typeof days !== 'undefined' && days !== null && days !== 0) params.set('days', String(days));
        params.set('key', window.ENTRY_KEY);

        const resp = await fetch(`/api/recents?${params.toString()}`);
        if (!resp.ok) {
            recentsError(await resp.text());
            renderRibbon([]);
            return;
        }
        const data = await resp.json();
        if (!Array.isArray(data)) {
            recentsError('API returned unexpected data.');
            renderRibbon([]);
            return;
        }

        renderRibbon(data);

        if (data.length === 0) {
            const tr = document.createElement('tr');
            const td = document.createElement('td');
            td.colSpan = 5;
            td.className = 'table-empty';
            td.textContent = 'No entries in this range.';
            tr.appendChild(td);
            tbody.appendChild(tr);
            return;
        }

        for (let i = 0; i < data.length; i++) {
            const entry = data[i];
            if (!Array.isArray(entry) || entry.length < 2) continue;
            const stateIdx = entry[0];
            const startMs = Number(entry[1]);
            const endMs = (i === 0) ? Date.now() : Number(data[i - 1][1]);
            const entryIdx = (length !== null) ? (length - 1 - i) : null;
            const row = buildRow(stateIdx, startMs, endMs, entryIdx);
            tbody.appendChild(row);
        }
    } catch (err) {
        recentsError((err && err.message) ? err.message : String(err));
        renderRibbon([]);
    }
}

function initRecents() {
    const countSelect = document.getElementById('count-select');
    const countCustom = document.getElementById('count-custom-input');
    const daysSelect = document.getElementById('days-select');
    const daysCustom = document.getElementById('days-custom-input');
    const applyBtn = document.getElementById('apply-filters');
    const editSheet = document.getElementById('edit-sheet');
    if (!countSelect || !editSheet) return;

    lastCount = 0;
    lastDays = 1;
    editingIdx = null;
    editingState = null;
    editUseNow = false;

    populateStateSelect();

    document.getElementById('edit-save').addEventListener('click', () => saveEdit());
    document.getElementById('edit-cancel').addEventListener('click', () => editSheet.close());
    editSheet.addEventListener('click', (ev) => {
        if (ev.target === editSheet) editSheet.close();
    });

    const editNowBtn = document.getElementById('edit-now-btn');
    const editStartInput = document.getElementById('edit-start-input');
    editNowBtn.addEventListener('click', () => setEditUseNow(true));
    editStartInput.addEventListener('focus', () => setEditUseNow(false));
    editStartInput.addEventListener('input', () => setEditUseNow(false));
    ['edit-start-input', 'edit-state-select'].forEach(id => {
        document.getElementById(id).addEventListener('keydown', (ev) => {
            if (ev.key === 'Enter') {
                ev.preventDefault();
                saveEdit();
            }
        });
    });

    function toggleCustom(selectEl, customInputEl) {
        if (selectEl.value === 'custom') {
            customInputEl.classList.remove('hidden');
            customInputEl.focus();
        } else {
            customInputEl.classList.add('hidden');
        }
    }

    countSelect.addEventListener('change', () => {
        toggleCustom(countSelect, countCustom);
        if (countSelect.value !== 'custom') {
            const r = parseInt(countSelect.value, 10);
            if (!Number.isNaN(r)) loadEvents(r, getDaysValue());
        }
    });

    daysSelect.addEventListener('change', () => {
        toggleCustom(daysSelect, daysCustom);
        if (daysSelect.value !== 'custom') {
            const d = parseInt(daysSelect.value, 10);
            if (!Number.isNaN(d)) loadEvents(getCountValue(), d);
        }
    });

    // Allow pressing Enter on custom inputs to apply
    [countCustom, daysCustom].forEach(inp => {
        inp.addEventListener('keydown', (ev) => {
            if (ev.key === 'Enter') applyBtn.click();
        });
    });

    applyBtn.addEventListener('click', () => {
        const r = getCountValue();
        const d = getDaysValue();
        if (!Number.isInteger(r) || r < 0) {
            recentsError('Please enter a valid positive integer for items.');
            return;
        }
        if (!Number.isInteger(d) || d < 0) {
            recentsError('Please enter a valid positive integer for days.');
            return;
        }
        loadEvents(r, d);
    });

    function getCountValue() {
        if (countSelect.value === 'custom') {
            const n = parseInt(countCustom.value, 10);
            return Number.isNaN(n) ? null : n;
        }
        const n = parseInt(countSelect.value, 10);
        return Number.isNaN(n) ? null : n;
    }

    function getDaysValue() {
        if (daysSelect.value === 'custom') {
            const n = parseInt(daysCustom.value, 10);
            return Number.isNaN(n) ? null : n;
        }
        const n = parseInt(daysSelect.value, 10);
        return Number.isNaN(n) ? null : n;
    }

    countSelect.value = '0';
    daysSelect.value = '1';
    countCustom.value = '10';
    daysCustom.value = '1';
    toggleCustom(countSelect, countCustom);
    toggleCustom(daysSelect, daysCustom);

    loadEvents(0, 1);
}

window.PAGES = window.PAGES || {};
window.PAGES.recents = { init: initRecents };
