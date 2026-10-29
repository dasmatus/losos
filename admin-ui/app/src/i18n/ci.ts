import { defineMessages } from "./define";

/* Mesh CI (src/ci/), the screen at /ci: the GitHub Actions workflows of
 * LosOS Git's repositories, run on the boxes of the mesh. It exists only on
 * the shared side (sharing on, in a mesh). The name is a product name and
 * stays the same in every language. */
export default defineMessages({
  "ci.title": { en: "Mesh CI", sk: "Mesh CI", de: "Mesh CI" },
  "ci.summary": {
    en: "The workflows in .github/workflows of your LosOS Git repositories run here on every push and pull request, spread over the boxes of your mesh.",
    sk: "Workflowy v .github/workflows vašich repozitárov v LosOS Git tu bežia pri každom pushi a pull requeste, rozdelené medzi boxy vašej siete.",
    de: "Die Workflows in .github/workflows deiner Repositorys in LosOS Git laufen hier bei jedem Push und Pull Request, verteilt auf die Boxen deines Mesh.",
  },
  "ci.missing": {
    en: "Mesh CI runs only while this box shares its storage with a mesh it has joined, and runs LosOS Git.",
    sk: "Mesh CI beží, len kým tento box zdieľa svoje úložisko so sieťou, ku ktorej sa pripojil, a má LosOS Git.",
    de: "Mesh CI läuft nur, solange diese Box ihren Speicher mit einem Mesh teilt, dem sie beigetreten ist, und LosOS Git betreibt.",
  },
  "ci.failed": { en: "Mesh CI did not load: {detail}", sk: "Mesh CI sa nenačítalo: {detail}", de: "Mesh CI wurde nicht geladen: {detail}" },
  "ci.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },

  // ── this box's runner ──
  "ci.runner.title": { en: "This box's runner", sk: "Runner tohto boxu", de: "Runner dieser Box" },
  "ci.runner.lend": { en: "Run other boxes' jobs", sk: "Spúšťať úlohy iných boxov", de: "Jobs anderer Boxen ausführen" },
  "ci.lend.always": { en: "Always", sk: "Vždy", de: "Immer" },
  "ci.lend.window": { en: "In the compute window", sk: "V okne výpočtov", de: "Im Rechenfenster" },
  "ci.lend.never": { en: "Never", sk: "Nikdy", de: "Nie" },
  "ci.lend.saved": { en: "Saved.", sk: "Uložené.", de: "Gespeichert." },
  "ci.lend.windowHint": {
    en: "The compute window is {start} to {end}, as set on the Mesh page.",
    sk: "Okno výpočtov je od {start} do {end}, ako je nastavené na stránke Sieť.",
    de: "Das Rechenfenster ist von {start} bis {end}, wie auf der Seite Mesh eingestellt.",
  },
  "ci.status.lending": {
    en: "Taking jobs from the mesh now. Its own jobs come first.",
    sk: "Teraz berie úlohy zo siete. Vlastné úlohy majú prednosť.",
    de: "Nimmt jetzt Jobs aus dem Mesh an. Eigene Jobs gehen vor.",
  },
  "ci.status.notLending": {
    en: "Running this box's own jobs only.",
    sk: "Spúšťa len vlastné úlohy tohto boxu.",
    de: "Führt nur die eigenen Jobs dieser Box aus.",
  },
  "ci.status.mesh": {
    en: "The mesh's queue answers, so jobs can go to other boxes.",
    sk: "Front siete odpovedá: úlohy môžu ísť na iné boxy.",
    de: "Die Warteschlange des Mesh antwortet: Jobs können an andere Boxen gehen.",
  },
  "ci.status.noMesh": {
    en: "No edge in reach, so every job runs on this box.",
    sk: "Žiadny edge nie je na dosah: každá úloha beží na tomto boxe.",
    de: "Kein Edge erreichbar: Jeder Job läuft auf dieser Box.",
  },
  "ci.status.private": {
    en: "Jobs of private repositories run on this box only.",
    sk: "Úlohy súkromných repozitárov bežia len na tomto boxe.",
    de: "Jobs privater Repositorys laufen nur auf dieser Box.",
  },
  "ci.status.privateOnMesh": {
    en: "Jobs of private repositories may run on other boxes too.",
    sk: "Úlohy súkromných repozitárov môžu bežať aj na iných boxoch.",
    de: "Jobs privater Repositorys dürfen auch auf anderen Boxen laufen.",
  },

  // ── runs ──
  "ci.runs.title": { en: "Runs", sk: "Behy", de: "Läufe" },
  "ci.runs.empty": {
    en: "No runs yet. Push a workflow to .github/workflows in a repository on LosOS Git.",
    sk: "Zatiaľ žiadne behy. Pushnite workflow do .github/workflows v repozitári v LosOS Git.",
    de: "Noch keine Läufe. Pushe einen Workflow nach .github/workflows in einem Repository in LosOS Git.",
  },
  "ci.runs.more": { en: "Show more", sk: "Zobraziť viac", de: "Mehr anzeigen" },
  "ci.run.number": { en: "{workflow} #{number}", sk: "{workflow} č. {number}", de: "{workflow} #{number}" },
  "ci.run.by": { en: "{event} by {actor}", sk: "{event}, spustil {actor}", de: "{event} von {actor}" },
  "ci.run.back": { en: "All runs", sk: "Všetky behy", de: "Alle Läufe" },
  "ci.run.missing": { en: "There is no such run.", sk: "Taký beh neexistuje.", de: "Diesen Lauf gibt es nicht." },
  "ci.run.cancel": { en: "Cancel run", sk: "Zrušiť beh", de: "Lauf abbrechen" },
  "ci.run.rerun": { en: "Run again", sk: "Spustiť znova", de: "Erneut ausführen" },
  "ci.run.cancelled": { en: "The run is being cancelled.", sk: "Beh sa ruší.", de: "Der Lauf wird abgebrochen." },
  "ci.run.started": { en: "Run #{number} started.", sk: "Beh č. {number} sa spustil.", de: "Lauf #{number} gestartet." },
  "ci.run.jobs": { en: "Jobs", sk: "Úlohy", de: "Jobs" },
  "ci.run.repo": { en: "Repository", sk: "Repozitár", de: "Repository" },
  "ci.run.commit": { en: "Commit", sk: "Commit", de: "Commit" },
  "ci.run.duration": { en: "Took {time}", sk: "Trvalo {time}", de: "Dauerte {time}" },

  // ── events ──
  "ci.event.push": { en: "Push", sk: "Push", de: "Push" },
  "ci.event.pull_request": { en: "Pull request", sk: "Pull request", de: "Pull Request" },
  "ci.event.workflow_dispatch": { en: "Started by hand", sk: "Spustené ručne", de: "Von Hand gestartet" },

  // ── states ──
  "ci.state.queued": { en: "Queued", sk: "Vo fronte", de: "Wartet" },
  "ci.state.running": { en: "Running", sk: "Beží", de: "Läuft" },
  "ci.state.success": { en: "Passed", sk: "Prešlo", de: "Bestanden" },
  "ci.state.failure": { en: "Failed", sk: "Zlyhalo", de: "Fehlgeschlagen" },
  "ci.state.cancelled": { en: "Cancelled", sk: "Zrušené", de: "Abgebrochen" },
  "ci.state.skipped": { en: "Skipped", sk: "Preskočené", de: "Übersprungen" },
  "ci.state.error": { en: "Error", sk: "Chyba", de: "Fehler" },

  // ── a job ──
  "ci.job.onBox": { en: "This box", sk: "Tento box", de: "Diese Box" },
  "ci.job.onMesh": { en: "Mesh", sk: "Sieť", de: "Mesh" },
  "ci.job.ranOn": { en: "Ran on {runner}", sk: "Bežalo na {runner}", de: "Lief auf {runner}" },
  "ci.job.waiting": { en: "Waiting for a runner", sk: "Čaká na runner", de: "Wartet auf einen Runner" },
  "ci.job.pick": { en: "Pick a job to read its log.", sk: "Vyberte úlohu, ktorej záznam chcete čítať.", de: "Wähle einen Job, um sein Protokoll zu lesen." },
  "ci.job.noLog": { en: "Nothing in the log yet.", sk: "Záznam je zatiaľ prázdny.", de: "Im Protokoll steht noch nichts." },
  "ci.job.setUp": { en: "Set up job", sk: "Príprava úlohy", de: "Job vorbereiten" },
  "ci.job.outputs": { en: "Outputs", sk: "Výstupy", de: "Ausgaben" },
  "ci.job.exit": { en: "Exit code {code}", sk: "Návratový kód {code}", de: "Exit-Code {code}" },

  // ── run by hand ──
  "ci.dispatch.open": { en: "Run workflow", sk: "Spustiť workflow", de: "Workflow ausführen" },
  "ci.dispatch.title": { en: "Run a workflow", sk: "Spustiť workflow", de: "Workflow ausführen" },
  "ci.dispatch.text": {
    en: "You can start a workflow by hand when it lists workflow_dispatch under on:.",
    sk: "Workflowy, ktoré majú workflow_dispatch pod on:, sa dajú spustiť ručne.",
    de: "Workflows, die workflow_dispatch unter on: nennen, lassen sich von Hand starten.",
  },
  "ci.dispatch.repo": { en: "Repository", sk: "Repozitár", de: "Repository" },
  "ci.dispatch.repoHint": { en: "owner/name", sk: "vlastník/názov", de: "Besitzer/Name" },
  "ci.dispatch.ref": { en: "Branch", sk: "Vetva", de: "Branch" },
  "ci.dispatch.refHint": { en: "The default branch", sk: "Predvolená vetva", de: "Der Standard-Branch" },
  "ci.dispatch.workflow": { en: "Workflow", sk: "Workflow", de: "Workflow" },
  "ci.dispatch.none": {
    en: "No workflow on this branch takes workflow_dispatch.",
    sk: "Žiadny workflow na tejto vetve nemá workflow_dispatch.",
    de: "Kein Workflow auf diesem Branch nimmt workflow_dispatch an.",
  },
  "ci.dispatch.find": { en: "Find workflows", sk: "Nájsť workflowy", de: "Workflows suchen" },
  "ci.dispatch.run": { en: "Run", sk: "Spustiť", de: "Ausführen" },
  "ci.dispatch.close": { en: "Close", sk: "Zavrieť", de: "Schließen" },

  // ── time ──
  "ci.time.seconds": { en: "{count} s", sk: "{count} s", de: "{count} s" },
  "ci.time.minutes": { en: "{minutes} min {seconds} s", sk: "{minutes} min {seconds} s", de: "{minutes} min {seconds} s" },
});
