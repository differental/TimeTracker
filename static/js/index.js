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
                <label for="switch-start-input" style="display:block;text-align:left;font-size:0.85rem;color:#374151;margin-bottom:0.25rem;">Start time</label>
                <input id="switch-start-input" type="datetime-local" class="swal2-input" style="margin:0;" value="${nowVal}" min="${minVal}" max="${nowVal}" step="60">
                <p style="font-size:0.8rem;color:#6b7280;margin-top:0.5rem;">Defaults to now. Adjust if you switched earlier.</p>
            `,
            showCancelButton: true,
            confirmButtonText: 'Yes',
            cancelButtonText: 'Cancel',
            focusConfirm: false,
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