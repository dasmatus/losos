#!/usr/bin/env bun
/* The box's production build of the admin UI, on Bun alone.
 *
 *   bun run build
 *
 * It does, in order:
 *   1. the basic-classes scan (a checkout only; see scripts/bundler.ts);
 *   2. `tailwindcss` once, into the app stylesheet every page links;
 *   3. `rm -rf dist/` — byte-identical hashed filenames across builds depend
 *      on Tailwind not scanning the last build (admin-ui/app/.gitignore);
 *   4. Bun.build over the pages' entrypoints, sharing chunks, and a second
 *      build under /calls/guest/ for the guest call page;
 *   5. theme-boot.js and the app frame runtime (classic, unhashed);
 *   6. dist/pages/ for the pages plugin, basic/messages.<locale>.json for
 *      the basic plugin, public/, the pinned WebAssembly and model files,
 *      and the built HTML documents.
 *
 * VITE_LAB_HOSTED=1 adds the hosted Diagrams copy's page (nothing else
 * changes); LOSOS_LAB_MEME, LOSOS_SEGMENTER_MODEL and LOSOS_NUDITY_MODEL
 * come from nix/packages.nix in that build. In a checkout you must have run
 * `bun run lab:core`, `lab:virt`, `lab:render` and `mdx` first — the typecheck
 * and this build need src/lab/*-pkg and src/mdx/pkg, which those write. */

import {
  bundleAll,
  buildEnv,
  compileAppCss,
  compileAppFrame,
  compileThemeBoot,
  DIST,
  emitBasicMessages,
  emitPageSources,
  ensureBasicClasses,
  pageSpecs,
  rewriteHtml,
  ROOT,
} from "./bundler";

/* The whole build, as a function: `bun run build` runs it once with the
 * process's environment (at the bottom), and scripts/dev.ts calls it again
 * after the files it watches change. */
export async function buildSite(
  bunEnv: Record<string, string | undefined>,
  log = true,
): Promise<void> {
  const env = buildEnv(bunEnv);
  if (log) console.log("[build] tailwind");
  await ensureBasicClasses();
  const appCss = await compileAppCss();

  if (log) console.log("[build] wiping dist/");
  await Bun.$`rm -rf ${DIST}`;

  if (log) console.log("[build] pages");
  const { chunks, appKeys } = await bundleAll(env, appCss);

  if (log) console.log("[build] theme-boot + app frame");
  await Bun.write(`${DIST}/theme-boot.js`, await compileThemeBoot(true));
  const frame = await compileAppFrame(true);
  await Bun.write(`${DIST}/app-frame/runtime.js`, frame.js);
  await Bun.write(`${DIST}/app-frame/runtime.css`, frame.css);

  if (log) console.log("[build] pages assets + basic messages");
  await emitPageSources(DIST);
  await emitBasicMessages(DIST);

  if (log) console.log("[build] static files");
  await Bun.$`cp -a ${ROOT}/public/. ${DIST}/`;
  /* The favicon: stable and unhashed. It is a <link> in every HTML
   * document, not an import, so the bundle's hashed file loader never
   * carries it; the documents are rewritten below to point at
   * assets/losos.png instead of /src/assets/. */
  await Bun.$`cp ${ROOT}/src/assets/losos.png ${DIST}/assets/losos.png`;

  /* The pinned media the pages fetch from the box itself. losos-mdx's
   * wasm-bindgen package is already under src/mdx/pkg (from `bun run mdx`,
   * or preConfigure in nix/packages.nix), the segmenter's WebAssembly and
   * the ONNX runtime come from the devDependencies' packages, and each
   * optional model arrives only when its environment named it — a build
   * without it still builds, and the call then sends no picture under the
   * black background, the warning then lets every picture through. */
  const segmenter = `${ROOT}/node_modules/@mediapipe/tasks-vision/wasm`;
  await Bun.write(
    `${DIST}/chat/calls-ml/vision_wasm_internal.js`,
    Bun.file(`${segmenter}/vision_wasm_internal.js`),
  );
  await Bun.write(
    `${DIST}/chat/calls-ml/vision_wasm_internal.wasm`,
    Bun.file(`${segmenter}/vision_wasm_internal.wasm`),
  );
  if (env.segmenterModel !== undefined) {
    await Bun.write(`${DIST}/chat/calls-ml/selfie_segmenter.tflite`, Bun.file(env.segmenterModel));
  }
  const ort = `${ROOT}/node_modules/onnxruntime-web/dist`;
  await Bun.write(`${DIST}/nudity/ort-wasm-simd-threaded.mjs`, Bun.file(`${ort}/ort-wasm-simd-threaded.mjs`));
  await Bun.write(`${DIST}/nudity/ort-wasm-simd-threaded.wasm`, Bun.file(`${ort}/ort-wasm-simd-threaded.wasm`));
  if (env.nudityModel !== undefined) {
    await Bun.write(`${DIST}/nudity/model.onnx`, Bun.file(env.nudityModel));
  }
  /* LosOS Lab's splash can carry a two-panel meme: only when
   * LOSOS_LAB_MEME names their folder (the repository carries none). */
  if (env.memeDir !== "") {
    await Bun.write(`${DIST}/diagrams/meme-no.jpg`, Bun.file(`${env.memeDir}/no.jpg`));
    await Bun.write(`${DIST}/diagrams/meme-yes.jpg`, Bun.file(`${env.memeDir}/yes.jpg`));
  }

  if (log) console.log("[build] HTML documents");
  for (const spec of pageSpecs(env)) {
    const source = await Bun.file(`${ROOT}/${spec.html}`).text();
    const html = rewriteHtml(source, `/${spec.entry}`, env, chunks.get(spec.html) ?? "", appKeys.box);
    await Bun.write(`${DIST}/${spec.html}`, html);
  }
  /* The guest document lands at /calls/guest/guest.html — the name nginx's
   * try_files hands a guest whose link stopped before the file part. */
  {
    const source = await Bun.file(`${ROOT}/guest.html`).text();
    const html = rewriteHtml(
      source,
      "/src/calls/guest-main.tsx",
      { ...env, base: "/calls/guest/" },
      chunks.get("guest.html") ?? "",
      appKeys.guest,
    );
    await Bun.write(`${DIST}/calls/guest/guest.html`, html);
  }

  if (log) console.log("[build] done");
}

if (import.meta.main) {
  await buildSite(Bun.env as unknown as Record<string, string | undefined>);
}
