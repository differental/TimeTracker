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
        const result = await Swal.fire({
            icon: 'warning',
            title: 'Confirmation',
            html: `
                <p style="margin-bottom:0.75rem;">Change state to <strong>${STATES_NAMES[newState]}</strong>?</p>
                <input id="switch-start-input" type="datetime-local" class="swal2-input" style="margin:0;" value="${nowVal}" min="${minVal}" max="${nowVal}" step="60">
            `,
            showCancelButton: true,
            showDenyButton: true,
            confirmButtonText: 'Yes',
            denyButtonText: 'Now',
            cancelButtonText: 'Cancel',
            preConfirm: () => {
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
        // The "Now" (deny) button replicates the legacy logic: skip the picker
        // and send the current wall-clock time straight to the backend. It is
        // reported in nanoseconds - Date.now() is in ms, so scale by 1e6 with
        // BigInt to keep full precision (ms * 1e6 overflows a safe Number).
        let startTimestamp;
        if (result.isConfirmed) {
            startTimestamp = result.value;
        } else if (result.isDenied) {
            startTimestamp = BigInt(Date.now()) * 1000000n;
        } else {
            return;
        }

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
