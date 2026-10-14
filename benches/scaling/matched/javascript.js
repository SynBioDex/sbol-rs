'use strict';
const fs = require('fs');
const crypto = require('crypto');
const {Graph, parseRDF, serialize, Filetype} = require('rdfoo');
const {SBOL3GraphView} = require('sboljs');
const S = 'http://sbols.org/v3#';
const TYPE = 'http://www.w3.org/1999/02/22-rdf-syntax-ns#type';

function snapshot(view) {
  const rows = [];
  function row(obj, predicate, kind, value) {
    if (value === undefined || value === null) throw Error('missing '+predicate);
    rows.push(`${obj.uri}\t${predicate}\t${kind}\t${value}\n`);
  }
  const prop = (obj, name, kind, value) => row(obj, S+name, kind, value);
  function base(obj, cls, top) {
    row(obj, TYPE, 'I', obj.facadeType);
    if (obj.facadeType !== S+cls) throw Error('unexpected class');
    prop(obj, 'displayId', 'S', obj.displayId);
    if (top) prop(obj, 'hasNamespace', 'I', obj.namespace);
  }
  for (const c of view.components) {
    base(c, 'Component', true);
    for (const v of c.types) prop(c, 'type', 'I', v);
    for (const v of c.roles) prop(c, 'role', 'I', v);
    for (const v of c.sequences) prop(c, 'hasSequence', 'I', v.uri);
    for (const f of c.subComponents) {
      prop(c, 'hasFeature', 'I', f.uri); base(f, 'SubComponent', false);
      prop(f, 'instanceOf', 'I', f.instanceOf.uri);
      prop(f, 'orientation', 'I', f.orientation);
      for (const r of f.locations) {
        prop(f, 'hasLocation', 'I', r.uri); base(r, 'Range', false);
        prop(r, 'hasSequence', 'I', r.sequence.uri);
        prop(r, 'orientation', 'I', r.orientation);
        prop(r, 'start', 'N', r.start); prop(r, 'end', 'N', r.end);
      }
    }
  }
  for (const c of view.sequences) {
    base(c, 'Sequence', true);
    prop(c, 'elements', 'S', c.elements); prop(c, 'encoding', 'I', c.encoding);
  }
  return rows;
}
function rdfRows(graph) {
  return graph.match(null,null,null).map(t => {
    const o = t.object;
    const kind = o.termType === 'NamedNode' ? 'I' : o.datatype.value.endsWith('#integer') ? 'N' : 'S';
    return `${t.subject.value}\t${t.predicate.value}\t${kind}\t${o.value}\n`;
  });
}
async function main() {
  const [path,mode,warmRaw,itersRaw,output,expected] = process.argv.slice(2);
  const warm = Number(warmRaw), iters = Number(itersRaw), input = fs.readFileSync(path,'utf8');
  const result = {implementation:'javascript',mode,warmup:warm,iterations:iters,time_ns:[],serialize_ns:[],fingerprints:[],row_counts:[],serialized_bytes:0};
  const original = process.stdout.write; process.stdout.write = () => true;
  try {
    for (let i=0; i<warm+iters; i++) {
      const start = process.hrtime.bigint();
      const graph = new Graph(); await parseRDF(graph,input,Filetype.RDFXML);
      let rows, elapsed, serial=0;
      if (mode === 'rdf') { elapsed = Number(process.hrtime.bigint()-start); rows=rdfRows(graph); }
      else {
        const view = new SBOL3GraphView(graph); rows=snapshot(view);
        elapsed=Number(process.hrtime.bigint()-start);
        const s0=process.hrtime.bigint(); const out=serialize(graph,view.defaultPrefixes||new Map(),()=>false,'');
        serial=Number(process.hrtime.bigint()-s0); result.serialized_bytes=Buffer.byteLength(out);
      }
      rows.sort(); const digest=crypto.createHash('sha256'); for (const row of rows) digest.update(row);
      const hash=digest.digest('hex'); if(hash!==expected) throw Error('endpoint hash '+hash+' != '+expected);
      if(i>=warm) { result.time_ns.push(elapsed); result.serialize_ns.push(serial); result.fingerprints.push(hash); result.row_counts.push(rows.length); }
      process.stderr.write(`completed ${mode} iteration ${i+1}/${warm+iters}\n`);
    }
  } finally { process.stdout.write=original; }
  fs.writeFileSync(output,JSON.stringify(result));
}
main().catch(e=>{process.stderr.write(e.stack+'\n');process.exitCode=1;});
