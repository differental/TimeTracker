function updateElapsed() {
    const d = Date.now() - start;
    const h = Math.floor(d / 3600000);
    const m = Math.floor((d % 3600000) / 60000);
    const s = Math.floor((d % 60000) / 1000);
    document.getElementById('elapsed').textContent = `${pad(h)}:${pad(m)}:${pad(s)}`;
}

const switchSheet = document.getElementById('switch-sheet');
const switchInput = document.getElementById('switch-start-input');
const switchNowBtn = document.getElementById('switch-now-btn');
const switchTarget = document.getElementById('switch-target');
const switchError = document.getElementById('switch-error');
const switchConfirm = document.getElementById('switch-confirm');
const switchCancel = document.getElementById('switch-cancel');

let pendingState = null;
let useNow = true;

function setUseNow(on) {
    useNow = on;
    switchNowBtn.setAttribute('aria-pressed', String(on));
}

function showSwitchError(message) {
    switchError.textContent = message;
    switchError.classList.remove('hidden');
}

function clearSwitchError() {
    switchError.textContent = '';
    switchError.classList.add('hidden');
}

function setSwitchBusy(busy, label) {
    switchConfirm.disabled = busy;
    switchCancel.disabled = busy;
    switchInput.disabled = busy;
    switchNowBtn.disabled = busy;
    switchConfirm.textContent = label;
}

function resolveStartTimestamp() {
    if (useNow) {
        return Date.now();
    }
    const val = switchInput && switchInput.value;
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
    if (ms < start) {
        showSwitchError('Start time must be after the current activity started.');
        return null;
    }
    return ms;
}

function closeSwitchSheet() {
    if (switchSheet.open) switchSheet.close();
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
        setTimeout(() => location.reload(), 420);
    } catch (err) {
        setSwitchBusy(false, 'Change activity');
        showSwitchError((err && err.message) ? err.message : String(err));
    }
}

document.querySelectorAll('.change-state-btn').forEach(btn => {
    btn.addEventListener('click', (ev) => {
        pendingState = parseInt(ev.currentTarget.value, 10);
        // `start` (from index.html) is the current activity's start time - the
        // earliest legal start for the next activity. Default the picker to now.
        const nowVal = msToDatetimeLocal(Date.now());
        switchInput.value = nowVal;
        switchInput.min = msToDatetimeLocal(start);
        switchInput.max = nowVal;
        switchTarget.textContent = STATES_NAMES[pendingState];
        clearSwitchError();
        setSwitchBusy(false, 'Change activity');
        switchSheet.showModal();
        document.getElementById('switch-title').focus();
        setUseNow(true);
    });
});

switchNowBtn.addEventListener('click', () => setUseNow(true));
switchInput.addEventListener('focus', () => setUseNow(false));
switchInput.addEventListener('input', () => setUseNow(false));
switchConfirm.addEventListener('click', () => submitSwitch());
switchCancel.addEventListener('click', () => closeSwitchSheet());

switchSheet.addEventListener('click', (ev) => {
    if (ev.target === switchSheet) closeSwitchSheet();
});

switchSheet.addEventListener('keydown', (ev) => {
    if (ev.key === 'Enter' && !switchConfirm.disabled) {
        ev.preventDefault();
        submitSwitch();
    }
});
