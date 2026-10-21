import { defineMessages } from "./define";

/* The games a long wait offers (components/play-while-waiting.tsx,
 * src/games/). The names are the games' own and are translated like any
 * other word. */
export default defineMessages({
  "games.offer": {
    en: "Play while you wait",
    sk: "Zahrajte si, kým čakáte",
    de: "Spiel was, während du wartest",
  },
  "games.loading": {
    en: "Loading the game…",
    sk: "Načítava sa hra…",
    de: "Das Spiel wird geladen…",
  },
  "games.failed": {
    en: "The game could not be loaded.",
    sk: "Hru sa nepodarilo načítať.",
    de: "Das Spiel konnte nicht geladen werden.",
  },
  "games.close": { en: "Close game", sk: "Zavrieť hru", de: "Spiel schließen" },
  "games.another": { en: "Another game", sk: "Iná hra", de: "Anderes Spiel" },
  "games.ready": {
    en: "The wait is over, so the game has stopped.",
    sk: "Čakanie skončilo, hra sa zastavila.",
    de: "Das Warten ist vorbei, das Spiel ist angehalten.",
  },
  "games.role": { en: "game", sk: "hra", de: "Spiel" },

  // ── Over the canvas ─────────────────────────────────────────────────────
  "games.start": {
    en: "Press Space or tap to start",
    sk: "Začnite medzerníkom alebo ťuknutím",
    de: "Drück die Leertaste oder tippe, um zu starten",
  },
  "games.paused": {
    en: "Paused. Press a key or tap to go on.",
    sk: "Pozastavené. Pokračujte klávesom alebo ťuknutím.",
    de: "Pausiert. Drück eine Taste oder tippe, um weiterzuspielen.",
  },
  "games.over": {
    en: {
      one: "Round over with {count} point. Space or tap to play again.",
      other: "Round over with {count} points. Space or tap to play again.",
    },
    sk: {
      one: "Koniec kola, {count} bod. Znova hráte medzerníkom alebo ťuknutím.",
      few: "Koniec kola, {count} body. Znova hráte medzerníkom alebo ťuknutím.",
      many: "Koniec kola, {count} bodu. Znova hráte medzerníkom alebo ťuknutím.",
      other: "Koniec kola, {count} bodov. Znova hráte medzerníkom alebo ťuknutím.",
    },
    de: {
      one: "Runde vorbei, {count} Punkt. Leertaste oder Tippen für eine neue Runde.",
      other: "Runde vorbei, {count} Punkte. Leertaste oder Tippen für eine neue Runde.",
    },
  },
  "games.cleared": {
    en: {
      one: "Every pair found in {count} try. Space or tap to play again.",
      other: "Every pair found in {count} tries. Space or tap to play again.",
    },
    sk: {
      one: "Všetky dvojice nájdené na {count} pokus. Znova hráte medzerníkom alebo ťuknutím.",
      few: "Všetky dvojice nájdené na {count} pokusy. Znova hráte medzerníkom alebo ťuknutím.",
      many: "Všetky dvojice nájdené na {count} pokusu. Znova hráte medzerníkom alebo ťuknutím.",
      other: "Všetky dvojice nájdené na {count} pokusov. Znova hráte medzerníkom alebo ťuknutím.",
    },
    de: {
      one: "Alle Paare in {count} Versuch gefunden. Leertaste oder Tippen für eine neue Runde.",
      other: "Alle Paare in {count} Versuchen gefunden. Leertaste oder Tippen für eine neue Runde.",
    },
  },

  // ── The tally ───────────────────────────────────────────────────────────
  "games.score": { en: "Score {count}", sk: "Skóre {count}", de: "Punkte {count}" },
  "games.tries": { en: "Tries {count}", sk: "Pokusy {count}", de: "Versuche {count}" },
  "games.best": { en: "Best {count}", sk: "Najlepšie {count}", de: "Bestwert {count}" },

  // ── The five ────────────────────────────────────────────────────────────
  "games.name.upstream": { en: "Upstream", sk: "Proti prúdu", de: "Stromaufwärts" },
  "games.name.eel": { en: "Eel", sk: "Úhor", de: "Aal" },
  "games.name.merge": { en: "Merge", sk: "Spájanie", de: "Verschmelzen" },
  "games.name.bricks": { en: "Bricks", sk: "Tehličky", de: "Steine" },
  "games.name.pairs": { en: "Pairs", sk: "Dvojice", de: "Paare" },

  "games.how.upstream": {
    en: "Space, ↑ or a tap leaps the rocks. Hold it for a higher leap.",
    sk: "Medzerníkom, ↑ alebo ťuknutím preskočíte kamene. Podržte pre vyšší skok.",
    de: "Mit Leertaste, ↑ oder Tippen springst du über die Steine. Halte für einen höheren Sprung.",
  },
  "games.how.eel": {
    en: "Steer with the arrow keys or WASD, or swipe or tap beside the eel. Eat the shrimp and keep off the walls and your tail.",
    sk: "Riaďte šípkami alebo WASD, prípadne potiahnutím či ťuknutím vedľa úhora. Jedzte krevety a vyhnite sa stenám aj vlastnému chvostu.",
    de: "Steuere mit den Pfeiltasten oder WASD, oder wisch oder tipp neben den Aal. Friss die Garnelen und meide die Wände und deinen Schwanz.",
  },
  "games.how.merge": {
    en: "The arrow keys, WASD or a swipe slide every tile. Two equal tiles become one.",
    sk: "Šípky, WASD alebo potiahnutie posunú všetky dlaždice. Dve rovnaké sa spoja do jednej.",
    de: "Pfeiltasten, WASD oder Wischen schieben alle Kacheln. Zwei gleiche werden zu einer.",
  },
  "games.how.bricks": {
    en: "Move the paddle with the pointer or the arrow keys. Space or a tap serves.",
    sk: "Pálku posúvajte myšou, prstom alebo šípkami. Medzerník alebo ťuknutie podáva loptičku.",
    de: "Beweg den Schläger mit Maus, Finger oder Pfeiltasten. Leertaste oder Tippen schlägt auf.",
  },
  "games.how.pairs": {
    en: "Turn two cards over with a tap, or with the arrow keys and Space. Find every pair in as few tries as you can.",
    sk: "Otočte dve karty ťuknutím, alebo šípkami a medzerníkom. Nájdite všetky dvojice na čo najmenej pokusov.",
    de: "Dreh zwei Karten um, mit Tippen oder mit Pfeiltasten und Leertaste. Finde alle Paare mit möglichst wenigen Versuchen.",
  },
  // ── Read out for a screen reader ────────────────────────────────────────
  "games.board.row": { en: "Row {row}: {tiles}.", sk: "Riadok {row}: {tiles}.", de: "Reihe {row}: {tiles}." },
  "games.board.empty": { en: "empty", sk: "prázdne", de: "leer" },
  "games.board.card": {
    en: "Row {row}, card {column}: {card}.",
    sk: "Riadok {row}, karta {column}: {card}.",
    de: "Reihe {row}, Karte {column}: {card}.",
  },
  "games.board.up": { en: "Face up: {shapes}.", sk: "Odkryté: {shapes}.", de: "Aufgedeckt: {shapes}." },
  "games.board.found": {
    en: "Pairs found: {found} of {all}.",
    sk: "Nájdené dvojice: {found} z {all}.",
    de: "Gefundene Paare: {found} von {all}.",
  },
  "games.card.down": { en: "face down", sk: "lícom nadol", de: "verdeckt" },
  "games.card.matched": { en: "{shape}, paired", sk: "{shape}, nájdená dvojica", de: "{shape}, Paar gefunden" },
  "games.shape.disc": { en: "disc", sk: "kruh", de: "Scheibe" },
  "games.shape.ring": { en: "ring", sk: "krúžok", de: "Ring" },
  "games.shape.square": { en: "square", sk: "štvorec", de: "Quadrat" },
  "games.shape.triangle": { en: "triangle", sk: "trojuholník", de: "Dreieck" },
  "games.shape.diamond": { en: "diamond", sk: "kosoštvorec", de: "Raute" },
  "games.shape.plus": { en: "plus", sk: "plus", de: "Plus" },
  "games.shape.star": { en: "star", sk: "hviezda", de: "Stern" },
  "games.shape.salmon": { en: "salmon", sk: "losos", de: "Lachs" },
});
