import { defineMessages } from "./define";

/* A person's profile (screens/settings/pane-account.tsx, profile-crop.tsx,
 * components/person.tsx): the picture, banner, colour, pronouns and line
 * about them that everyone signed in to the box sees. */

export default defineMessages({
  "profile.title": { en: "Profile", sk: "Profil", de: "Profil" },
  "profile.caption": {
    en: "Everyone who signs in to this box sees your profile next to your name, here and in its apps. Pictures are cropped and saved again on the box, so where a photo was taken stays on your device.",
    sk: "Každý, kto sa prihlási do tohto boxu, vidí váš profil vedľa vášho mena, tu aj v jeho aplikáciách. Obrázky sa na boxe orežú a uložia nanovo, takže miesto, kde bola fotka nasnímaná, zostane vo vašom zariadení.",
    de: "Alle, die sich an dieser Box anmelden, sehen dein Profil neben deinem Namen, hier und in ihren Apps. Bilder werden auf der Box zugeschnitten und neu gespeichert, sodass der Aufnahmeort eines Fotos auf deinem Gerät bleibt.",
  },
  "profile.failed": {
    en: "Your profile could not be saved.",
    sk: "Váš profil sa nepodarilo uložiť.",
    de: "Dein Profil konnte nicht gespeichert werden.",
  },
  "profile.saved": { en: "Profile saved", sk: "Profil uložený", de: "Profil gespeichert" },
  "profile.save": { en: "Save profile", sk: "Uložiť profil", de: "Profil speichern" },
  "profile.choose": { en: "Choose…", sk: "Vybrať…", de: "Auswählen…" },
  "profile.replace": { en: "Replace…", sk: "Nahradiť…", de: "Ersetzen…" },
  "profile.remove": { en: "Remove", sk: "Odstrániť", de: "Entfernen" },

  "profile.picture.title": { en: "Picture", sk: "Fotka", de: "Bild" },
  "profile.picture.detail": {
    en: "Square, shown in a circle",
    sk: "Štvorcová, zobrazená v kruhu",
    de: "Quadratisch, im Kreis gezeigt",
  },
  "profile.picture.saved": { en: "Picture saved", sk: "Fotka uložená", de: "Bild gespeichert" },
  "profile.picture.removed": { en: "Picture removed", sk: "Fotka odstránená", de: "Bild entfernt" },
  "profile.banner.title": { en: "Banner", sk: "Banner", de: "Banner" },
  "profile.banner.detail": {
    en: "Three times as wide as it is tall",
    sk: "Trikrát širší ako vysoký",
    de: "Dreimal so breit wie hoch",
  },
  "profile.banner.saved": { en: "Banner saved", sk: "Banner uložený", de: "Banner gespeichert" },
  "profile.banner.removed": { en: "Banner removed", sk: "Banner odstránený", de: "Banner entfernt" },

  "profile.colour.title": { en: "Colour", sk: "Farba", de: "Farbe" },
  "profile.colour.teal": { en: "Teal", sk: "Modrozelená", de: "Petrol" },
  "profile.colour.blue": { en: "Blue", sk: "Modrá", de: "Blau" },
  "profile.colour.violet": { en: "Violet", sk: "Fialová", de: "Violett" },
  "profile.colour.rose": { en: "Rose", sk: "Ružová", de: "Rosé" },
  "profile.colour.amber": { en: "Amber", sk: "Jantárová", de: "Bernstein" },
  "profile.colour.green": { en: "Green", sk: "Zelená", de: "Grün" },

  "profile.pronouns.title": { en: "Pronouns", sk: "Zámená", de: "Pronomen" },
  "profile.pronouns.placeholder": { en: "they/them", sk: "ona/jej", de: "sie/ihr" },
  "profile.about.title": { en: "About you", sk: "O vás", de: "Über dich" },
  "profile.about.placeholder": {
    en: "Keeps the family photos in order",
    sk: "Stará sa o rodinné fotky",
    de: "Hält die Familienfotos in Ordnung",
  },
  "profile.about.left": {
    en: { one: "{count} character left", other: "{count} characters left" },
    sk: {
      one: "Zostáva {count} znak",
      few: "Zostávajú {count} znaky",
      many: "Zostáva {count} znakov",
      other: "Zostáva {count} znakov",
    },
    de: { one: "Noch {count} Zeichen", other: "Noch {count} Zeichen" },
  },

  "profile.crop.title.picture": {
    en: "Your new picture",
    sk: "Vaša nová fotka",
    de: "Dein neues Bild",
  },
  "profile.crop.title.banner": {
    en: "Your new banner",
    sk: "Váš nový banner",
    de: "Dein neues Banner",
  },
  "profile.crop.body": {
    en: "Drag to move the photo and zoom to pick the part you want.",
    sk: "Ťahaním posuňte fotku a priblížením vyberte časť, ktorú chcete.",
    de: "Zieh das Foto zurecht und zoome auf den Teil, den du willst.",
  },
  "profile.crop.canvas": {
    en: "The part of the photo that will be used. Arrow keys move it, plus and minus zoom.",
    sk: "Časť fotky, ktorá sa použije. Šípky ju posúvajú, plus a mínus približujú.",
    de: "Der Teil des Fotos, der verwendet wird. Pfeiltasten verschieben, Plus und Minus zoomen.",
  },
  "profile.crop.unreadable": {
    en: "This file could not be opened as a picture. Try a PNG, JPEG, GIF or WebP.",
    sk: "Tento súbor sa nepodarilo otvoriť ako obrázok. Skúste PNG, JPEG, GIF alebo WebP.",
    de: "Diese Datei ließ sich nicht als Bild öffnen. Versuch es mit PNG, JPEG, GIF oder WebP.",
  },
  "profile.crop.zoom": { en: "Zoom", sk: "Priblíženie", de: "Zoom" },
  "profile.crop.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "profile.crop.save": { en: "Use this", sk: "Použiť", de: "Übernehmen" },
  "profile.crop.saving": { en: "Saving…", sk: "Ukladám…", de: "Speichere…" },

  "profile.users.takeDown.picture": {
    en: "Remove picture",
    sk: "Odstrániť fotku",
    de: "Bild entfernen",
  },
  "profile.users.takeDown.banner": {
    en: "Remove banner",
    sk: "Odstrániť banner",
    de: "Banner entfernen",
  },
  "profile.users.takenDown": {
    en: "Removed from {name}'s profile",
    sk: "Odstránené z profilu používateľa {name}",
    de: "Aus dem Profil von {name} entfernt",
  },
});
