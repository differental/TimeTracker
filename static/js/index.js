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
        // "Now" selection state. While selected, pressing "Yes" submits the
        // current wall-clock time (captured at the Yes click) instead of the
        // value in the date picker.
        let useNow = false;
        const result = await Swal.fire({
            icon: 'warning',
            title: 'Confirmation',
            html: `
                <p style="margin-bottom:0.75rem;">Change state to <strong>${STATES_NAMES[newState]}</strong>?</p>
                <div style="display:flex; gap:0.5rem; align-items:stretch;">
                    <input id="switch-start-input" type="datetime-local" class="swal2-input" style="margin:0; flex:1 1 auto; min-width:0;" value="${nowVal}" min="${minVal}" max="${nowVal}" step="60">
                    <button type="button" id="switch-now-btn" class="swal2-styled" aria-pressed="false" style="margin:0; flex:0 0 auto; padding:0 0.75rem; font-size:0.9rem; background-color:#6e7881; color:#fff;">Now</button>
                </div>
            `,
            showCancelButton: true,
            confirmButtonText: 'Yes',
            cancelButtonText: 'Cancel',
            didOpen: () => {
                // The inline "Now" button (right of the picker, sized so both
                // share one line on mobile) does NOT submit. Clicking it just
                // marks "Now" as selected; focusing the picker clears it, both
                // in state and visually.
                const nowBtn = document.getElementById('switch-now-btn');
                const dtInput = document.getElementById('switch-start-input');
                const setNow = (on) => {
                    useNow = on;
                    nowBtn.setAttribute('aria-pressed', on ? 'true' : 'false');
                    nowBtn.style.boxShadow = on ? '0 0 0 3px rgba(110, 120, 129, 0.55)' : '';
                    nowBtn.style.filter = on ? 'brightness(1.15)' : '';
                };
                nowBtn.addEventListener('click', () => setNow(true));
                dtInput.addEventListener('focus', () => setNow(false));
            },
            preConfirm: () => {
                // "Now" selected: submit the current time as of this Yes click,
                // in nanoseconds. Date.now() is in ms, so scale by 1e6 with
                // BigInt to keep full precision (ms * 1e6 overflows a safe
                // Number).
                if (useNow) {
                    return BigInt(Date.now()) * 1000000n;
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
        // Either the "Now" nanosecond BigInt or the picker value in ms.
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
            // Build the body by hand: start_timestamp may be a BigInt (the "Now"
            // nanosecond value), which JSON.stringify cannot serialize.
            const body = `{"new_state":${newState},"start_timestamp":${startTimestamp},"force":true}`;
            const response = await fetch(`/api/entry?key=${window.ENTRY_KEY}`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body
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
