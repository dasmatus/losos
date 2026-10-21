import { defineMessages } from "./define";

/* Calls from LosOS chat (src/calls/). */
export default defineMessages({
  "calls.voice": { en: "Voice call", sk: "Hlasový hovor", de: "Sprachanruf" },
  "calls.video": { en: "Video call", sk: "Videohovor", de: "Videoanruf" },
  "calls.incomingVoice": {
    en: "{name} is calling you",
    sk: "{name} vám volá",
    de: "{name} ruft dich an",
  },
  "calls.incomingVideo": {
    en: "{name} is video calling you",
    sk: "{name} vám volá s videom",
    de: "{name} ruft dich per Video an",
  },
  "calls.inRoom": { en: "in {room}", sk: "v miestnosti {room}", de: "in {room}" },
  "calls.accept": { en: "Answer", sk: "Prijať", de: "Annehmen" },
  "calls.acceptVoice": { en: "Answer without video", sk: "Prijať bez videa", de: "Ohne Video annehmen" },
  "calls.decline": { en: "Decline", sk: "Odmietnuť", de: "Ablehnen" },
  "calls.hangUp": { en: "Leave the call", sk: "Opustiť hovor", de: "Anruf verlassen" },
  "calls.mute": { en: "Turn off microphone", sk: "Vypnúť mikrofón", de: "Mikrofon ausschalten" },
  "calls.unmute": { en: "Turn on microphone", sk: "Zapnúť mikrofón", de: "Mikrofon einschalten" },
  "calls.cameraOff": { en: "Turn off camera", sk: "Vypnúť kameru", de: "Kamera ausschalten" },
  "calls.cameraOn": { en: "Turn on camera", sk: "Zapnúť kameru", de: "Kamera einschalten" },
  "calls.you": { en: "You", sk: "Vy", de: "Du" },
  "calls.connecting": { en: "Connecting…", sk: "Pripája sa…", de: "Verbindung wird aufgebaut…" },
  "calls.waiting": {
    en: "Waiting for the others to answer",
    sk: "Čaká sa, kým ostatní prijmú hovor",
    de: "Warten, bis die anderen annehmen",
  },
  "calls.noKeyYet": { en: "Waiting for the key", sk: "Čaká sa na kľúč", de: "Warten auf den Schlüssel" },
  "calls.encrypted": {
    en: "End-to-end encrypted",
    sk: "Šifrované end-to-end",
    de: "Ende-zu-Ende-verschlüsselt",
  },
  "calls.encryptedDetail": {
    en: "Every frame is encrypted on the sender's device and only the people in the call can open it. The box forwards the call without being able to see or hear it.",
    sk: "Každý snímok sa zašifruje v zariadení odosielateľa a otvoriť ho môžu len ľudia v hovore. Zariadenie LosOS hovor len preposiela a nevidí ani nepočuje ho.",
    de: "Jedes Bild wird auf dem Gerät des Absenders verschlüsselt und nur die Leute im Anruf können es öffnen. Die Box leitet den Anruf weiter, ohne ihn sehen oder hören zu können.",
  },
  "calls.frames": {
    en: "{sealed} frames sent encrypted, {opened} received and decrypted",
    sk: "Odoslané šifrované snímky: {sealed}, prijaté a dešifrované: {opened}",
    de: "{sealed} Bilder verschlüsselt gesendet, {opened} empfangen und entschlüsselt",
  },
  "calls.ended": { en: "The call ended", sk: "Hovor sa skončil", de: "Der Anruf ist beendet" },
  "calls.failed": {
    en: "The call could not continue",
    sk: "Hovor nemohol pokračovať",
    de: "Der Anruf konnte nicht fortgesetzt werden",
  },
  "calls.noEncryption": {
    en: "This browser cannot encrypt calls, so it cannot join them. Use a current Firefox, Chrome, Edge or Safari.",
    sk: "Tento prehliadač nevie šifrovať hovory, preto sa k nim nemôže pripojiť. Použite aktuálny Firefox, Chrome, Edge alebo Safari.",
    de: "Dieser Browser kann Anrufe nicht verschlüsseln und daher nicht teilnehmen. Nutze ein aktuelles Firefox, Chrome, Edge oder Safari.",
  },
  "calls.noDevices": {
    en: "The browser did not let the call use the microphone or camera",
    sk: "Prehliadač nepovolil hovoru použiť mikrofón ani kameru",
    de: "Der Browser hat dem Anruf Mikrofon und Kamera nicht freigegeben",
  },
  "calls.ongoing": { en: "Call in progress", sk: "Prebieha hovor", de: "Laufender Anruf" },
  "calls.join": { en: "Join", sk: "Pripojiť sa", de: "Beitreten" },
  "calls.started": { en: "{name} started a call", sk: "Hovor od {name}", de: "{name} hat einen Anruf gestartet" },
});
