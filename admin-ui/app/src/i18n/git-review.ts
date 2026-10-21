import { defineMessages } from "./define";

/* LosOS Reviewer's switch in a LosOS Git repository's settings
 * (src/git/ai-review.tsx). Its rows on a pull request live in git.ts. */

export default defineMessages({
  "gitReview.title": { en: "AI review", sk: "Kontrola s AI", de: "KI-Review" },
  "gitReview.label": {
    en: "Review pull requests with LosOS Reviewer",
    sk: "Kontrolovať pull requesty cez LosOS Reviewer",
    de: "Pull-Requests mit LosOS Reviewer prüfen",
  },
  "gitReview.hint": {
    en: "The box's own code model comments on every pull request when it is opened or pushed to. It never approves or merges, and the code stays on this box.",
    sk: "Vlastný model zariadenia okomentuje každý pull request, keď sa otvorí alebo doň pribudnú commity. Nikdy neschvaľuje ani nezlučuje a kód zostáva na tomto zariadení.",
    de: "Das eigene Code-Modell der Box kommentiert jeden Pull-Request, wenn er geöffnet wird oder neue Commits bekommt. Es genehmigt nie und führt nie zusammen, und der Code bleibt auf dieser Box.",
  },
  "gitReview.hintModel": {
    en: "{model} on this box comments on every pull request when it is opened or pushed to. It never approves or merges, and the code stays on this box.",
    sk: "{model} na tomto zariadení okomentuje každý pull request, keď sa otvorí alebo doň pribudnú commity. Nikdy neschvaľuje ani nezlučuje a kód zostáva na tomto zariadení.",
    de: "{model} auf dieser Box kommentiert jeden Pull-Request, wenn er geöffnet wird oder neue Commits bekommt. Es genehmigt nie und führt nie zusammen, und der Code bleibt auf dieser Box.",
  },
  "gitReview.unavailable": {
    en: "Needs the box's code model. Turn on Build with AI in the settings first.",
    sk: "Potrebuje model zariadenia. Najprv v nastaveniach zapnite Vytvoriť s AI.",
    de: "Braucht das Code-Modell der Box. Schalte zuerst Mit KI bauen in den Einstellungen ein.",
  },
  "gitReview.turnedOn": { en: "AI review is on", sk: "Kontrola s AI je zapnutá", de: "KI-Review ist an" },
  "gitReview.turnedOff": { en: "AI review is off", sk: "Kontrola s AI je vypnutá", de: "KI-Review ist aus" },
  "gitReview.failed": {
    en: "AI review was not changed",
    sk: "Kontrola s AI sa nezmenila",
    de: "KI-Review wurde nicht geändert",
  },
});
