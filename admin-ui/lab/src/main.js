// ── Boot ────────────────────────────────────────────────────────────────
PLATE = LAB.plate;
if (LAB.box) $('#back').hidden = false;
$('#logo').src = PLATE;
$('#splashLogo').src = PLATE;
if (LAB.meme) { $('#memeNo').src = LAB.meme[0]; $('#memeYes').src = LAB.meme[1]; $('#memeNo').hidden = $('#memeYes').hidden = false; }
const splashShown = performance.now();
function splashSay(t) { const m = document.getElementById('splashMsg'); if (m) m.textContent = t; }
function splashDone() {
  const s = document.getElementById('splash'); if (!s) return;
  setTimeout(() => { s.classList.add('gone'); setTimeout(() => s.remove(), 500); }, Math.max(0, 1400 - (performance.now() - splashShown)));
}
$('#scenario').onchange = (e) => pickScenario(e.target.value);
document.querySelectorAll('[data-view]').forEach(b => b.onclick = () => { UI.view = b.dataset.view; fitView(); renderAll({ keepSide: true }); });
document.querySelectorAll('[data-mode]').forEach(b => b.onclick = () => {
  SIM.mode = b.dataset.mode; SIM.playing = false;
  if (SIM.mode === 'realtime') ensureRunning();
  toast(SIM.mode === 'simulation' ? 'Simulation: traffic waits for Play or Step, and every hop is listed.' : 'Realtime: traffic flows as it happens.');
  renderAll({ keepSide: true });
});
const cv = svg();
cv.addEventListener('pointerdown', onDown);
cv.addEventListener('pointermove', onMove);
cv.addEventListener('pointerup', onUp);
cv.addEventListener('pointercancel', onUp);
cv.addEventListener('wheel', onWheel, { passive: false });
cv.addEventListener('keydown', (e) => { const n = e.target.closest && e.target.closest('[data-dev]'); if (n && (e.key === 'Enter' || e.key === ' ')) { UI.sel = { kind: 'dev', id: n.dataset.dev }; renderCanvas(); renderSide(); e.preventDefault(); } });
$('#zin').onclick = () => zoomBy(1.2); $('#zout').onclick = () => zoomBy(1 / 1.2); $('#zfit').onclick = () => { fitView(); renderCanvas(); };
let rsz; window.addEventListener('resize', () => { clearTimeout(rsz); rsz = setTimeout(() => { fitView(); renderCanvas(); }, 120); });
const startEngine = typeof ENGINE !== 'undefined' && ENGINE.init ? ENGINE.init() : Promise.resolve();
startClock();
const readBox = LAB.box ? readThisBox() : Promise.resolve(null);
Promise.allSettled([startEngine, readBox]).then(async () => {
  const box = await readBox;
  if (box && box.settings) SCENARIOS['this-box'] = thisBoxScenario(box);
  const keys = Object.keys(SCENARIOS).sort((a, b) => (b === 'this-box') - (a === 'this-box'));
  // Signed out, "This box" stays in the list, greyed, saying what it needs.
  const signedOut = box && box.error ? `<option disabled>This box: ${esc(box.error.startsWith('Sign in') ? 'sign in on the admin page first' : 'not answering')}</option>` : '';
  $('#scenario').innerHTML = signedOut + keys.map(k => `<option value="${k}">${esc(SCENARIOS[k].name)}</option>`).join('');
  initFiles();
  splashSay('Placing the devices');
  loadScenario(keys[0] === 'this-box' ? 'this-box' : 'two-sites');
  $('#scenario').value = UI.scenario;
  renderTop();
  requestAnimationFrame(() => { fitView(); renderCanvas(); splashDone(); });
  if (box && box.error) toast(box.error, 9000);
});
