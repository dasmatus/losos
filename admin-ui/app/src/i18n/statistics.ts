import { defineMessages } from "./define";

/* Statistics (src/statistics/): what Traefik counted, on the box's Overview
 * and Settings › Statistics and on the edge's manage page. */

export default defineMessages({
  "statistics.title": { en: "Statistics", sk: "Štatistiky", de: "Statistik" },
  "statistics.open": { en: "Open", sk: "Otvoriť", de: "Öffnen" },
  "statistics.loading": {
    en: "Reading Traefik's counters…",
    sk: "Čítajú sa počítadlá Traefiku…",
    de: "Traefiks Zähler werden gelesen…",
  },
  "statistics.failed": {
    en: "The statistics did not load: {message}",
    sk: "Štatistiky sa nenačítali: {message}",
    de: "Die Statistik wurde nicht geladen: {message}",
  },
  "statistics.unavailable": {
    en: "Traefik's counters cannot be read yet.",
    sk: "Počítadlá Traefiku sa zatiaľ nedajú prečítať.",
    de: "Traefiks Zähler können noch nicht gelesen werden.",
  },
  "statistics.errorRate": {
    en: "{rate} errors",
    sk: "{rate} chýb",
    de: "{rate} Fehler",
  },
  "statistics.summary": {
    en: { one: "request in the last hour, {bytes} sent", other: "requests in the last hour, {bytes} sent" },
    sk: {
      one: "požiadavka za poslednú hodinu, odoslané {bytes}",
      few: "požiadavky za poslednú hodinu, odoslané {bytes}",
      many: "požiadaviek za poslednú hodinu, odoslané {bytes}",
      other: "požiadaviek za poslednú hodinu, odoslané {bytes}",
    },
    de: {
      one: "Anfrage in der letzten Stunde, {bytes} gesendet",
      other: "Anfragen in der letzten Stunde, {bytes} gesendet",
    },
  },
  "statistics.chart.label": {
    en: "{n} requests in the last hour, one bar per minute",
    sk: "{n} požiadaviek za poslednú hodinu, jeden stĺpec za minútu",
    de: "{n} Anfragen in der letzten Stunde, ein Balken pro Minute",
  },
  "statistics.chart.minute": {
    en: "{time}: {requests} requests, {errors} errors",
    sk: "{time}: {requests} požiadaviek, {errors} chýb",
    de: "{time}: {requests} Anfragen, {errors} Fehler",
  },

  // ── The page ──────────────────────────────────────────────────────────
  "statistics.figure.requests": {
    en: "Requests, last hour",
    sk: "Požiadavky za hodinu",
    de: "Anfragen, letzte Stunde",
  },
  "statistics.figure.errors": {
    en: "Server errors, last hour",
    sk: "Chyby servera za hodinu",
    de: "Serverfehler, letzte Stunde",
  },
  "statistics.figure.sent": {
    en: "Sent, last hour",
    sk: "Odoslané za hodinu",
    de: "Gesendet, letzte Stunde",
  },
  "statistics.figure.connections": {
    en: "Open connections",
    sk: "Otvorené spojenia",
    de: "Offene Verbindungen",
  },
  "statistics.hour.title": { en: "The last hour", sk: "Posledná hodina", de: "Die letzte Stunde" },
  "statistics.hour.start": { en: "60 min ago", sk: "pred 60 min", de: "vor 60 Min." },
  "statistics.hour.now": { en: "now", sk: "teraz", de: "jetzt" },
  "statistics.hour.p95": {
    en: "slowest minute's p95: {time}",
    sk: "p95 najpomalšej minúty: {time}",
    de: "p95 der langsamsten Minute: {time}",
  },
  "statistics.codes.title": { en: "Answers", sk: "Odpovede", de: "Antworten" },
  "statistics.codes.2xx": { en: "Answered (2xx)", sk: "Vybavené (2xx)", de: "Beantwortet (2xx)" },
  "statistics.codes.3xx": {
    en: "Sent elsewhere (3xx)",
    sk: "Presmerované (3xx)",
    de: "Weitergeleitet (3xx)",
  },
  "statistics.codes.4xx": {
    en: "Refused or not found (4xx)",
    sk: "Odmietnuté alebo nenájdené (4xx)",
    de: "Abgelehnt oder nicht gefunden (4xx)",
  },
  "statistics.codes.5xx": {
    en: "Server errors (5xx)",
    sk: "Chyby servera (5xx)",
    de: "Serverfehler (5xx)",
  },
  "statistics.protocols.title": { en: "Protocols", sk: "Protokoly", de: "Protokolle" },
  "statistics.protocols.caption": {
    en: "HTTP/3 runs over QUIC on UDP. A browser moves to it after its first visit.",
    sk: "HTTP/3 beží cez QUIC na UDP. Prehliadač naň prejde po prvej návšteve.",
    de: "HTTP/3 läuft über QUIC auf UDP. Ein Browser wechselt nach dem ersten Besuch dorthin.",
  },
  "statistics.routes.title": { en: "Routes", sk: "Trasy", de: "Routen" },
  "statistics.routes.none": {
    en: "Nothing has been routed since Traefik started.",
    sk: "Od spustenia Traefiku sa nič nesmerovalo.",
    de: "Seit dem Start von Traefik wurde nichts weitergeleitet.",
  },
  "statistics.routes.name": { en: "Name", sk: "Názov", de: "Name" },
  "statistics.routes.requests": { en: "Requests", sk: "Požiadavky", de: "Anfragen" },
  "statistics.routes.errors": { en: "Errors", sk: "Chyby", de: "Fehler" },
  "statistics.routes.sent": { en: "Sent", sk: "Odoslané", de: "Gesendet" },
  "statistics.routes.p95": { en: "p95", sk: "p95", de: "p95" },
  "statistics.routes.port": { en: "port {port}", sk: "port {port}", de: "Port {port}" },
  "statistics.since": {
    en: "Counted since Traefik started on {date} at {time}.",
    sk: "Počíta sa od spustenia Traefiku {date} o {time}.",
    de: "Gezählt seit dem Start von Traefik am {date} um {time}.",
  },
});
