import {
  catalogFixture,
  cleanup,
  runConsumer,
  runDictation,
  startBackend,
  stopBackend,
} from "./app-consumer-headless.mjs";

const apps = {
  arcadia: {
    id: "com.kosmos.arcadia",
    version: "0.1.11",
    sha256: "a4957008562b3ddc1c2bc59d15c03cfe1cbf2c4f0c402bf9d0160ef7c3bb5d7d",
    archive:
      process.env.KOSMOS_ARCADIA_ARCHIVE ??
      "C:/Users/kirill/Coding/makekosmos/arcadia/release/arcadia-0.1.11.kspkg",
    catalogFixture:
      process.env.KOSMOS_ARCADIA_CATALOG_FIXTURE ??
      "C:/Users/kirill/Coding/makekosmos/.tmp-cortex-arcadia011-fixture",
    catalogSequence: 1,
    backend: process.env.KOSMOS_ARCADIA_BACKEND,
  },
  dictation: {
    id: "com.kosmos.dictation",
    version: "0.2.4",
    sha256: "a7eaf9c84ee63fc01799df531ba0469390a20c4a4df6e37f1c9901af8a4c5fd5",
    archive:
      process.env.KOSMOS_DICTATION_ARCHIVE ??
      "C:/Users/kirill/Coding/makekosmos/package-index-catalog-14/.tmp-artifacts/dictation-0.2.4.kspkg",
    catalogFixture:
      process.env.KOSMOS_DICTATION_CATALOG_FIXTURE ??
      "C:/Users/kirill/Coding/makekosmos/.tmp-cortex-catalog14-fixture",
    catalogSequence: 14,
    backend: process.env.KOSMOS_DICTATION_BACKEND,
  },
};

let running;
try {
  running = await startBackend(apps.dictation);
  const dictation = await runConsumer(apps.dictation, running);
  await runDictation(dictation);
  await stopBackend(running);
  running = undefined;
  running = await startBackend(apps.arcadia);
  await runConsumer(apps.arcadia, running);
  console.log("arcadia consumer production install and boundary cases passed");
  console.log(
    `app consumer headless evidence passed using ${running.dataDir} and ${catalogFixture}`,
  );
} finally {
  if (running) await stopBackend(running);
  await cleanup();
}
