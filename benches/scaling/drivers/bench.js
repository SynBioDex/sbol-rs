// Round-trip benchmark wrapper for sboljs (a.k.a. sboljs3). Reads an
// SBOL 3 RDF document, runs the configured number of warmup and timed
// (parse + serialize) iterations, and writes per-iteration nanosecond
// timings to a JSON file.
//
// Invoked from inside the Docker container built by the sibling
// Dockerfile. rdfoo (sboljs's parser stack) only supports N-Triples
// and RDF/XML input and only serializes RDF/XML, so the driver in
// crates/sbol/src/bin/run-cross-impl-bench.rs restricts the bench
// matrix accordingly. The script fails loudly if asked to use an
// unsupported format combination.
//
// Usage:
//   node bench.js <input> <parse_fmt> <serialize_fmt> <warmup> <iters> <output_json>

'use strict';

const fs = require('fs');
const { Graph, parseRDF, serialize, Filetype } = require('rdfoo');
const { SBOL3GraphView } = require('sboljs');

const SUPPORTED_PARSE = new Set(['ntriples', 'rdfxml']);
const SUPPORTED_SERIALIZE = new Set(['rdfxml']);

function filetypeFor(format) {
  switch (format) {
    case 'ntriples':
      return Filetype.NTriples;
    case 'rdfxml':
      return Filetype.RDFXML;
    default:
      throw new Error(`unsupported sboljs parse format: ${format}`);
  }
}

async function main(argv) {
  if (argv.length < 7) {
    process.stderr.write(
      'usage: bench.js <input> <parse_fmt> <serialize_fmt> <warmup> <iters> <output_json>\n',
    );
    process.exit(2);
  }

  const [, , inputPath, parseFmt, serializeFmt, warmupRaw, itersRaw, outputJson] = argv;
  const warmup = parseInt(warmupRaw, 10);
  const iters = parseInt(itersRaw, 10);
  if (!Number.isFinite(warmup) || !Number.isFinite(iters) || warmup < 0 || iters <= 0) {
    process.stderr.write(`bad warmup/iters: ${warmupRaw}, ${itersRaw}\n`);
    process.exit(2);
  }

  if (!SUPPORTED_PARSE.has(parseFmt)) {
    process.stderr.write(
      `sboljs cannot parse format=${parseFmt}; supported: ${[...SUPPORTED_PARSE].join(',')}\n`,
    );
    process.exit(3);
  }
  if (!SUPPORTED_SERIALIZE.has(serializeFmt)) {
    process.stderr.write(
      `sboljs cannot serialize format=${serializeFmt}; supported: ${[...SUPPORTED_SERIALIZE].join(',')}\n`,
    );
    process.exit(3);
  }

  const rdfText = fs.readFileSync(inputPath, { encoding: 'utf8' });
  const filetype = filetypeFor(parseFmt);

  const parseNs = new Array(iters);
  const serializeNs = new Array(iters);
  // Track the byte size of the most recent serialization so the driver
  // can sanity-check that the round trip produced output (versus an
  // empty string from an error path).
  let lastSerializedBytes = 0;

  // The library prints `console.log('cows')` from serialize() in
  // released versions. Swallow stdout writes during the timed loop so
  // they neither pollute the JSON output channel nor pay I/O cost on
  // every iteration.
  const originalStdoutWrite = process.stdout.write.bind(process.stdout);
  process.stdout.write = () => true;

  try {
    for (let i = 0; i < warmup; i += 1) {
      const graph = new Graph();
      await parseRDF(graph, rdfText, filetype);
      const view = new SBOL3GraphView(graph);
      serialize(graph, view.defaultPrefixes || new Map(), () => false, '');
    }

    for (let i = 0; i < iters; i += 1) {
      const graph = new Graph();
      const t0 = process.hrtime.bigint();
      await parseRDF(graph, rdfText, filetype);
      const t1 = process.hrtime.bigint();
      const view = new SBOL3GraphView(graph);
      const out = serialize(graph, view.defaultPrefixes || new Map(), () => false, '');
      const t2 = process.hrtime.bigint();
      parseNs[i] = Number(t1 - t0);
      serializeNs[i] = Number(t2 - t1);
      lastSerializedBytes = Buffer.byteLength(out, 'utf8');
    }
  } finally {
    process.stdout.write = originalStdoutWrite;
  }

  const result = {
    impl: 'sboljs',
    version: require('sboljs/package.json').version,
    fixture: inputPath,
    parse_format: parseFmt,
    serialize_format: serializeFmt,
    warmup_iters: warmup,
    measured_iters: iters,
    serialized_bytes: lastSerializedBytes,
    parse_ns: parseNs,
    serialize_ns: serializeNs,
  };
  fs.writeFileSync(outputJson, JSON.stringify(result));
}

main(process.argv).catch((error) => {
  process.stderr.write(`bench failed: ${error && error.stack ? error.stack : error}\n`);
  process.exit(1);
});
