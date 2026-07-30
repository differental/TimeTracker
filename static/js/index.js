const NOW_TICK_MS = 1000;
const NOW_POLL_MS = 5000;
const NOW_POLL_MAX_MS = 60000;

let nowPairs = [];
let nowSig = null;
let nowStart = 0;
let nowState = null;
let nowHasEntry = false;
let nowPollMs = NOW_POLL_MS;
let nowTickTimer = null;
let nowPollTimer = null;
let nowQueue = null;
let nowGeneration = 0;

let switchEls = null;
let pendingState = null;
let useNow = true;

function updateElapsed() {
    const el = document.getElementById('elapsed');
    if (!el) return;

    if (!nowHasEntry) {
        el.textContent = '-';
        return;
    }

    const d = Math.max(0, Date.now() - nowStart);
    const h = Math.floor(d / 3600000);
    const m = Math.floor((d % 3600000) / 60000);
    const s = Math.floor((d % 60000) / 1000);
    el.textContent = `${pad(h)}:${pad(m)}:${pad(s)}`;
}

function renderCurrentState() {
    const idle = APP.idleState || { emoji: '', name: '', colour: '#FFF' };
    const meta = nowHasEntry ? stateMeta(nowState) : null;

    const label = document.getElementById('current-state');
    if (label) label.textContent = meta ? `${meta.emoji} ${meta.name}` : `${idle.emoji} ${idle.name}`;

    const since = document.getElementById('since');
    if (since) since.textContent = nowHasEntry ? `since ${clockHM(nowStart)}` : '';

    const root = document.documentElement;
    root.style.setProperty('--state', meta ? meta.colour : idle.colour);
    if (nowHasEntry && nowState === APP.emergencyState) {
        root.setAttribute('data-emergency', 'true');
    } else {
        root.removeAttribute('data-emergency');
    }

    document.querySelectorAll('.change-state-btn').forEach(btn => {
        if (nowHasEntry && Number(btn.value) === nowState) {
            btn.setAttribute('aria-current', 'true');
        } else {
            btn.removeAttribute('aria-current');
        }
    });
}

function applyCurrentEntry(entry) {
    const state = entry ? Number(entry[0]) : null;
    const startMs = entry ? Number(entry[1]) : 0;

    if (state === nowState && startMs === nowStart) return false;

    const stateChanged = state !== nowState;
    nowState = state;
    nowStart = startMs;
    nowHasEntry = entry !== null;
    renderCurrentState();
    updateElapsed();
    return stateChanged;
}

function nowSignature(pairs) {
    return pairs.map(p => `${p[0]}@${p[1]}`).join('|');
}

function applyPairs(pairs, sweep) {
    const sig = nowSignature(pairs);
    const changed = sig !== nowSig;
    nowSig = sig;
    nowPairs = pairs;

    const stateChanged = applyCurrentEntry(pairs.length ? pairs[0] : null);
    dialUpdate(nowPairs, { rebuild: changed, sweep });
    if (stateChanged) loadSuggestions();
}

async function fetchNow(sweep) {
    const generation = nowGeneration;

    try {
        const resp = await fetch(`/api/recents?days=2&key=${encodeURIComponent(window.ENTRY_KEY)}`);
        if (!resp.ok) throw new Error(`Request failed (${resp.status}).`);
        const pairs = await resp.json();
        if (!Array.isArray(pairs)) throw new Error('Unexpected data.');
        if (generation !== nowGeneration) return;
        nowPollMs = NOW_POLL_MS;
        applyPairs(pairs, sweep);
    } catch (err) {
        if (generation !== nowGeneration) return;
        nowPollMs = Math.min(nowPollMs * 2, NOW_POLL_MAX_MS);
    }
}

function refreshNow(options = {}) {
    if (document.hidden && !options.force) return Promise.resolve();
    nowQueue = (nowQueue || Promise.resolve()).then(() => fetchNow(options.sweep === true));
    return nowQueue;
}

function schedulePoll() {
    clearTimeout(nowPollTimer);
    nowPollTimer = setTimeout(async () => {
        await refreshNow();
        if (nowTickTimer !== null) schedulePoll();
    }, nowPollMs);
}

function nowTick() {
    updateElapsed();
    dialUpdate(nowPairs);
}

function onNowVisibility() {
    if (document.hidden) return;
    if (nowTickTimer === null) return;
    refreshNow({ force: true });
    schedulePoll();
}

function onNowPageShow() {
    if (nowTickTimer === null) return;
    clearInterval(nowTickTimer);
    nowTickTimer = setInterval(nowTick, NOW_TICK_MS);
    onNowVisibility();
}

function setUseNow(on) {
    useNow = on;
    switchEls.nowBtn.setAttribute('aria-pressed', String(on));
}

function showSwitchError(message) {
    switchEls.error.textContent = message;
    switchEls.error.classList.remove('hidden');
}

function clearSwitchError() {
    switchEls.error.textContent = '';
    switchEls.error.classList.add('hidden');
}

function setSwitchBusy(busy, label) {
    switchEls.confirm.disabled = busy;
    switchEls.cancel.disabled = busy;
    switchEls.input.disabled = busy;
    switchEls.nowBtn.disabled = busy;
    switchEls.confirm.textContent = label;
}

function resolveStartTimestamp() {
    if (useNow) {
        return Date.now();
    }
    const val = switchEls.input && switchEls.input.value;
    if (!val) {
        showSwitchError('Please choose a start time.');
        return null;
    }
    const ms = new Date(val).getTime();
    if (Number.isNaN(ms)) {
        showSwitchError('Invalid date/time.');
        return null;
    }
    if (ms > Date.now()) {
        showSwitchError('Start time cannot be in the future.');
        return null;
    }
    if (ms < nowStart) {
        showSwitchError('Start time must be after the current activity started.');
        return null;
    }
    return ms;
}

function closeSwitchSheet() {
    if (switchEls.sheet.open) switchEls.sheet.close();
}

async function submitSwitch() {
    clearSwitchError();
    const startTimestamp = resolveStartTimestamp();
    if (startTimestamp === null) return;

    setSwitchBusy(true, 'Saving…');
    try {
        // force: true lets add_entry accept a backdated start (it otherwise
        // rejects timestamps older than ~5s). The picker already guards
        // start < ts <= now, and the backend still enforces ordering.
        const payload = { new_state: pendingState, start_timestamp: startTimestamp, force: true };
        const response = await fetch(`/api/entry?key=${window.ENTRY_KEY}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
        });

        if (!response.ok) {
            const errText = await response.text();
            setSwitchBusy(false, 'Change activity');
            showSwitchError(errText || `Request failed (${response.status}).`);
            return;
        }

        await response.json();
        setSwitchBusy(true, 'Saved');
        closeSwitchSheet();
        navInvalidate();
        await refreshNow({ force: true });
        schedulePoll();
    } catch (err) {
        setSwitchBusy(false, 'Change activity');
        showSwitchError((err && err.message) ? err.message : String(err));
    }
}

function openSwitchSheet(stateIdx) {
    pendingState = stateIdx;
    // `nowStart` is the current activity's start time - the earliest legal start
    // for the next activity. Default the picker to now.
    const nowVal = msToDatetimeLocal(Date.now());
    switchEls.input.value = nowVal;
    switchEls.input.min = msToDatetimeLocal(nowStart);
    switchEls.input.max = nowVal;
    switchEls.target.textContent = stateName(pendingState);
    clearSwitchError();
    setSwitchBusy(false, 'Change activity');
    switchEls.sheet.showModal();
    document.getElementById('switch-title').focus();
    setUseNow(true);
}

function wireSwitchSheet() {
    document.querySelectorAll('.change-state-btn').forEach(btn => {
        btn.addEventListener('click', (ev) => {
            openSwitchSheet(parseInt(ev.currentTarget.value, 10));
        });
    });

    switchEls.nowBtn.addEventListener('click', () => setUseNow(true));
    switchEls.input.addEventListener('focus', () => setUseNow(false));
    switchEls.input.addEventListener('input', () => setUseNow(false));
    switchEls.confirm.addEventListener('click', () => submitSwitch());
    switchEls.cancel.addEventListener('click', () => closeSwitchSheet());

    switchEls.sheet.addEventListener('click', (ev) => {
        if (ev.target === switchEls.sheet) closeSwitchSheet();
    });

    switchEls.sheet.addEventListener('keydown', (ev) => {
        if (ev.key === 'Enter' && !switchEls.confirm.disabled) {
            ev.preventDefault();
            submitSwitch();
        }
    });
}

function initNow() {
    switchEls = {
        sheet: document.getElementById('switch-sheet'),
        input: document.getElementById('switch-start-input'),
        nowBtn: document.getElementById('switch-now-btn'),
        target: document.getElementById('switch-target'),
        error: document.getElementById('switch-error'),
        confirm: document.getElementById('switch-confirm'),
        cancel: document.getElementById('switch-cancel')
    };
    if (!switchEls.sheet) return;

    nowGeneration += 1;
    nowPairs = [];
    nowSig = null;
    nowPollMs = NOW_POLL_MS;
    nowQueue = null;
    nowState = Number.isInteger(APP.currentState) ? APP.currentState : null;
    nowHasEntry = nowState !== null;
    nowStart = nowHasEntry ? Date.now() - Number(APP.elapsedMs || 0) : 0;

    dialReset();
    dialUpdate([], { rebuild: true });
    updateElapsed();
    renderCurrentState();
    wireSwitchSheet();
    initSuggest();

    nowTickTimer = setInterval(nowTick, NOW_TICK_MS);
    refreshNow({ force: true, sweep: true });
    schedulePoll();

    document.addEventListener('visibilitychange', onNowVisibility);
    window.addEventListener('focus', onNowVisibility);
    window.addEventListener('pageshow', onNowPageShow);
}

function destroyNow() {
    nowGeneration += 1;
    clearInterval(nowTickTimer);
    clearTimeout(nowPollTimer);
    nowTickTimer = null;
    nowPollTimer = null;
    nowQueue = null;
    switchEls = null;
    dialReset();
    document.removeEventListener('visibilitychange', onNowVisibility);
    window.removeEventListener('focus', onNowVisibility);
    window.removeEventListener('pageshow', onNowPageShow);
}

window.PAGES = window.PAGES || {};
window.PAGES.index = { init: initNow, destroy: destroyNow };
