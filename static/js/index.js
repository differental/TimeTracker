function updateElapsed() {
    const d = Date.now() - start;
    const h = Math.floor(d / 3600000);
    const m = Math.floor((d % 3600000) / 60000);
    const s = Math.floor((d % 60000) / 1000);
    document.getElementById('elapsed').textContent = `${pad(h)}:${pad(m)}:${pad(s)}`;
}

document.querySelectorAll('.change-state-btn').forEach(btn => {
    btn.addEventListener('click', async (ev) => {
        const newState = parseInt(ev.currentTarget.value, 10);
        // `start` (from index.html) is the current activity's start time - the
        // earliest legal start for the next activity. Default the picker to now.
        const nowVal = msToDatetimeLocal(Date.now());
        const minVal = msToDatetimeLocal(start);
        // When `useNow` is set, "Yes" logs at the instant it is clicked rather
        // than at the value in the picker. It is toggled on by the NOW button
        // and toggled back off as soon as the picker regains focus.
        let useNow = false;
        const result = await Swal.fire({
            icon: 'warning',
            title: 'Confirmation',
            html: `
                <p style="margin-bottom:0.75rem;">Change state to <strong>${STATES_NAMES[newState]}</strong>?</p>
                <div style="display:flex; align-items:center; justify-content:center; gap:0.5rem;">
                    <input id="switch-start-input" type="datetime-local" class="swal2-input" style="margin:0;" value="${nowVal}" min="${minVal}" max="${nowVal}" step="60">
                    <button type="button" id="switch-now-btn" class="swal2-styled" style="margin:0; padding:0.4em 0.9em; font-size:0.9em; background:transparent; color:#7066e0; box-shadow:inset 0 0 0 2px #7066e0;">NOW</button>
                </div>
            `,
            showCancelButton: true,
            confirmButtonText: 'Yes',
            cancelButtonText: 'Cancel',
            didOpen: () => {
                const input = document.getElementById('switch-start-input');
                const nowBtn = document.getElementById('switch-now-btn');
                const setFilled = (filled) => {
                    useNow = filled;
                    if (filled) {
                        nowBtn.style.background = '#7066e0';
                        nowBtn.style.color = '#fff';
                    } else {
                        nowBtn.style.background = 'transparent';
                        nowBtn.style.color = '#7066e0';
                    }
                };
                nowBtn.addEventListener('click', () => setFilled(true));
                input.addEventListener('focus', () => setFilled(false));
            },
            preConfirm: () => {
                if (useNow) {
                    // Log at the moment "Yes" is clicked.
                    return Date.now();
                }
                const el = document.getElementById('switch-start-input');
                const val = el && el.value;
                if (!val) {
                    Swal.showValidationMessage('Please choose a start time.');
                    return false;
                }
                const ms = new Date(val).getTime();
                if (Number.isNaN(ms)) {
                    Swal.showValidationMessage('Invalid date/time.');
                    return false;
                }
                if (ms > Date.now()) {
                    Swal.showValidationMessage('Start time cannot be in the future.');
                    return false;
                }
                if (ms < start) {
                    Swal.showValidationMessage('Start time must be after the current activity started.');
                    return false;
                }
                return ms;
            }
        });
        if (!result.isConfirmed) return;
        const startTimestamp = result.value;

        try {
            Swal.fire({
                allowOutsideClick: false,
                didOpen: () => {
                    Swal.showLoading();
                }
            });
            // force: true lets add_entry accept a backdated start (it otherwise
            // rejects timestamps older than ~5s). The picker already guards
            // start < ts <= now, and the backend still enforces ordering.
            const payload = { new_state: newState, start_timestamp: startTimestamp, force: true };
            const response = await fetch(`/api/entry?key=${window.ENTRY_KEY}`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload)
            });
            Swal.close();

            if (!response.ok) {
                const errText = await response.text();
                await Swal.fire({ icon: 'error', title: 'Error', text: errText });
                return;
            }

            await response.json();

            Swal.fire({
                icon: 'success',
                title: 'Update Successful',
                text: 'State saved. Reloading...',
                showConfirmButton: false,
                timer: 2000,
                timerProgressBar: true,
                willClose: () => location.reload()
            });
        } catch (err) {
            Swal.close();
            const msg = (err && err.message) ? err.message : String(err);
            await Swal.fire({ icon: 'error', title: 'Error', text: msg });
        }
    });
});
