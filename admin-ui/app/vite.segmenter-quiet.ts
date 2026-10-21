import type { Plugin } from "vite";

/* MediaPipe's tasks-vision posts usage counts to Google every minute, from
 * a logger nothing in its API turns off. The page's CSP refuses the
 * request anyway, but each refusal is a console error on a call. This
 * empties the logger's queue before it is sent, so nothing leaves the
 * page and nothing is refused. A MediaPipe release that changes the
 * logger fails the build here instead of calling out again. */
const SEND = "flush(t,e){if(this.error)";
const QUIET = "flush(t,e){this.i=[];if(this.error)";
const URL = "https://odml.pa.googleapis.com/v1/log";

export function segmenterQuiet(): Plugin {
  return {
    name: "losos:segmenter-quiet",
    apply: "build",
    transform(code, id) {
      if (!id.includes("@mediapipe/tasks-vision/vision_bundle")) return null;
      if (!code.includes(URL)) return null;
      if (code.split(SEND).length !== 2) {
        this.error("MediaPipe's usage logger changed: look at vite.segmenter-quiet.ts again");
      }
      return { code: code.replace(SEND, QUIET), map: null };
    },
  };
}
